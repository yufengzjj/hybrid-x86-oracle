//! WHP backend: **the host CPU itself** as the reference, on Windows.
//!
//! The Windows counterpart of [`KvmOracle`](crate::KvmOracle) — same idea, same
//! guest ([`crate::hw`]), different hypervisor interface. Both are hardware
//! ground truth: where a model and this backend disagree, the model is wrong (or
//! the behaviour is architecturally undefined and both are allowed). The same
//! caveats apply too: coverage is whatever *this* machine implements, and
//! `RDTSC`/`RDRAND`/`CPUID` are not reproducible references.
//!
//! # Requirements
//!
//! The **Windows Hypervisor Platform** optional feature (which implies Hyper-V):
//!
//! ```text
//! dism /Online /Enable-Feature /FeatureName:HypervisorPlatform /All
//! ```
//!
//! No SDK, import library or admin rights are needed to *build* — `winhvplatform.dll`
//! is resolved with `GetProcAddress` at first use, deliberately. An import-table
//! dependency would stop the whole test binary from starting on a machine without
//! WHP; instead [`WhpOracle::is_available`] returns `false` there,
//! [`Backend::available`](crate::Backend::available) omits it, and the portable
//! suite skips.
//!
//! # More than one instance
//!
//! Nothing stops you holding several `WhpOracle`s at once — each has its own
//! partition and its own RAM — but WHP lets only one partition per process have
//! guest RAM *mapped*, so they take turns (see [`MAPPING_OWNER`]). The handover
//! happens on [`step`](X86Oracle::step) and costs a few milliseconds, so code
//! that alternates between two live instances instruction by instruction will
//! feel it; using one at a time does not pay it at all.
//!
//! # Single-stepping and faults
//!
//! WHP has no single-step control of its own, so RFLAGS.TF is the mechanism —
//! which means it must be re-armed before every run, since every register write
//! rewrites RFLAGS. There are then two ways to see what the step did, and this
//! backend sets up both:
//!
//! 1. **Exception intercepts** (preferred). With `ExceptionExit` enabled and the
//!    vectors we care about in `ExceptionExitBitmap`, an exception exits to us
//!    *before* the CPU delivers it — RIP, RSP and RFLAGS are still the faulting
//!    instruction's, so the "faults do not vector" contract needs no repair work
//!    at all, and #DB tells us a step retired.
//! 2. **The IDT stubs** ([`crate::hw`]) otherwise. The fault (or the step's #DB)
//!    is delivered normally, lands on a `HLT`, and the pushed frame is unwound.
//!
//! The second is a complete mechanism on its own — it needs nothing from WHP but
//! halt exits — so this backend works even where exception intercepts are
//! unavailable. [`WhpOracle::uses_exception_intercepts`] reports which is live.

use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use windows_sys::core::{HRESULT, PCSTR};
use windows_sys::Win32::System::Hypervisor::{
    WHvCapabilityCodeExceptionExitBitmap, WHvCapabilityCodeExtendedVmExits,
    WHvCapabilityCodeHypervisorPresent, WHvCapabilityCodeProcessorFeatures,
    WHvCapabilityCodeProcessorXsaveFeatures, WHvMapGpaRangeFlagExecute, WHvMapGpaRangeFlagRead,
    WHvMapGpaRangeFlagWrite, WHvPartitionPropertyCodeExceptionExitBitmap,
    WHvPartitionPropertyCodeExtendedVmExits, WHvPartitionPropertyCodeProcessorCount,
    WHvPartitionPropertyCodeProcessorFeatures, WHvPartitionPropertyCodeProcessorXsaveFeatures,
    WHvRunVpExitReasonCanceled, WHvRunVpExitReasonException, WHvRunVpExitReasonMemoryAccess,
    WHvRunVpExitReasonUnrecoverableException, WHvRunVpExitReasonX64Halt, WHvX64RegisterCr0,
    WHvX64RegisterCr2, WHvX64RegisterCr3, WHvX64RegisterCr4, WHvX64RegisterCr8, WHvX64RegisterCs,
    WHvX64RegisterDs, WHvX64RegisterEfer, WHvX64RegisterEs, WHvX64RegisterFs, WHvX64RegisterGdtr,
    WHvX64RegisterGs, WHvX64RegisterIdtr, WHvX64RegisterKernelGsBase, WHvX64RegisterLdtr,
    WHvX64RegisterLstar, WHvX64RegisterR10, WHvX64RegisterR11, WHvX64RegisterR12,
    WHvX64RegisterR13, WHvX64RegisterR14, WHvX64RegisterR15, WHvX64RegisterR8, WHvX64RegisterR9,
    WHvX64RegisterRax, WHvX64RegisterRbp, WHvX64RegisterRbx, WHvX64RegisterRcx,
    WHvX64RegisterRdi, WHvX64RegisterRdx, WHvX64RegisterRflags, WHvX64RegisterRip,
    WHvX64RegisterRsi, WHvX64RegisterRsp, WHvX64RegisterSfmask, WHvX64RegisterSs,
    WHvX64RegisterStar, WHvX64RegisterTr, WHvX64RegisterXCr0, WHV_CAPABILITY_CODE,
    WHV_CPUID_OUTPUT, WHV_MAP_GPA_RANGE_FLAGS, WHV_PARTITION_HANDLE, WHV_PARTITION_PROPERTY_CODE,
    WHV_REGISTER_NAME, WHV_REGISTER_VALUE, WHV_RUN_VP_EXIT_CONTEXT, WHV_RUN_VP_EXIT_REASON,
    WHV_VP_EXCEPTION_CONTEXT, WHV_VP_EXIT_CONTEXT, WHV_X64_SEGMENT_REGISTER,
    WHV_X64_SEGMENT_REGISTER_0, WHV_X64_TABLE_REGISTER,
};

use crate::hw::{
    self, CpuidRegs, GuestRam, XsaveLayout, CR0_INIT, CR4_INIT, CTRL_BASE, EFER_INIT, GDT_ADDR,
    GDT_LIMIT, IDT_ADDR, IDT_LIMIT, PML4_ADDR, RAM_SIZE, SEL_CODE64, SEL_DATA, SEL_TSS, TSS_ADDR,
    TSS_LIMIT,
};
use crate::{
    FaultKind, StepOutcome, X86Oracle, ZMM_CHUNKS, ZMM_REGS, FLAG_RF, FLAG_TF, IA32_EFER,
    IA32_FMASK, IA32_FS_BASE, IA32_GS_BASE, IA32_KERNEL_GS_BASE, IA32_LSTAR, IA32_STAR, SEG_COUNT,
    SEG_CS, SEG_DS, SEG_ES, SEG_FS, SEG_SS,
};

// ------------------------------------------------------------------- ABI checks

// The parts of the WHP ABI this file reads through unions, pinned at compile
// time. These layouts come from windows-sys' generated bindings rather than the
// SDK headers, and a silent disagreement would corrupt every register read
// instead of failing — so disagree loudly, at build time.
const _: () = {
    assert!(size_of::<WHV_REGISTER_VALUE>() == 16);
    // Aligning must not repad: an array of these is handed to WHP as an array of
    // the bare type, so the stride has to stay identical.
    assert!(size_of::<Aligned16<WHV_REGISTER_VALUE>>() == size_of::<WHV_REGISTER_VALUE>());
    assert!(size_of::<WHV_X64_SEGMENT_REGISTER>() == 16);
    assert!(size_of::<WHV_X64_TABLE_REGISTER>() == 16);
    assert!(size_of::<WHV_VP_EXIT_CONTEXT>() == 40);
    assert!(size_of::<WHV_CPUID_OUTPUT>() == 16);
    // The exit context is a fixed header (reason, reserved, VpContext) followed
    // by the union of per-reason contexts, so it is at least both together.
    assert!(std::mem::offset_of!(WHV_RUN_VP_EXIT_CONTEXT, VpContext) == 8);
    assert!(
        size_of::<WHV_RUN_VP_EXIT_CONTEXT>()
            >= 8 + size_of::<WHV_VP_EXIT_CONTEXT>() + size_of::<WHV_VP_EXCEPTION_CONTEXT>()
    );
};

