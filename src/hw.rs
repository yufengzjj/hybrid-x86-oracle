//! The guest environment shared by the two **hardware** backends.
//!
//! [`KvmOracle`](crate::KvmOracle) and [`WhpOracle`](crate::WhpOracle) differ
//! only in the hypervisor interface they drive; everything *inside* the guest —
//! the RAM mapping, the page tables, the descriptor tables, how a fault is made
//! observable, where the vector registers live in an XSAVE image — is one design
//! described once here. Two host drivers, one guest.
//!
//! # How one instruction is executed
//!
//! A minimal VM: one vCPU, one flat RAM slot, identity-mapped page tables so
//! linear == physical (matching the other backends' flat address space), a GDT
//! with a 64-bit code segment, and single-stepping via RFLAGS.TF.
//!
//! Faults are the interesting part. Real hardware *must* vector through an IDT,
//! so we install one whose 256 gates each point at a distinct one-byte `HLT`
//! stub. A fault therefore lands on `HLT`, which exits to userspace, and the
//! stub's address identifies the vector ([`stub_vector`]). The exception frame
//! the CPU pushed then yields the faulting RIP, the original RSP and the error
//! code ([`unwind_frame`]), which we use to *undo* the vectoring — restoring RIP
//! and RSP so the observable state matches the other backends' "faults do not
//! vector" contract.
//!
//! A hypervisor that can intercept exceptions *before* delivery (WHP can) gets
//! the same information with no unwinding at all; the IDT then only has to exist
//! so that anything unforeseen stops at a `HLT` instead of triple-faulting.

use crate::{FaultKind, ZMM_CHUNKS, ZMM_REGS};

// ---------------------------------------------------------------- guest layout

/// Guest RAM. Lazily populated by the host, so the virtual size costs little;
/// 256 MiB is far more than single-instruction tests need.
pub const RAM_SIZE: u64 = 256 << 20;

/// Control structures live in the top 4 MiB, out of the way of test addresses.
pub const CTRL_BASE: u64 = RAM_SIZE - (4 << 20);
/// Root page table — what CR3 points at.
pub const PML4_ADDR: u64 = CTRL_BASE;
const PDPT_ADDR: u64 = CTRL_BASE + 0x1000;
const PD_ADDR: u64 = CTRL_BASE + 0x2000;
// GDT/IDT addresses and the TSS selector are the crate-level descriptor-table
// contract (Bochs mirrors the register values); this assert pins the contract's
// literals to the layout actually built here, so neither can drift alone: the
// addresses to the control-region slots, the GDT limit to its last slot (the
// 16-byte TSS descriptor, written at offset SEL_TSS), and the IDT limit to its
// 256 16-byte gates.
pub use crate::{GDT_ADDR, GDT_LIMIT, IDT_ADDR, IDT_LIMIT, SEL_TSS};
const _: () = assert!(
    GDT_ADDR == CTRL_BASE + 0x3000
        && IDT_ADDR == CTRL_BASE + 0x4000
        && GDT_LIMIT as u64 == SEL_TSS as u64 + 16 - 1
        && IDT_LIMIT as u64 == 256 * 16 - 1,
    "the crate contract must match the control-region layout"
);
/// 256 fault stubs, one per vector, `STUB_STRIDE` bytes apart.
const STUB_BASE: u64 = CTRL_BASE + 0x5000;
const STUB_STRIDE: u64 = 16;
const STUB_AREA: u64 = 256 * STUB_STRIDE;
/// 64-bit TSS. Its only job is to hold IST1 (below).
pub const TSS_ADDR: u64 = CTRL_BASE + 0x6000;
pub const TSS_LIMIT: u32 = 0x67;
/// Dedicated exception stack, referenced by IST1 and grown downwards from the
/// top. Every IDT gate uses it, so delivering a fault never touches the guest's
/// RSP — which is what lets a fault be observed from *any* state, including
/// RSP == 0, matching what the model backends can do.
const IST_STACK_TOP: u64 = CTRL_BASE + 0x8000;