// ------------------------------------------------------- buffers we hand to WHP

/// A value staged with the 16-byte alignment `winhvplatform.dll` requires.
///
/// The DLL moves register values and the exit context around with `movaps`, which
/// raises #GP unless the buffer is 16-byte aligned. windows-sys declares these
/// types `#[repr(C)]` over 64-bit members, so Rust guarantees only 8 — and 8 is
/// enough often enough to look fine: whether a given array lands on 0 or 8 mod 16
/// depends on where the compiler happened to put it, and WHP only uses the
/// aligned move on some of its register paths (`WHvX64RegisterTr` and
/// `WHvX64RegisterLdtr` are two that do).
///
/// When it does bite, the report is spectacularly misleading. An alignment fault
/// has no faulting address, so Windows surfaces it as `STATUS_ACCESS_VIOLATION`
/// *"reading address 0xFFFFFFFFFFFFFFFF"* from an address inside
/// winhvplatform.dll — which reads like a null-ish pointer bug in WHP rather than
/// "your buffer is off by 8". Asking for `align(16)` in the type makes every
/// array of them correctly aligned wherever it lives, so no call site has to
/// know any of this.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Aligned16<T>(T);

/// A register value, in the alignment WHP wants to see it in.
type RegValue = Aligned16<WHV_REGISTER_VALUE>;

fn reg64(value: u64) -> RegValue {
    Aligned16(WHV_REGISTER_VALUE { Reg64: value })
}

fn seg_value(seg: WHV_X64_SEGMENT_REGISTER) -> RegValue {
    Aligned16(WHV_REGISTER_VALUE { Segment: seg })
}

fn table_value(table: WHV_X64_TABLE_REGISTER) -> RegValue {
    Aligned16(WHV_REGISTER_VALUE { Table: table })
}

// --------------------------------------------------------------- segment layout

/// Segment attribute words, in the "access rights byte + high nibble" packing
/// WHP uses for a segment's `Attributes` (the same packing VMX stores).
const ATTR_CODE64: u16 = 0xA09B; // type 0xB, S, P, L, G
const ATTR_DATA: u16 = 0xC093; // type 0x3, S, P, D/B, G
const ATTR_TSS64: u16 = 0x008B; // type 0xB (busy 64-bit TSS), system, P

/// `WHV_EXTENDED_VM_EXITS.ExceptionExit` — bit 2 of that bitfield.
const EXTENDED_EXIT_EXCEPTION: u64 = 1 << 2;

// The exit reasons this backend acts on. Aliased partly because a pattern needs
// a screaming-case path to read as a constant rather than a binding, and partly
// because these names say what the exit means to us.
const EXIT_EXCEPTION_INTERCEPT: WHV_RUN_VP_EXIT_REASON = WHvRunVpExitReasonException;
const EXIT_HALT: WHV_RUN_VP_EXIT_REASON = WHvRunVpExitReasonX64Halt;
const EXIT_CANCELED: WHV_RUN_VP_EXIT_REASON = WHvRunVpExitReasonCanceled;
const EXIT_MEMORY_ACCESS: WHV_RUN_VP_EXIT_REASON = WHvRunVpExitReasonMemoryAccess;
const EXIT_TRIPLE_FAULT: WHV_RUN_VP_EXIT_REASON = WHvRunVpExitReasonUnrecoverableException;

/// Vectors worth intercepting: everything a user-mode instruction can raise,
/// plus #DB because that is how a completed step announces itself. Deliberately
/// absent are NMI (2), #DF (8) and #MC (18) — a double fault would mean our own
/// IDT is broken, and the other two are machine events, not instruction
/// semantics. The bitmap is intersected with what WHP reports as supported.
const WANTED_EXCEPTIONS: u64 = (1 << 0) // #DE
    | (1 << 1) // #DB  <- the single-step trap
    | (1 << 3) // #BP
    | (1 << 4) // #OF
    | (1 << 5) // #BR
    | (1 << 6) // #UD
    | (1 << 7) // #NM
    | (1 << 10) // #TS
    | (1 << 11) // #NP
    | (1 << 12) // #SS
    | (1 << 13) // #GP
    | (1 << 14) // #PF
    | (1 << 16) // #MF
    | (1 << 17) // #AC
    | (1 << 19) // #XM
    | (1 << 21); // #CP

/// XCR0 bits. X87 and SSE are mandatory; the rest are what we try to enable.
const XCR0_X87_SSE: u64 = 0b11;
const XCR0_AVX: u64 = 1 << 2;
const XCR0_AVX512: u64 = (1 << 5) | (1 << 6) | (1 << 7);
/// MPX (3, 4) and PKRU (9) — the first components to give up on, since neither
/// is vector state we expose.
const XCR0_OPTIONAL: u64 = (1 << 3) | (1 << 4) | (1 << 9);

// ------------------------------------------------------------------ WHP binding

/// What `GetProcAddress` hands back before we assert a real signature onto it.
type RawProc = unsafe extern "system" fn() -> isize;

type FnGetCapability =
    unsafe extern "system" fn(WHV_CAPABILITY_CODE, *mut c_void, u32, *mut u32) -> HRESULT;
type FnCreatePartition = unsafe extern "system" fn(*mut WHV_PARTITION_HANDLE) -> HRESULT;
type FnPartition = unsafe extern "system" fn(WHV_PARTITION_HANDLE) -> HRESULT;
type FnSetProperty = unsafe extern "system" fn(
    WHV_PARTITION_HANDLE,
    WHV_PARTITION_PROPERTY_CODE,
    *const c_void,
    u32,
) -> HRESULT;
type FnGetProperty = unsafe extern "system" fn(
    WHV_PARTITION_HANDLE,
    WHV_PARTITION_PROPERTY_CODE,
    *mut c_void,
    u32,
    *mut u32,
) -> HRESULT;
type FnMapGpaRange = unsafe extern "system" fn(
    WHV_PARTITION_HANDLE,
    *const c_void,
    u64,
    u64,
    WHV_MAP_GPA_RANGE_FLAGS,
) -> HRESULT;
type FnUnmapGpaRange = unsafe extern "system" fn(WHV_PARTITION_HANDLE, u64, u64) -> HRESULT;
type FnCreateVp = unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32, u32) -> HRESULT;
type FnDeleteVp = unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32) -> HRESULT;
type FnRunVp = unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32, *mut c_void, u32) -> HRESULT;
type FnGetRegisters = unsafe extern "system" fn(
    WHV_PARTITION_HANDLE,
    u32,
    *const WHV_REGISTER_NAME,
    u32,
    *mut WHV_REGISTER_VALUE,
) -> HRESULT;
type FnSetRegisters = unsafe extern "system" fn(
    WHV_PARTITION_HANDLE,
    u32,
    *const WHV_REGISTER_NAME,
    u32,
    *const WHV_REGISTER_VALUE,
) -> HRESULT;
type FnGetXsave =
    unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32, *mut c_void, u32, *mut u32) -> HRESULT;
type FnSetXsave =
    unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32, *const c_void, u32) -> HRESULT;
type FnCpuidOutput =
    unsafe extern "system" fn(WHV_PARTITION_HANDLE, u32, u32, u32, *mut WHV_CPUID_OUTPUT) -> HRESULT;

/// The WHP entry points, resolved once per process.
struct Whp {
    get_capability: FnGetCapability,
    create_partition: FnCreatePartition,
    setup_partition: FnPartition,
    delete_partition: FnPartition,
    set_property: FnSetProperty,
    get_property: FnGetProperty,
    map_gpa_range: FnMapGpaRange,
    unmap_gpa_range: FnUnmapGpaRange,
    create_vp: FnCreateVp,
    delete_vp: FnDeleteVp,
    run_vp: FnRunVp,
    get_registers: FnGetRegisters,
    set_registers: FnSetRegisters,
    get_xsave: FnGetXsave,
    set_xsave: FnSetXsave,
    /// Added after the original WHP release: lets us ask what CPUID the *guest*
    /// sees rather than assuming it matches the host's.
    cpuid_output: Option<FnCpuidOutput>,
}

// SAFETY: plain function pointers into a DLL that stays loaded for the process
// lifetime (it is never freed).
unsafe impl Send for Whp {}
unsafe impl Sync for Whp {}

static WHP: OnceLock<Option<Whp>> = OnceLock::new();

fn whp() -> Option<&'static Whp> {
    WHP.get_or_init(load_whp).as_ref()
}

fn load_whp() -> Option<Whp> {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    // SAFETY: a constant, NUL-terminated library name; the handle is
    // intentionally never freed, so the resolved pointers stay valid forever.
    let module = unsafe { LoadLibraryA(c"winhvplatform.dll".as_ptr().cast::<u8>()) };
    if module.is_null() {
        return None;
    }

    // Resolve or give up: a partial binding would fail later in a much more
    // confusing place than "WHP is not available". Each site names the signature
    // it is asserting, since that assertion is the whole safety argument.
    macro_rules! sym {
        ($ty:ty, $name:literal) => {{
            let name = concat!($name, "\0");
            // SAFETY: `name` is NUL-terminated, and `$ty` is the signature WHP
            // documents for that export.
            match unsafe { GetProcAddress(module, name.as_ptr() as PCSTR) } {
                Some(p) => unsafe { std::mem::transmute::<RawProc, $ty>(p) },
                None => return None,
            }
        }};
    }
    macro_rules! sym_opt {
        ($ty:ty, $name:literal) => {{
            let name = concat!($name, "\0");
            // SAFETY: as above.
            unsafe { GetProcAddress(module, name.as_ptr() as PCSTR) }
                .map(|p| unsafe { std::mem::transmute::<RawProc, $ty>(p) })
        }};
    }

    Some(Whp {
        get_capability: sym!(FnGetCapability, "WHvGetCapability"),
        create_partition: sym!(FnCreatePartition, "WHvCreatePartition"),
        setup_partition: sym!(FnPartition, "WHvSetupPartition"),
        delete_partition: sym!(FnPartition, "WHvDeletePartition"),
        set_property: sym!(FnSetProperty, "WHvSetPartitionProperty"),
        get_property: sym!(FnGetProperty, "WHvGetPartitionProperty"),
        map_gpa_range: sym!(FnMapGpaRange, "WHvMapGpaRange"),
        unmap_gpa_range: sym!(FnUnmapGpaRange, "WHvUnmapGpaRange"),
        create_vp: sym!(FnCreateVp, "WHvCreateVirtualProcessor"),
        delete_vp: sym!(FnDeleteVp, "WHvDeleteVirtualProcessor"),
        run_vp: sym!(FnRunVp, "WHvRunVirtualProcessor"),
        get_registers: sym!(FnGetRegisters, "WHvGetVirtualProcessorRegisters"),
        set_registers: sym!(FnSetRegisters, "WHvSetVirtualProcessorRegisters"),
        get_xsave: sym!(FnGetXsave, "WHvGetVirtualProcessorXsaveState"),
        set_xsave: sym!(FnSetXsave, "WHvSetVirtualProcessorXsaveState"),
        cpuid_output: sym_opt!(FnCpuidOutput, "WHvGetVirtualProcessorCpuidOutput"),
    })
}

fn ok(hr: HRESULT) -> bool {
    hr >= 0
}

fn check(what: &str, hr: HRESULT) -> std::io::Result<()> {
    if ok(hr) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("{what} failed: hr={hr:#010x}")))
    }
}

/// Read a capability that WHP reports as a `UINT64`-shaped union member.
fn capability_u64(api: &Whp, code: WHV_CAPABILITY_CODE) -> Option<u64> {
    let mut value = 0u64;
    let mut written = 0u32;
    // SAFETY: the buffer is exactly the 8 bytes we declare.
    let hr = unsafe {
        (api.get_capability)(
            code,
            (&mut value as *mut u64).cast(),
            size_of::<u64>() as u32,
            &mut written,
        )
    };
    (ok(hr) && written as usize == size_of::<u64>()).then_some(value)
}

// ---------------------------------------------------------------- the partition

/// Which partition currently holds this process's one guest-RAM mapping.
///
/// `WHvMapGpaRange` fails with `HV_STATUS_OPERATION_DENIED` (`0xC0370008`) while
/// *any other* partition in the process has a range mapped — one memory-backed
/// partition per process, whatever the size or guest address it asks for
/// (measured on Windows 11 build 26100). Unmapping the incumbent's range frees
/// the slot immediately, so oracles stay the independent objects they are
/// documented to be and simply take turns: the mapping is only needed while the
/// guest is *executing*, since registers go through the WHP API and memory
/// through our own host buffer.
///
/// Handing the slot over costs ~4 ms, so it happens lazily — an oracle keeps the
/// mapping until another one runs, and the ordinary case of a single live oracle
/// never hands it over at all.
static MAPPING_OWNER: Mutex<Option<WHV_PARTITION_HANDLE>> = Mutex::new(None);

fn mapping_owner() -> MutexGuard<'static, Option<WHV_PARTITION_HANDLE>> {
    MAPPING_OWNER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Owns the partition handle (and its single vCPU), so both are torn down even
/// if construction fails part-way.
struct Partition(WHV_PARTITION_HANDLE);

// SAFETY: a WHP partition handle is usable from any thread; the crate serializes
// access anyway.
unsafe impl Send for Partition {}
unsafe impl Sync for Partition {}

impl Drop for Partition {
    fn drop(&mut self) {
        let Some(api) = whp() else { return };
        // Give up the mapping slot before the handle dies: WHP is free to hand
        // the same handle value to the next partition, and a stale owner would
        // then send the next unmap to the wrong place.
        let mut owner = mapping_owner();
        if *owner == Some(self.0) {
            // SAFETY: our own mapping, released once.
            unsafe { (api.unmap_gpa_range)(self.0, 0, RAM_SIZE) };
            *owner = None;
        }
        // Errors are meaningless here: deleting a vCPU that was never created
        // simply fails, and there is nothing to report to.
        // SAFETY: our own handle, deleted exactly once.
        unsafe {
            (api.delete_vp)(self.0, 0);
            (api.delete_partition)(self.0);
        }
    }
}

// ------------------------------------------------------------------ XSAVE image