pub const SEL_CODE64: u16 = 0x08;
pub const SEL_DATA: u16 = 0x10;

const PTE_P: u64 = 1 << 0;
const PTE_W: u64 = 1 << 1;
const PTE_PS: u64 = 1 << 7;

/// CR0: PE | MP | ET | NE | WP | AM | PG.
pub const CR0_INIT: u64 = 0x8005_0033;
/// CR4: PAE | OSFXSR | OSXMMEXCPT | OSXSAVE.
pub const CR4_INIT: u64 = (1 << 5) | (1 << 9) | (1 << 10) | (1 << 18);
/// EFER: SCE off, LME | LMA | NXE. LMA is stated explicitly because a
/// register-set interface has no `mov cr0` for the CPU to derive it from, and
/// VM entry rejects an inconsistent (CR0.PG, EFER.LME, EFER.LMA) triple.
pub const EFER_INIT: u64 = (1 << 8) | (1 << 10) | (1 << 11);

// ------------------------------------------------------------------- host RAM

/// The host mapping backing guest RAM. Owned so it is released on drop, after
/// the partition/VM whose memory region points here has been torn down.
pub struct GuestRam {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: the pointer is a private mapping owned solely by this struct; all
// access goes through &self/&mut self, and the VM referencing it is dropped
// first (field order in the oracle structs).
unsafe impl Send for GuestRam {}
unsafe impl Sync for GuestRam {}

impl GuestRam {
    pub fn new(len: usize) -> std::io::Result<Self> {
        Ok(GuestRam { ptr: host_alloc(len)?, len })
    }

    /// Host address of guest physical 0 — what the hypervisor's
    /// map-memory-region call wants.
    pub fn host_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn read(&self, addr: u64, buf: &mut [u8]) {
        let Some(end) = (addr as usize).checked_add(buf.len()) else {
            buf.fill(0);
            return;
        };
        if end > self.len {
            buf.fill(0); // outside RAM: reads as zero, like the other backends
            return;
        }
        // SAFETY: the range is inside the mapping, checked above.
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr.add(addr as usize), buf.as_mut_ptr(), buf.len())
        };
    }

    pub fn write(&mut self, addr: u64, data: &[u8]) {
        let Some(end) = (addr as usize).checked_add(data.len()) else {
            return;
        };
        if end > self.len {
            return; // outside RAM: dropped
        }
        // SAFETY: as above.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(addr as usize), data.len())
        };
    }

    pub fn write_u64(&mut self, addr: u64, value: u64) {
        self.write(addr, &value.to_le_bytes());
    }

    pub fn read_u64(&self, addr: u64) -> u64 {
        let mut b = [0u8; 8];
        self.read(addr, &mut b);
        u64::from_le_bytes(b)
    }
}

impl Drop for GuestRam {
    fn drop(&mut self) {
        // SAFETY: releasing our own mapping exactly once.
        unsafe { host_free(self.ptr, self.len) };
    }
}

#[cfg(unix)]
fn host_alloc(len: usize) -> std::io::Result<*mut u8> {
    // MAP_NORESERVE: the guest touches a few pages of a 256 MiB space.
    // SAFETY: a fresh anonymous mapping; the kernel validates the arguments.
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
            -1,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ptr.cast())
}

#[cfg(unix)]
unsafe fn host_free(ptr: *mut u8, len: usize) {
    libc::munmap(ptr.cast(), len);
}