/// A 64-byte-aligned XSAVE image buffer. The architecture aligns XSAVE areas to
/// 64 bytes; WHP does not document a requirement, but honouring it costs nothing.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct XsaveBlock([u8; 64]);

struct XsaveImage {
    blocks: Vec<XsaveBlock>,
    /// How much of the buffer is meaningful — what WHP reported writing.
    len: usize,
}

impl XsaveImage {
    fn new(capacity: usize) -> Self {
        XsaveImage { blocks: vec![XsaveBlock([0; 64]); capacity.div_ceil(64)], len: 0 }
    }

    fn capacity(&self) -> usize {
        self.blocks.len() * 64
    }

    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `len <= capacity`, and the blocks are a contiguous POD array.
        unsafe { std::slice::from_raw_parts(self.blocks.as_ptr().cast::<u8>(), self.len) }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as above.
        unsafe { std::slice::from_raw_parts_mut(self.blocks.as_mut_ptr().cast::<u8>(), self.len) }
    }
}

// ---------------------------------------------------------------------- oracle

/// WHP partitions are independent, so instances are too; the crate serializes
/// anyway to keep the API uniform across backends.
static WHP_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    WHP_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct WhpOracle {
    // Drop order matters: the partition must go before the RAM its GPA range
    // points at. Rust drops fields in declaration order.
    partition: Partition,
    ram: GuestRam,
    xsave: XsaveLayout,
    /// Buffer size for the XSAVE get/set calls.
    xsave_capacity: usize,
    exception_intercept: bool,
    last_fault: Option<(u8, u64)>,
    /// Pages this instance has written through the direct memory API. Guest
    /// stores are invisible here, so `is_mapped` is a hint (as documented).
    touched: HashSet<u64>,
}

impl WhpOracle {
    /// Linear addresses below this are usable; above it there is no RAM.
    pub const ADDRESS_LIMIT: u64 = CTRL_BASE;
    /// Page tables, GDT/IDT and the fault stubs live here — do not use this
    /// region as data.
    pub const RESERVED_START: u64 = CTRL_BASE;

    /// Whether this host can be used as a WHP oracle at all: `winhvplatform.dll`
    /// present *and* the hypervisor actually running. Check it before
    /// [`new`](Self::new) in code that must not panic.
    pub fn is_available() -> bool {
        let Some(api) = whp() else { return false };
        hypervisor_present(api)
    }

    /// A fresh partition in the reset state described at the crate level.
    ///
    /// # Panics
    /// If WHP is unavailable — usually because the optional feature is off. Use
    /// [`try_new`](Self::try_new) to handle that.
    pub fn new() -> Self {
        Self::try_new().expect(
            "the Windows Hypervisor Platform is unavailable: enable it with \
             `dism /Online /Enable-Feature /FeatureName:HypervisorPlatform /All` \
             (it requires Hyper-V and hardware virtualization), then reboot",
        )
    }

    pub fn try_new() -> std::io::Result<Self> {
        let _g = lock();
        let api = whp().ok_or_else(|| {
            std::io::Error::other("winhvplatform.dll could not be loaded (no WHP on this system)")
        })?;
        if !hypervisor_present(api) {
            return Err(std::io::Error::other(
                "WHP reports no hypervisor present: enable the \"Windows Hypervisor Platform\" \
                 optional feature and hardware virtualization in firmware",
            ));
        }

        let mut handle: WHV_PARTITION_HANDLE = 0;
        // SAFETY: `handle` is a live out-parameter of the documented type.
        check("WHvCreatePartition", unsafe { (api.create_partition)(&mut handle) })?;
        // From here on, `partition` owns the handle: every early return cleans up.
        let partition = Partition(handle);

        set_property(api, handle, WHvPartitionPropertyCodeProcessorCount, &1u32)?;

        // Show the guest everything the host has. WHP's defaults are already
        // host-derived, so this is belt and braces — and harmless if refused.
        for (cap, prop) in [
            (WHvCapabilityCodeProcessorFeatures, WHvPartitionPropertyCodeProcessorFeatures),
            (
                WHvCapabilityCodeProcessorXsaveFeatures,
                WHvPartitionPropertyCodeProcessorXsaveFeatures,
            ),
        ] {
            if let Some(v) = capability_u64(api, cap) {
                let _ = set_property(api, handle, prop, &v);
            }
        }

        let mut exception_intercept = enable_exception_exits(api, handle);

        // SAFETY: our own partition, set up once (a failed attempt leaves it
        // unconfigured, not half-configured).
        let mut hr = unsafe { (api.setup_partition)(handle) };
        if !ok(hr) && exception_intercept {
            // The intercept configuration is the only thing here WHP might
            // accept as a property and then refuse at setup. Back it out and use
            // the IDT-stub path rather than losing the backend entirely.
            let _ = set_property(api, handle, WHvPartitionPropertyCodeExceptionExitBitmap, &0u64);
            let _ = set_property(api, handle, WHvPartitionPropertyCodeExtendedVmExits, &0u64);
            exception_intercept = false;
            // SAFETY: as above.
            hr = unsafe { (api.setup_partition)(handle) };
        }
        check("WHvSetupPartition", hr)?;

        let ram = GuestRam::new(RAM_SIZE as usize)?;
        // SAFETY: vCPU 0 of our own partition, created once.
        check("WHvCreateVirtualProcessor", unsafe { (api.create_vp)(handle, 0, 0) })?;

        let mut oracle = WhpOracle {
            partition,
            ram,
            // Placeholder: the real layout can only be probed once XCR0 is set
            // and an image has been read back, which enter_long_mode does.
            xsave: XsaveLayout::probe(&hw::host_cpuid, 0, 0),
            xsave_capacity: 0,
            exception_intercept,
            last_fault: None,
            touched: HashSet::new(),
        };
        // Claim the mapping here rather than waiting for the first step, so that a
        // host which cannot map guest RAM at all reports it from `try_new`, where
        // there is still an error to return it in.
        oracle.claim_mapping()?;
        hw::build_control_structures(&mut oracle.ram);
        oracle.enter_long_mode()?;
        Ok(oracle)
    }

    /// Make this partition the holder of the process's single guest-RAM mapping
    /// (see [`MAPPING_OWNER`]). A no-op once it already is, which is every call
    /// but the first unless another oracle has run in between.
    fn claim_mapping(&self) -> std::io::Result<()> {
        let api = whp().expect("WHP was available when this oracle was built");
        let mut owner = mapping_owner();
        if *owner == Some(self.handle()) {
            return Ok(());
        }
        if let Some(previous) = *owner {
            // SAFETY: a live partition of ours, whose range we mapped ourselves.
            check("WHvUnmapGpaRange", unsafe {
                (api.unmap_gpa_range)(previous, 0, RAM_SIZE)
            })?;
            *owner = None;
        }
        // SAFETY: the range describes our own live, committed allocation, and
        // `ram` outlives the partition (drop order in the struct).
        check("WHvMapGpaRange", unsafe {
            (api.map_gpa_range)(
                self.handle(),
                self.ram.host_ptr().cast(),
                0,
                RAM_SIZE,
                WHvMapGpaRangeFlagRead | WHvMapGpaRangeFlagWrite | WHvMapGpaRangeFlagExecute,
            )
        })?;
        *owner = Some(self.handle());
        Ok(())
    }

    /// Widest vector register the host CPU exposes to the guest, in bits. On a
    /// host without AVX-512 this is 256 (or 128 without AVX): `get_zmm` chunks
    /// above that read as zero, and AVX-512 encodings raise #UD — which is the
    /// *correct* ground truth for this machine, not a model bug. Skip such cases
    /// when diffing against a backend that implements them.
    pub fn host_vector_width(&self) -> usize {
        self.xsave.vector_width()
    }

    /// Whether faults are being observed through exception intercepts (the
    /// preferred path) rather than the IDT stubs. Diagnostic only: both mean the
    /// same thing to callers.
    pub fn uses_exception_intercepts(&self) -> bool {
        self.exception_intercept
    }

    /// The exception vector of the last fault (6 = #UD, 0 = #DE, ...).
    pub fn last_vector(&self) -> Option<u8> {
        self.last_fault.map(|(v, _)| v)
    }

    /// The error code the CPU pushed with the last fault, if that vector has one.
    pub fn last_error_code(&self) -> Option<u64> {
        self.last_fault.and_then(|(v, e)| hw::has_error_code(v).then_some(e))
    }

    /// Diagnostic dump of the control state the partition actually holds (WHP may
    /// reject or normalize parts of what we set).
    pub fn debug_state(&self) -> String {
        let cs = self.get_seg(WHvX64RegisterCs);
        let tr = self.get_seg(WHvX64RegisterTr);
        format!(
            "cr0={:#x} cr3={:#x} cr4={:#x} efer={:#x} xcr0={:#x} \
             cs{{sel:{:#x} base:{:#x} attr:{:#x}}} tr{{sel:{:#x} base:{:#x} attr:{:#x}}} \
             rip={:#x} rflags={:#x} intercepts={} vector_width={}",
            self.get_reg64(WHvX64RegisterCr0),
            self.get_reg64(WHvX64RegisterCr3),
            self.get_reg64(WHvX64RegisterCr4),
            self.get_reg64(WHvX64RegisterEfer),
            self.get_reg64(WHvX64RegisterXCr0),
            cs.Selector,
            cs.Base,
            // SAFETY: `Attributes` is the union's plain-integer view.
            unsafe { cs.Anonymous.Attributes },
            tr.Selector,
            tr.Base,
            // SAFETY: as above.
            unsafe { tr.Anonymous.Attributes },
            self.get_reg64(WHvX64RegisterRip),
            self.get_reg64(WHvX64RegisterRflags),
            self.exception_intercept,
            self.xsave.vector_width(),
        )
    }

    // -- register plumbing. None of these take the lock: the trait methods do,
    //    and std's Mutex is not reentrant.

    fn handle(&self) -> WHV_PARTITION_HANDLE {
        self.partition.0
    }

    fn get_regs(&self, names: &[WHV_REGISTER_NAME]) -> Vec<RegValue> {
        // SAFETY: WHV_REGISTER_VALUE is a POD union, so all-zero is a valid
        // starting value.
        let zero = Aligned16(unsafe { std::mem::zeroed::<WHV_REGISTER_VALUE>() });
        let mut values = vec![zero; names.len()];
        let api = whp().expect("WHP was available when this oracle was built");
        // SAFETY: both arrays have `names.len()` elements, as declared.
        let hr = unsafe {
            (api.get_registers)(
                self.handle(),
                0,
                names.as_ptr(),
                names.len() as u32,
                values.as_mut_ptr().cast(),
            )
        };
        debug_assert!(ok(hr), "WHvGetVirtualProcessorRegisters failed: hr={hr:#010x}");
        values
    }

    fn set_regs(&self, names: &[WHV_REGISTER_NAME], values: &[RegValue]) -> HRESULT {
        debug_assert_eq!(names.len(), values.len());
        let api = whp().expect("WHP was available when this oracle was built");
        // SAFETY: both arrays have `names.len()` elements, as declared.
        unsafe {
            (api.set_registers)(
                self.handle(),
                0,
                names.as_ptr(),
                names.len() as u32,
                values.as_ptr().cast(),
            )
        }
    }

    fn get_reg64(&self, name: WHV_REGISTER_NAME) -> u64 {
        // SAFETY: a 64-bit register's value is the union's `Reg64` view.
        unsafe { self.get_regs(&[name])[0].0.Reg64 }
    }

    fn set_reg64(&self, name: WHV_REGISTER_NAME, value: u64) -> HRESULT {
        self.set_regs(&[name], &[reg64(value)])
    }

    fn get_seg(&self, name: WHV_REGISTER_NAME) -> WHV_X64_SEGMENT_REGISTER {
        // SAFETY: a segment register's value is the union's `Segment` view.
        unsafe { self.get_regs(&[name])[0].0.Segment }
    }

    fn set_seg(&self, name: WHV_REGISTER_NAME, seg: WHV_X64_SEGMENT_REGISTER) -> HRESULT {
        self.set_regs(&[name], &[seg_value(seg)])
    }

    fn modify_seg(&self, name: WHV_REGISTER_NAME, f: impl FnOnce(&mut WHV_X64_SEGMENT_REGISTER)) {
        let mut seg = self.get_seg(name);
        f(&mut seg);
        let _ = self.set_seg(name, seg);
    }

    /// The `WHvX64Register*` name for a portable `SEG_*` index.
    fn seg_name(n: u32) -> WHV_REGISTER_NAME {
        match n {
            SEG_ES => WHvX64RegisterEs,
            SEG_CS => WHvX64RegisterCs,
            SEG_SS => WHvX64RegisterSs,
            SEG_DS => WHvX64RegisterDs,
            SEG_FS => WHvX64RegisterFs,
            _ => WHvX64RegisterGs,
        }
    }

    /// CPUID as the *guest* sees it, which is what decides whether an instruction
    /// exists at all. Falls back to the host's CPUID on the older WHP that has no
    /// such query.
    fn cpuid(&self, leaf: u32, subleaf: u32) -> CpuidRegs {
        if let Some(api) = whp() {
            if let Some(f) = api.cpuid_output {
                let mut out = WHV_CPUID_OUTPUT { Eax: 0, Ebx: 0, Ecx: 0, Edx: 0 };
                // SAFETY: `out` is a live out-parameter of the documented type.
                if ok(unsafe { f(self.handle(), 0, leaf, subleaf, &mut out) }) {
                    return CpuidRegs { eax: out.Eax, ebx: out.Ebx, ecx: out.Ecx, edx: out.Edx };
                }
            }
        }
        hw::host_cpuid(leaf, subleaf)
    }