#[cfg(windows)]
fn host_alloc(len: usize) -> std::io::Result<*mut u8> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
    };
    // WHvMapGpaRange requires the whole source range be committed, so unlike the
    // mmap path this cannot be reservation-only. Commit charge is not residency,
    // though: untouched pages still cost no physical memory.
    // SAFETY: a fresh private allocation; the kernel validates the arguments.
    let ptr =
        unsafe { VirtualAlloc(std::ptr::null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    if ptr.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(ptr.cast())
}

#[cfg(windows)]
unsafe fn host_free(ptr: *mut u8, _len: usize) {
    use windows_sys::Win32::System::Memory::{VirtualFree, MEM_RELEASE};
    // MEM_RELEASE frees the whole reservation and requires a size of 0.
    VirtualFree(ptr.cast(), 0, MEM_RELEASE);
}

// ---------------------------------------------------- guest control structures

/// Identity-mapped page tables, a flat GDT, a TSS holding IST1, and an IDT whose
/// 256 gates each point at their own `HLT` — that is how a fault becomes an
/// observable userspace exit carrying its vector.
pub fn build_control_structures(ram: &mut GuestRam) {
    // 4-level paging, 2 MiB leaves: PML4[0] -> PDPT[0] -> PD[0..128).
    ram.write_u64(PML4_ADDR, PDPT_ADDR | PTE_P | PTE_W);
    ram.write_u64(PDPT_ADDR, PD_ADDR | PTE_P | PTE_W);
    for i in 0..(RAM_SIZE >> 21) {
        let frame = i << 21;
        ram.write_u64(PD_ADDR + i * 8, frame | PTE_P | PTE_W | PTE_PS);
    }

    // GDT: null, 64-bit code (L=1), flat data, then a 16-byte TSS descriptor at
    // selector 0x18. Interrupt delivery reloads CS from the gate's selector and
    // consults the TSS for IST, so these descriptors must really exist in memory.
    ram.write_u64(GDT_ADDR, 0);
    ram.write_u64(GDT_ADDR + 8, 0x00AF_9B00_0000_FFFF);
    ram.write_u64(GDT_ADDR + 16, 0x00CF_9300_0000_FFFF);
    // 64-bit TSS descriptor: base split across the two halves, limit 0x67,
    // type 9 (available 64-bit TSS), present.
    let tss_low = (TSS_LIMIT as u64 & 0xFFFF)
        | ((TSS_ADDR & 0xFFFF) << 16)
        | (((TSS_ADDR >> 16) & 0xFF) << 32)
        | (0x89 << 40) // present, type 9
        | (((TSS_ADDR >> 24) & 0xFF) << 56);
    ram.write_u64(GDT_ADDR + SEL_TSS as u64, tss_low);
    ram.write_u64(GDT_ADDR + SEL_TSS as u64 + 8, TSS_ADDR >> 32);

    // The TSS itself: everything zero except IST1.
    let mut tss = [0u8; 0x68];
    tss[0x24..0x2C].copy_from_slice(&IST_STACK_TOP.to_le_bytes()); // IST1
    ram.write(TSS_ADDR, &tss);

    // IDT: 64-bit interrupt gates, one per vector, targeting its stub.
    for vector in 0..256u64 {
        let target = STUB_BASE + vector * STUB_STRIDE;
        let low = (target & 0xFFFF)
            | ((SEL_CODE64 as u64) << 16)
            // IST 1: deliver on the dedicated stack, never the guest's.
            | (1 << 32)
            // type_attr 0x8E: present, DPL 0, 64-bit interrupt gate.
            | (0x8E << 40)
            | (((target >> 16) & 0xFFFF) << 48);
        ram.write_u64(IDT_ADDR + vector * 16, low);
        ram.write_u64(IDT_ADDR + vector * 16 + 8, target >> 32);
    }

    // The stubs themselves: HLT, which leaves the guest immediately. Interrupt
    // gates clear RFLAGS.TF, so single-stepping does not fire before the HLT
    // runs.
    let mut stubs = vec![0u8; STUB_AREA as usize];
    for vector in 0..256usize {
        stubs[vector * STUB_STRIDE as usize] = 0xF4; // HLT
    }
    ram.write(STUB_BASE, &stubs);
}

// ---------------------------------------------------------------------- faults

/// Vectors that push an error code (Intel SDM Vol. 3, Table 6-1).
pub fn has_error_code(vector: u8) -> bool {
    matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17 | 21)
}