    /// Put the vCPU in 64-bit long mode, CPL 0, with the flat segments and the
    /// tables `hw::build_control_structures` laid down.
    fn enter_long_mode(&mut self) -> std::io::Result<()> {
        // Walk to long mode the way the architecture does, one consistent state
        // at a time: PAE and CR3 first, then LME, then paging, and only then
        // LMA. A register-set interface may validate each write, and every
        // intermediate state here is one real hardware passes through.
        check("set CR4", self.set_reg64(WHvX64RegisterCr4, CR4_INIT))?;
        check("set CR3", self.set_reg64(WHvX64RegisterCr3, PML4_ADDR))?;
        check("set EFER.LME", self.set_reg64(WHvX64RegisterEfer, EFER_INIT & !(1 << 10)))?;
        check("set CR0.PG", self.set_reg64(WHvX64RegisterCr0, CR0_INIT))?;
        check("set EFER.LMA", self.set_reg64(WHvX64RegisterEfer, EFER_INIT))?;

        let code = segment(0, 0xFFFF_FFFF, SEL_CODE64, ATTR_CODE64);
        let data = segment(0, 0xFFFF_FFFF, SEL_DATA, ATTR_DATA);
        for name in [WHvX64RegisterDs, WHvX64RegisterEs, WHvX64RegisterSs, WHvX64RegisterFs,
                     WHvX64RegisterGs] {
            check("set data segment", self.set_seg(name, data))?;
        }
        check("set CS", self.set_seg(WHvX64RegisterCs, code))?;
        // TR must point at the TSS holding IST1, and be marked busy; LDTR is left
        // not-present, which is how this interface spells "unusable".
        check(
            "set TR",
            self.set_seg(
                WHvX64RegisterTr,
                segment(TSS_ADDR, TSS_LIMIT, SEL_TSS, ATTR_TSS64),
            ),
        )?;
        check("set LDTR", self.set_seg(WHvX64RegisterLdtr, segment(0, 0, 0, 0)))?;
        check(
            "set GDTR",
            self.set_regs(&[WHvX64RegisterGdtr], &[table_value(table(GDT_ADDR, GDT_LIMIT))]),
        )?;
        check(
            "set IDTR",
            self.set_regs(&[WHvX64RegisterIdtr], &[table_value(table(IDT_ADDR, IDT_LIMIT))]),
        )?;

        let xcr0 = self.enable_widest_xcr0();

        // Everything the program can see starts at zero, RFLAGS at its reset
        // value. WHP resets a vCPU to real-mode power-on state (RIP 0xFFF0 and a
        // CPU signature in RDX), which would show up as a bogus starting state.
        let mut names = GPR_NAMES.to_vec();
        names.extend([WHvX64RegisterRip, WHvX64RegisterRflags]);
        let mut values = vec![reg64(0); GPR_NAMES.len() + 1];
        values.push(reg64(0x2)); // RFLAGS: bit 1 reads as 1
        check("zero registers", self.set_regs(&names, &values))?;

        // Vector state starts zeroed like the other backends. The image's own
        // XCOMP_BV says which of the two XSAVE layouts WHP produced.
        self.xsave_capacity = {
            let leaf = self.cpuid(0x0D, 0);
            (leaf.ecx as usize).max(4096)
        };
        let mut image = self.read_xsave()?;
        self.xsave =
            XsaveLayout::probe(&|l, s| self.cpuid(l, s), xcr0, hw::xsave_xcomp_bv(image.as_bytes()));
        hw::xsave_reset_vectors(image.as_bytes_mut(), &self.xsave);
        self.write_xsave(&image)
    }

    /// Enable as much of XCR0 as the vCPU accepts, widest first, and report what
    /// stuck. A component WHP refuses simply is not there, which the trait's
    /// [`vector_chunks`](X86Oracle::vector_chunks) contract already covers.
    fn enable_widest_xcr0(&self) -> u64 {
        let leaf = self.cpuid(0x0D, 0);
        let host = ((leaf.edx as u64) << 32) | leaf.eax as u64;
        let candidates = [
            host,
            host & !XCR0_OPTIONAL,
            XCR0_X87_SSE | XCR0_AVX | (host & XCR0_AVX512),
            XCR0_X87_SSE | (host & XCR0_AVX),
            XCR0_X87_SSE,
        ];
        for candidate in candidates {
            // X87 and SSE are not optional, and AVX-512's three components only
            // make sense together with AVX.
            let mut want = candidate | XCR0_X87_SSE;
            if want & XCR0_AVX512 != XCR0_AVX512 || want & XCR0_AVX == 0 {
                want &= !XCR0_AVX512;
            }
            if ok(self.set_reg64(WHvX64RegisterXCr0, want)) {
                // WHP may normalize the value, so believe the read-back — but
                // only if it is a state the guest can actually run in. An XCR0
                // without X87 and SSE would fail VM entry instead of failing
                // here, which is a much harder failure to read.
                let effective = self.get_reg64(WHvX64RegisterXCr0);
                if effective & XCR0_X87_SSE == XCR0_X87_SSE {
                    return effective;
                }
            }
        }
        XCR0_X87_SSE
    }

    fn read_xsave(&self) -> std::io::Result<XsaveImage> {
        let api = whp().expect("WHP was available when this oracle was built");
        let mut image = XsaveImage::new(self.xsave_capacity);
        let mut written = 0u32;
        // SAFETY: the buffer is `capacity()` bytes, as declared.
        let hr = unsafe {
            (api.get_xsave)(
                self.handle(),
                0,
                image.blocks.as_mut_ptr().cast(),
                image.capacity() as u32,
                &mut written,
            )
        };
        check("WHvGetVirtualProcessorXsaveState", hr)?;
        image.len = (written as usize).min(image.capacity());
        if image.len < 576 {
            return Err(std::io::Error::other(format!(
                "WHP returned a {}-byte XSAVE image, too short to contain one",
                image.len
            )));
        }
        Ok(image)
    }

    fn write_xsave(&self, image: &XsaveImage) -> std::io::Result<()> {
        let api = whp().expect("WHP was available when this oracle was built");
        // SAFETY: the buffer is `image.len` bytes, as declared, and holds an
        // image WHP itself produced with only vector-state bytes changed.
        let hr = unsafe {
            (api.set_xsave)(self.handle(), 0, image.blocks.as_ptr().cast(), image.len as u32)
        };
        check("WHvSetVirtualProcessorXsaveState", hr)
    }

    /// Set RFLAGS.TF so exactly one instruction runs.
    ///
    /// Must happen immediately before each run, not once at startup: TF *is* the
    /// single-step mechanism, so every later `set_rip`/`set_gpr` — each of which
    /// rewrites RFLAGS — would clear it and the guest would run away.
    fn arm_singlestep(&self) -> HRESULT {
        let rflags = self.get_reg64(WHvX64RegisterRflags);
        self.set_reg64(WHvX64RegisterRflags, (rflags | FLAG_TF) & !FLAG_RF)
    }

    fn disarm_singlestep(&self) {
        let rflags = self.get_reg64(WHvX64RegisterRflags);
        let _ = self.set_reg64(WHvX64RegisterRflags, rflags & !(FLAG_TF | FLAG_RF));
    }

    /// Undo the hardware's exception vectoring, for the path where the fault was
    /// delivered through the IDT rather than intercepted: RIP back on the
    /// faulting instruction, RSP back to its pre-fault value.
    fn unwind_fault(&self, vector: u8) -> u64 {
        let handler_rsp = self.get_reg64(WHvX64RegisterRsp);
        let frame = hw::unwind_frame(&self.ram, handler_rsp, vector);
        let _ = self.set_regs(
            &[WHvX64RegisterRip, WHvX64RegisterRsp, WHvX64RegisterRflags],
            &[
                reg64(frame.rip),
                reg64(frame.rsp),
                reg64(frame.rflags & !(FLAG_TF | FLAG_RF)),
            ],
        );
        frame.error_code
    }

    fn fault(&mut self, vector: u8, error_code: u64) -> StepOutcome {
        self.last_fault = Some((vector, error_code));
        StepOutcome::Fault { kind: hw::classify(vector), msg: hw::fault_message(vector, error_code) }
    }
}

/// GPRs in the portable ModRM order, so index N of this table is register N.
const GPR_NAMES: [WHV_REGISTER_NAME; 16] = [
    WHvX64RegisterRax,
    WHvX64RegisterRcx,
    WHvX64RegisterRdx,
    WHvX64RegisterRbx,
    WHvX64RegisterRsp,
    WHvX64RegisterRbp,
    WHvX64RegisterRsi,
    WHvX64RegisterRdi,
    WHvX64RegisterR8,
    WHvX64RegisterR9,
    WHvX64RegisterR10,
    WHvX64RegisterR11,
    WHvX64RegisterR12,
    WHvX64RegisterR13,
    WHvX64RegisterR14,
    WHvX64RegisterR15,
];

fn segment(base: u64, limit: u32, selector: u16, attributes: u16) -> WHV_X64_SEGMENT_REGISTER {
    WHV_X64_SEGMENT_REGISTER {
        Base: base,
        Limit: limit,
        Selector: selector,
        Anonymous: WHV_X64_SEGMENT_REGISTER_0 { Attributes: attributes },
    }
}

fn table(base: u64, limit: u16) -> WHV_X64_TABLE_REGISTER {
    WHV_X64_TABLE_REGISTER { Pad: [0; 3], Limit: limit, Base: base }
}

fn hypervisor_present(api: &Whp) -> bool {
    let mut present: i32 = 0;
    let mut written = 0u32;
    // SAFETY: the buffer is exactly the 4 bytes (a BOOL) we declare.
    let hr = unsafe {
        (api.get_capability)(
            WHvCapabilityCodeHypervisorPresent,
            (&mut present as *mut i32).cast(),
            size_of::<i32>() as u32,
            &mut written,
        )
    };
    ok(hr) && present != 0
}

fn set_property<T>(
    api: &Whp,
    handle: WHV_PARTITION_HANDLE,
    code: WHV_PARTITION_PROPERTY_CODE,
    value: &T,
) -> std::io::Result<()> {
    // SAFETY: the buffer is exactly `size_of::<T>()` bytes, and T is the plain
    // integer type WHP documents for this property code.
    let hr = unsafe {
        (api.set_property)(handle, code, (value as *const T).cast(), size_of::<T>() as u32)
    };
    check("WHvSetPartitionProperty", hr)
}

fn get_property_u64(
    api: &Whp,
    handle: WHV_PARTITION_HANDLE,
    code: WHV_PARTITION_PROPERTY_CODE,
) -> Option<u64> {
    let mut value = 0u64;
    let mut written = 0u32;
    // SAFETY: the buffer is exactly the 8 bytes we declare.
    let hr = unsafe {
        (api.get_property)(
            handle,
            code,
            (&mut value as *mut u64).cast(),
            size_of::<u64>() as u32,
            &mut written,
        )
    };
    (ok(hr) && written as usize == size_of::<u64>()).then_some(value)
}

/// Try to arrange for exceptions to exit *before* the CPU delivers them, and
/// report whether it worked. On failure the IDT-stub path in [`crate::hw`] does
/// the same job unaided, so this is an optimization, not a requirement — which is
/// why every step here degrades rather than errors.
fn enable_exception_exits(api: &Whp, handle: WHV_PARTITION_HANDLE) -> bool {
    // Without a #DB intercept there is no point: that is how a completed step
    // reports itself, and the IDT path would be needed anyway.
    let bitmap = WANTED_EXCEPTIONS & capability_u64(api, WHvCapabilityCodeExceptionExitBitmap).unwrap_or(0);
    if bitmap & (1 << 1) == 0 {
        return false;
    }
    if capability_u64(api, WHvCapabilityCodeExtendedVmExits).unwrap_or(0) & EXTENDED_EXIT_EXCEPTION == 0 {
        return false;
    }
    if set_property(api, handle, WHvPartitionPropertyCodeExtendedVmExits, &EXTENDED_EXIT_EXCEPTION).is_err() {
        return false;
    }
    if set_property(api, handle, WHvPartitionPropertyCodeExceptionExitBitmap, &bitmap).is_err() {
        return false;
    }
    // Confirm it stuck rather than assuming: a silently ignored bitmap would
    // turn every fault into a triple fault.
    get_property_u64(api, handle, WHvPartitionPropertyCodeExceptionExitBitmap) == Some(bitmap)
}