pub fn classify(vector: u8) -> FaultKind {
    match vector {
        0 => FaultKind::DivideError,
        6 => FaultKind::UndefinedOpcode,
        13 => FaultKind::GeneralProtection,
        14 => FaultKind::PageFault,
        17 => FaultKind::AlignmentCheck,
        21 => FaultKind::ControlProtection,
        _ => FaultKind::OtherException,
    }
}

fn vector_name(vector: u8) -> &'static str {
    match vector {
        0 => "#DE divide error",
        1 => "#DB debug",
        3 => "#BP breakpoint",
        4 => "#OF overflow",
        5 => "#BR bound range",
        6 => "#UD invalid opcode",
        7 => "#NM device not available",
        8 => "#DF double fault",
        10 => "#TS invalid TSS",
        11 => "#NP segment not present",
        12 => "#SS stack fault",
        13 => "#GP general protection",
        14 => "#PF page fault",
        16 => "#MF x87 FP",
        17 => "#AC alignment check",
        18 => "#MC machine check",
        19 => "#XM SIMD FP",
        21 => "#CP control protection",
        _ => "exception",
    }
}

pub fn fault_message(vector: u8, error_code: u64) -> String {
    if has_error_code(vector) {
        format!("{} (vector {vector}, error code {error_code:#x})", vector_name(vector))
    } else {
        format!("{} (vector {vector})", vector_name(vector))
    }
}

/// Which vector's stub a `HLT` exit landed in, given the RIP *after* the
/// one-byte `HLT`. `None` means the guest executed a `HLT` of its own.
pub fn stub_vector(halt_rip: u64) -> Option<u8> {
    (halt_rip > STUB_BASE && halt_rip <= STUB_BASE + STUB_AREA)
        .then(|| ((halt_rip - 1 - STUB_BASE) / STUB_STRIDE) as u8)
}

/// The state a faulting instruction left behind, recovered from the frame the
/// CPU pushed when it vectored.
pub struct Unwound {
    pub rip: u64,
    pub rsp: u64,
    pub rflags: u64,
    pub error_code: u64,
}

/// Read the exception frame at `handler_rsp` (the RSP a fault stub sees) and
/// report the pre-fault RIP/RSP/RFLAGS, so the caller can undo the vectoring.
///
/// Long mode always pushes SS:RSP, so the frame is:
///   \[rsp\] error?, RIP, CS, RFLAGS, RSP, SS
pub fn unwind_frame(ram: &GuestRam, handler_rsp: u64, vector: u8) -> Unwound {
    let (error_code, base) = if has_error_code(vector) {
        (ram.read_u64(handler_rsp), handler_rsp + 8)
    } else {
        (0, handler_rsp)
    };
    Unwound {
        rip: ram.read_u64(base),
        // RFLAGS as the faulting instruction left it (the gate cleared TF/IF).
        rflags: ram.read_u64(base + 16),
        rsp: ram.read_u64(base + 24),
        error_code,
    }
}

// ------------------------------------------------------------- XSAVE component

/// The four CPUID result registers, so the XSAVE layout can be probed through
/// whichever CPUID the *guest* sees — a hypervisor may show it less than the
/// host has.
#[derive(Debug, Clone, Copy)]
pub struct CpuidRegs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

pub fn host_cpuid(leaf: u32, subleaf: u32) -> CpuidRegs {
    let r = std::arch::x86_64::__cpuid_count(leaf, subleaf);
    CpuidRegs { eax: r.eax, ebx: r.ebx, ecx: r.ecx, edx: r.edx }
}

// XSAVE area offsets fixed by the architecture (legacy FXSAVE image).
const XSAVE_MXCSR: usize = 24;
const XSAVE_XMM_OFFSET: usize = 160; // XMM0-15, 16 bytes each
const XSAVE_XSTATE_BV: usize = 512; // 8 bytes
const XSAVE_XCOMP_BV: usize = 520; // 8 bytes
/// Where the extended region starts in the *compacted* format.
const XSAVE_COMPACT_BASE: usize = 576;

/// XCOMP_BV of an XSAVE image: which components it has room for, and — in bit 63
/// — which of the two layouts it uses.
pub fn xsave_xcomp_bv(bytes: &[u8]) -> u64 {
    read_u64_at(bytes, XSAVE_XCOMP_BV)
}

/// One XSAVE state component's location within an XSAVE image.
#[derive(Debug, Clone, Copy)]
pub struct XsaveComponent {
    offset: usize,
    size: usize,
}

/// Where a vCPU keeps the vector state we expose. `None` means this component is
/// not available, so those `get_zmm` chunks read as zero and writes to them are
/// dropped.
#[derive(Debug, Clone, Copy)]
pub struct XsaveLayout {
    /// Component 2: YMM_Hi128 — upper halves of YMM0-15 (chunks 2..4).
    ymm_hi: Option<XsaveComponent>,
    /// Component 6: ZMM_Hi256 — upper halves of ZMM0-15 (chunks 4..8).
    zmm_hi: Option<XsaveComponent>,
    /// Component 7: Hi16_ZMM — the whole of ZMM16-31.
    zmm_hi16: Option<XsaveComponent>,
}

impl XsaveLayout {
    /// Locate the vector components in an XSAVE image.
    ///
    /// `xcr0` is the enabled-feature mask the vCPU actually accepted, and
    /// `xcomp_bv` is the XCOMP_BV field of an image the hypervisor produced.
    /// Both formats the architecture defines are handled: *standard*, where a
    /// component's offset is an absolute number from `CPUID.(EAX=0Dh):EBX` (what
    /// `KVM_GET_XSAVE` returns, with XCOMP_BV zero), and *compacted*, where
    /// bit 63 of XCOMP_BV is set and offsets have to be accumulated over the
    /// components actually present. Assuming the wrong one would silently read
    /// the wrong bytes rather than fail, so both are worth handling.
    pub fn probe(cpuid: &dyn Fn(u32, u32) -> CpuidRegs, xcr0: u64, xcomp_bv: u64) -> Self {
        let component = |index: u32| Self::locate(cpuid, xcr0, xcomp_bv, index);
        XsaveLayout { ymm_hi: component(2), zmm_hi: component(6), zmm_hi16: component(7) }
    }

    fn locate(
        cpuid: &dyn Fn(u32, u32) -> CpuidRegs,
        xcr0: u64,
        xcomp_bv: u64,
        index: u32,
    ) -> Option<XsaveComponent> {
        if xcr0 & (1u64 << index) == 0 {
            return None; // the vCPU cannot hold this component at all
        }
        let leaf = cpuid(0x0D, index);
        let size = leaf.eax as usize;
        if size == 0 {
            return None;
        }

        if xcomp_bv & (1u64 << 63) == 0 {
            // Standard format: an absolute offset, valid whatever XCR0 holds.
            let offset = leaf.ebx as usize;
            return (offset != 0).then_some(XsaveComponent { offset, size });
        }

        if xcomp_bv & (1u64 << index) == 0 {
            return None; // enabled, but absent from this image's format
        }
        // Compacted format: accumulate over the components present below this one.
        let mut offset = XSAVE_COMPACT_BASE;
        for i in 2..=index {
            if xcomp_bv & (1u64 << i) == 0 {
                continue;
            }
            let l = cpuid(0x0D, i);
            if l.eax == 0 {
                continue;
            }
            if l.ecx & 2 != 0 {
                offset = (offset + 63) & !63; // ECX[1]: this component is 64-byte aligned
            }
            if i == index {
                return Some(XsaveComponent { offset, size });
            }
            offset += l.eax as usize;
        }
        None
    }

    /// Widest vector register this vCPU can hold, in bits (128, 256 or 512).
    pub fn vector_width(&self) -> usize {
        if self.zmm_hi.is_some() {
            512
        } else if self.ymm_hi.is_some() {
            256
        } else {
            128
        }
    }

    /// ZMM16-31 exist only with AVX-512.
    pub fn vector_regs(&self) -> usize {
        if self.zmm_hi16.is_some() {
            ZMM_REGS
        } else {
            16
        }
    }
}

fn read_u64_at(bytes: &[u8], offset: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(b)
}