impl Default for WhpOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl X86Oracle for WhpOracle {
    fn backend_name(&self) -> &'static str {
        "whp"
    }

    /// The host's widest vector register, in 64-bit chunks. On a CPU without
    /// AVX-512 this is 4 (YMM); the remaining chunks read as zero.
    fn vector_chunks(&self) -> usize {
        self.xsave.vector_width() / 64
    }

    fn vector_regs(&self) -> usize {
        self.xsave.vector_regs()
    }

    fn set_gpr(&mut self, n: u32, value: u64) {
        let _g = lock();
        let _ = self.set_reg64(GPR_NAMES[(n as usize).min(15)], value);
    }

    fn get_gpr(&self, n: u32) -> u64 {
        let _g = lock();
        self.get_reg64(GPR_NAMES[(n as usize).min(15)])
    }

    fn set_rip(&mut self, value: u64) {
        let _g = lock();
        let _ = self.set_reg64(WHvX64RegisterRip, value);
    }

    fn get_rip(&self) -> u64 {
        let _g = lock();
        self.get_reg64(WHvX64RegisterRip)
    }

    /// TF/RF are masked out: single-stepping is implemented with the trap flag,
    /// so the guest's raw RFLAGS carries state that belongs to the harness, not
    /// to the program under test.
    fn set_rflags(&mut self, value: u64) {
        let _g = lock();
        let _ = self.set_reg64(WHvX64RegisterRflags, (value & !(FLAG_TF | FLAG_RF)) | 0x2);
    }

    fn get_rflags(&self) -> u64 {
        let _g = lock();
        self.get_reg64(WHvX64RegisterRflags) & !(FLAG_TF | FLAG_RF)
    }

    /// Chunks the host cannot hold are dropped (see
    /// [`host_vector_width`](Self::host_vector_width)).
    fn set_zmm(&mut self, n: u32, data: &[u64]) {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = lock();
        let Ok(mut image) = self.read_xsave() else { return };
        let layout = self.xsave;
        hw::xsave_set_zmm(image.as_bytes_mut(), &layout, n, data);
        let _ = self.write_xsave(&image);
    }

    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS] {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = lock();
        match self.read_xsave() {
            Ok(image) => hw::xsave_get_zmm(image.as_bytes(), &self.xsave, n),
            Err(_) => [0; ZMM_CHUNKS],
        }
    }

    fn set_msr(&mut self, msr: u32, value: u64) {
        let _g = lock();
        // WHP has no FS_BASE/GS_BASE MSR registers: those bases live in the
        // segment registers, which is also where set_seg_base puts them, so the
        // two APIs stay in sync for free.
        match msr {
            IA32_EFER => drop(self.set_reg64(WHvX64RegisterEfer, value)),
            IA32_FS_BASE => self.modify_seg(WHvX64RegisterFs, |s| s.Base = value),
            IA32_GS_BASE => self.modify_seg(WHvX64RegisterGs, |s| s.Base = value),
            IA32_KERNEL_GS_BASE => drop(self.set_reg64(WHvX64RegisterKernelGsBase, value)),
            IA32_STAR => drop(self.set_reg64(WHvX64RegisterStar, value)),
            IA32_LSTAR => drop(self.set_reg64(WHvX64RegisterLstar, value)),
            IA32_FMASK => drop(self.set_reg64(WHvX64RegisterSfmask, value)),
            _ => {} // not modelled: ignored, per the trait contract
        }
    }

    fn get_msr(&self, msr: u32) -> u64 {
        let _g = lock();
        match msr {
            IA32_EFER => self.get_reg64(WHvX64RegisterEfer),
            IA32_FS_BASE => self.get_seg(WHvX64RegisterFs).Base,
            IA32_GS_BASE => self.get_seg(WHvX64RegisterGs).Base,
            IA32_KERNEL_GS_BASE => self.get_reg64(WHvX64RegisterKernelGsBase),
            IA32_STAR => self.get_reg64(WHvX64RegisterStar),
            IA32_LSTAR => self.get_reg64(WHvX64RegisterLstar),
            IA32_FMASK => self.get_reg64(WHvX64RegisterSfmask),
            _ => 0,
        }
    }

    fn set_cr(&mut self, n: u32, value: u64) {
        let _g = lock();
        let name = match n {
            0 => WHvX64RegisterCr0,
            2 => WHvX64RegisterCr2,
            3 => WHvX64RegisterCr3,
            4 => WHvX64RegisterCr4,
            8 => WHvX64RegisterCr8,
            _ => return,
        };
        let _ = self.set_reg64(name, value);
    }

    fn get_cr(&self, n: u32) -> u64 {
        let _g = lock();
        let name = match n {
            0 => WHvX64RegisterCr0,
            2 => WHvX64RegisterCr2,
            3 => WHvX64RegisterCr3,
            4 => WHvX64RegisterCr4,
            8 => WHvX64RegisterCr8,
            _ => return 0,
        };
        self.get_reg64(name)
    }

    fn set_seg_selector(&mut self, n: u32, value: u16) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        self.modify_seg(Self::seg_name(n), |s| s.Selector = value);
    }

    fn get_seg_selector(&self, n: u32) -> u16 {
        let _g = lock();
        self.get_seg(Self::seg_name(n)).Selector
    }

    fn set_seg_base(&mut self, n: u32, value: u64) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        self.modify_seg(Self::seg_name(n), |s| s.Base = value);
    }

    fn get_seg_base(&self, n: u32) -> u64 {
        let _g = lock();
        self.get_seg(Self::seg_name(n)).Base
    }

    fn set_seg_limit(&mut self, n: u32, value: u32) {
        let _g = lock();
        self.modify_seg(Self::seg_name(n), |s| s.Limit = value);
    }

    fn get_seg_limit(&self, n: u32) -> u32 {
        let _g = lock();
        self.get_seg(Self::seg_name(n)).Limit
    }

    fn read_mem_byte(&self, addr: u64) -> u8 {
        let mut b = [0u8; 1];
        self.ram.read(addr, &mut b);
        b[0]
    }

    fn write_mem_byte(&mut self, addr: u64, byte: u8) {
        self.touched.insert(addr & !0xFFF);
        self.ram.write(addr, &[byte]);
    }

    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        self.ram.read(addr, buf);
    }

    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        for page in (addr & !0xFFF)..=((addr + data.len() as u64) & !0xFFF) {
            self.touched.insert(page & !0xFFF);
        }
        self.ram.write(addr, data);
    }

    /// Only tracks writes made through this API — the guest's own stores are
    /// invisible to it, so this is the "hint" the trait describes.
    fn is_mapped(&self, addr: u64) -> bool {
        self.touched.contains(&(addr & !0xFFF))
    }

    fn step(&mut self) -> StepOutcome {
        let _g = lock();
        self.last_fault = None;

        // The guest cannot run without its RAM, and another oracle may have taken
        // the mapping since this one last ran.
        if let Err(e) = self.claim_mapping() {
            return StepOutcome::Fault {
                kind: FaultKind::OtherException,
                msg: format!("could not map guest RAM: {e}"),
            };
        }

        let hr = self.arm_singlestep();
        if !ok(hr) {
            return StepOutcome::Fault {
                kind: FaultKind::OtherException,
                msg: format!("could not arm single-stepping: hr={hr:#010x}"),
            };
        }
        let api = whp().expect("WHP was available when this oracle was built");

        // A cancelled run is not the guest's doing; retry a bounded number of
        // times.
        for _ in 0..64 {
            // SAFETY: `exit` is a live out-parameter, and the size we declare is
            // its own. Aligned16 because WHP fills it with the same aligned moves
            // it reads register values with.
            let mut staged = Aligned16(unsafe { std::mem::zeroed::<WHV_RUN_VP_EXIT_CONTEXT>() });
            let hr = unsafe {
                (api.run_vp)(
                    self.handle(),
                    0,
                    (&mut staged.0 as *mut WHV_RUN_VP_EXIT_CONTEXT).cast(),
                    size_of::<WHV_RUN_VP_EXIT_CONTEXT>() as u32,
                )
            };
            let exit = &staged.0;
            if !ok(hr) {
                return StepOutcome::Fault {
                    kind: FaultKind::OtherException,
                    msg: format!("WHvRunVirtualProcessor failed: hr={hr:#010x}"),
                };
            }

            match exit.ExitReason {
                // Intercepted before delivery: RIP, RSP and RFLAGS are still the
                // faulting instruction's, so there is nothing to unwind.
                EXIT_EXCEPTION_INTERCEPT => {
                    // SAFETY: the exit reason selects this union member.
                    let e = unsafe { exit.Anonymous.VpException };
                    self.disarm_singlestep();
                    if e.ExceptionType == 1 {
                        return StepOutcome::Retired; // the single-step trap
                    }
                    // SAFETY: `AsUINT32` is the union's plain-integer view; bit 0
                    // is ErrorCodeValid.
                    let has_code = unsafe { e.ExceptionInfo.AsUINT32 } & 1 != 0;
                    let error = if has_code { e.ErrorCode as u64 } else { 0 };
                    return self.fault(e.ExceptionType, error);
                }

                // Either a fault stub (identified by RIP) or the guest's own HLT.
                EXIT_HALT => match hw::stub_vector(exit.VpContext.Rip) {
                    Some(1) => {
                        // The step's own #DB, delivered rather than intercepted.
                        // Unwinding restores the post-instruction RIP.
                        self.unwind_fault(1);
                        return StepOutcome::Retired;
                    }
                    Some(vector) => {
                        let error = self.unwind_fault(vector);
                        return self.fault(vector, error);
                    }
                    None => {
                        // A real HLT: nothing can wake this oracle, so treat it
                        // as a retired instruction, like the other backends do.
                        self.disarm_singlestep();
                        return StepOutcome::Retired;
                    }
                },

                EXIT_CANCELED => continue,

                // No devices exist, so any unmapped-GPA access is off the end of
                // guest RAM.
                EXIT_MEMORY_ACCESS => {
                    // SAFETY: the exit reason selects this union member.
                    let gpa = unsafe { exit.Anonymous.MemoryAccess }.Gpa;
                    return StepOutcome::Fault {
                        kind: FaultKind::PageFault,
                        msg: format!("access outside guest RAM at {gpa:#x}"),
                    };
                }

                // A triple fault means the IDT itself is broken — a bug here, not
                // a property of the instruction.
                EXIT_TRIPLE_FAULT => {
                    return StepOutcome::Fault {
                        kind: FaultKind::OtherException,
                        msg: "guest triple fault (WHvRunVpExitReasonUnrecoverableException)"
                            .to_string(),
                    };
                }

                other => {
                    return StepOutcome::Fault {
                        kind: FaultKind::OtherException,
                        msg: format!("unexpected WHP exit reason {other:#x}"),
                    };
                }
            }
        }

        StepOutcome::Fault {
            kind: FaultKind::OtherException,
            msg: "WHvRunVirtualProcessor did not make progress".to_string(),
        }
    }

    fn fault_msg(&self) -> String {
        match self.last_fault {
            None => String::new(),
            Some((v, e)) => hw::fault_message(v, e),
        }
    }
}