fn write_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// Zero the vector state of an XSAVE image and give MXCSR its reset value, so a
/// fresh hardware oracle starts where the model backends do.
pub fn xsave_reset_vectors(bytes: &mut [u8], layout: &XsaveLayout) {
    bytes[XSAVE_XMM_OFFSET..XSAVE_XMM_OFFSET + 16 * 16].fill(0);
    let len = bytes.len();
    for c in [layout.ymm_hi, layout.zmm_hi, layout.zmm_hi16].into_iter().flatten() {
        if c.offset + c.size <= len {
            bytes[c.offset..c.offset + c.size].fill(0);
        }
    }
    // MXCSR's reserved-bit pattern must stay valid.
    bytes[XSAVE_MXCSR..XSAVE_MXCSR + 4].copy_from_slice(&0x1F80u32.to_le_bytes());
}

/// Read ZMM`n` out of an XSAVE image. Chunks the vCPU cannot hold read as zero.
pub fn xsave_get_zmm(bytes: &[u8], layout: &XsaveLayout, n: u32) -> [u64; ZMM_CHUNKS] {
    let mut out = [0u64; ZMM_CHUNKS];
    let len = bytes.len();
    let fits = move |c: XsaveComponent| c.offset + c.size <= len;

    if n < 16 {
        let off = XSAVE_XMM_OFFSET + n as usize * 16;
        out[0] = read_u64_at(bytes, off);
        out[1] = read_u64_at(bytes, off + 8);
        if let Some(c) = layout.ymm_hi.filter(|&c| fits(c)) {
            let off = c.offset + n as usize * 16;
            out[2] = read_u64_at(bytes, off);
            out[3] = read_u64_at(bytes, off + 8);
        }
        if let Some(c) = layout.zmm_hi.filter(|&c| fits(c)) {
            let off = c.offset + n as usize * 32;
            for i in 0..4 {
                out[4 + i] = read_u64_at(bytes, off + i * 8);
            }
        }
    } else if let Some(c) = layout.zmm_hi16.filter(|&c| fits(c)) {
        let off = c.offset + (n as usize - 16) * 64;
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = read_u64_at(bytes, off + i * 8);
        }
    }
    out
}

/// Write ZMM`n` into an XSAVE image, marking the components it touches present
/// in XSTATE_BV. Chunks the vCPU cannot hold are dropped.
pub fn xsave_set_zmm(bytes: &mut [u8], layout: &XsaveLayout, n: u32, data: &[u64]) {
    let chunk = |i: usize| data.get(i).copied().unwrap_or(0);
    let mut xstate_bv = read_u64_at(bytes, XSAVE_XSTATE_BV);
    let len = bytes.len();
    let fits = move |c: XsaveComponent| c.offset + c.size <= len;

    if n < 16 {
        let off = XSAVE_XMM_OFFSET + n as usize * 16;
        write_u64_at(bytes, off, chunk(0));
        write_u64_at(bytes, off + 8, chunk(1));
        xstate_bv |= 0b10; // SSE state present

        if let Some(c) = layout.ymm_hi.filter(|&c| fits(c)) {
            let off = c.offset + n as usize * 16;
            write_u64_at(bytes, off, chunk(2));
            write_u64_at(bytes, off + 8, chunk(3));
            xstate_bv |= 1 << 2;
        }
        if let Some(c) = layout.zmm_hi.filter(|&c| fits(c)) {
            let off = c.offset + n as usize * 32;
            for i in 0..4 {
                write_u64_at(bytes, off + i * 8, chunk(4 + i));
            }
            xstate_bv |= 1 << 6;
        }
    } else if let Some(c) = layout.zmm_hi16.filter(|&c| fits(c)) {
        let off = c.offset + (n as usize - 16) * 64;
        for i in 0..ZMM_CHUNKS {
            write_u64_at(bytes, off + i * 8, chunk(i));
        }
        xstate_bv |= 1 << 7;
    }

    write_u64_at(bytes, XSAVE_XSTATE_BV, xstate_bv);
}
