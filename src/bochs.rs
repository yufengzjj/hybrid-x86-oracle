//! Bochs backend: the Bochs CPU emulator core, embedded via
//! `csrc/bochs_shim.cpp`.
//!
//! Strengths: essentially the whole user-mode x86-64 instruction set —
//! SSE through AVX-512, BMI, CET — decoded and executed by a mature emulator.
//! Limits: it is an emulator, not a proof-checked model, and **only one
//! instance can exist per process** (Bochs is built with SMP disabled, so its
//! CPU is a process-wide singleton).
//!
//! Reset state matches the crate-level contract: 64-bit long mode, CPL 0, and a
//! flat address space produced by an identity-mapped page table (long mode
//! requires paging, so "no paging" is emulated by mapping linear == physical
//! across the low 512 GiB). See [`BochsOracle::ADDRESS_LIMIT`] and
//! [`BochsOracle::RESERVED_START`].

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use crate::{
    FaultKind, ShadowStackError, StepOutcome, X86Oracle, ZMM_CHUNKS, ZMM_REGS, GDT_ADDR, GDT_LIMIT,
    IA32_EFER, IA32_FMASK, IA32_FS_BASE, IA32_GS_BASE, IA32_KERNEL_GS_BASE, IA32_LSTAR, IA32_STAR,
    IA32_S_CET, IA32_U_CET, IDT_ADDR, IDT_LIMIT, SEG_COUNT, SEL_TSS,
};

extern "C" {
    fn oracle_bochs_new() -> *mut std::ffi::c_void;
    fn oracle_bochs_free(h: *mut std::ffi::c_void);
    fn oracle_bochs_cpu_mode() -> u32;
    fn oracle_bochs_set_gpr(n: u32, v: u64);
    fn oracle_bochs_get_gpr(n: u32) -> u64;
    fn oracle_bochs_set_rip(v: u64);
    fn oracle_bochs_get_rip() -> u64;
    fn oracle_bochs_set_rflags(v: u64);
    fn oracle_bochs_get_rflags() -> u64;
    fn oracle_bochs_set_zmm(n: u32, chunk: u32, v: u64);
    fn oracle_bochs_get_zmm(n: u32, chunk: u32) -> u64;
    fn oracle_bochs_set_msr(msr: u32, v: u64);
    fn oracle_bochs_get_msr(msr: u32) -> u64;
    fn oracle_bochs_set_cr(n: u32, v: u64);
    fn oracle_bochs_get_cr(n: u32) -> u64;
    fn oracle_bochs_set_ssp(v: u64);
    fn oracle_bochs_get_ssp() -> u64;
    fn oracle_bochs_enable_shadow_stack(base: u64, len: u64) -> i32;
    fn oracle_bochs_set_dtr(which: u32, base: u64, limit: u64);
    fn oracle_bochs_set_tr_selector(v: u64);
    fn oracle_bochs_set_seg_selector(n: u32, v: u64);
    fn oracle_bochs_get_seg_selector(n: u32) -> u64;
    fn oracle_bochs_set_seg_base(n: u32, v: u64);
    fn oracle_bochs_get_seg_base(n: u32) -> u64;
    fn oracle_bochs_set_seg_limit(n: u32, v: u64);
    fn oracle_bochs_get_seg_limit(n: u32) -> u64;
    fn oracle_bochs_read_mem(addr: u64) -> u8;
    fn oracle_bochs_write_mem(addr: u64, byte: u8);
    fn oracle_bochs_read_mem_slice(addr: u64, dst: *mut u8, len: u64);
    fn oracle_bochs_write_mem_slice(addr: u64, src: *const u8, len: u64);
    fn oracle_bochs_is_mapped(addr: u64) -> i32;
    fn oracle_bochs_step(out_vector: *mut u32, out_error: *mut u32) -> u32;
}

/// `BX_MODE_LONG_64` in Bochs' `BxCpuMode` enum.
const MODE_LONG_64: u32 = 4;

// The Bochs core keeps all state in process globals, so this serializes access.
static BOCHS_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    BOCHS_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

// Occupancy of the one and only CPU. `new()` waits here rather than failing, so
// a parallel test runner just serializes on it; the flag is released on Drop.
static OCCUPIED: Mutex<bool> = Mutex::new(false);
static RELEASED: Condvar = Condvar::new();

/// Waits for the CPU to become free, then claims it. Bounded, so forgetting to
/// drop an oracle surfaces as a clear panic instead of a silent deadlock — the
/// usual cause is shadowing (`let c = ...; let c = ...`), which does not drop.
fn claim_instance() {
    const WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);
    let mut occupied = OCCUPIED.lock().unwrap_or_else(PoisonError::into_inner);
    while *occupied {
        let (guard, timeout) = RELEASED
            .wait_timeout(occupied, WAIT_LIMIT)
            .unwrap_or_else(PoisonError::into_inner);
        occupied = guard;
        if timeout.timed_out() && *occupied {
            panic!(
                "timed out waiting for the Bochs CPU: another BochsOracle is still alive. \
                 The Bochs core is a process-wide singleton — drop the first oracle before \
                 creating the next (note that `let c = ...; let c = ...` shadows rather than \
                 drops), or run one oracle per process (`cargo nextest run`)."
            );
        }
    }
    *occupied = true;
}

/// Claims the CPU only if it is free right now.
fn try_claim_instance() -> bool {
    let mut occupied = OCCUPIED.lock().unwrap_or_else(PoisonError::into_inner);
    if *occupied {
        return false;
    }
    *occupied = true;
    true
}

fn release_instance() {
    let mut occupied = OCCUPIED.lock().unwrap_or_else(PoisonError::into_inner);
    *occupied = false;
    RELEASED.notify_one();
}

pub struct BochsOracle {
    h: *mut std::ffi::c_void,
    /// Vector/error code of the last fault, for `fault_msg`.
    last_fault: Option<(u32, u32)>,
}

// SAFETY: every FFI call runs under BOCHS_LOCK, and only one instance can exist.
unsafe impl Send for BochsOracle {}
unsafe impl Sync for BochsOracle {}

impl BochsOracle {
    /// Linear addresses below this are identity-mapped and usable. Above it
    /// there is no page table, so an access faults.
    pub const ADDRESS_LIMIT: u64 = 512 << 30;
    /// The identity map's own page tables live here (about 2 MiB). Treat this
    /// region as reserved: writing it corrupts the address space.
    pub const RESERVED_START: u64 = 0x7F_C000_0000;

    /// A fresh CPU in the reset state described at the crate level.
    ///
    /// Bochs' CPU is a process-wide singleton (it is built with SMP disabled),
    /// so unlike [`crate::SailOracle`] only one can exist at a time. Rather
    /// than fail, this **blocks** until any live instance is dropped — so a
    /// parallel test runner simply serializes here. Use
    /// [`try_new`](Self::try_new) for the non-blocking form, and process-per-test
    /// (`cargo nextest run`) for real parallelism.
    pub fn new() -> Self {
        claim_instance();
        match Self::init() {
            Some(cpu) => cpu,
            None => {
                release_instance();
                panic!("Bochs CPU initialization failed");
            }
        }
    }

    /// `None` if an instance already exists, instead of waiting for it.
    pub fn try_new() -> Option<Self> {
        if !try_claim_instance() {
            return None;
        }
        match Self::init() {
            Some(cpu) => Some(cpu),
            None => {
                release_instance();
                None
            }
        }
    }

    /// Resets the singleton core; the instance slot must already be claimed.
    fn init() -> Option<Self> {
        let _g = lock();
        let h = unsafe { oracle_bochs_new() };
        if h.is_null() {
            return None;
        }
        // Catches an init regression immediately: without the mode-derivation
        // and A20 setup in the shim, the core silently stays in real mode and
        // every fetch reads zeroes.
        assert_eq!(
            unsafe { oracle_bochs_cpu_mode() },
            MODE_LONG_64,
            "Bochs did not enter 64-bit long mode"
        );
        // Align the descriptor-table REGISTER values to the crate contract, so
        // SGDT/SIDT/STR read the same as on the hardware backends. Values only:
        // no memory backs the tables. Faulting still behaves exactly as with
        // the reset-value IDTR — a fault's delivery does walk the IDT (the
        // instrumentation stop is only a flag), but sparse guest memory reads
        // as zeros, so every gate is non-present and delivery aborts before
        // committing state (see oracle_bochs_set_dtr in the shim, which also
        // explains why real tables must never be mirrored into Bochs memory).
        // LDTR is already the null selector from reset.
        unsafe {
            oracle_bochs_set_dtr(0, GDT_ADDR, GDT_LIMIT as u64);
            oracle_bochs_set_dtr(1, IDT_ADDR, IDT_LIMIT as u64);
            oracle_bochs_set_tr_selector(SEL_TSS as u64);
        }
        Some(BochsOracle { h, last_fault: None })
    }

    /// Bochs' `BxCpuMode` (4 = long 64-bit). Bochs-specific; exposed so a test
    /// can assert the core really is in long mode.
    pub fn cpu_mode(&self) -> u32 {
        let _g = lock();
        unsafe { oracle_bochs_cpu_mode() }
    }

    /// x86 exception vector of the last fault, if any (6 = #UD, 0 = #DE, ...).
    /// Bochs-specific: it is more precise than [`FaultKind`], which is the
    /// portable projection.
    pub fn last_vector(&self) -> Option<u32> {
        self.last_fault.map(|(v, _)| v)
    }

    /// Error code pushed with the last fault (0 when the exception has none).
    pub fn last_error_code(&self) -> Option<u32> {
        self.last_fault.map(|(_, e)| e)
    }

    fn classify(vector: u32) -> FaultKind {
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

    fn vector_name(vector: u32) -> &'static str {
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
}

impl Default for BochsOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BochsOracle {
    fn drop(&mut self) {
        {
            let _g = lock();
            unsafe { oracle_bochs_free(self.h) }
        }
        release_instance(); // lets a waiting new() proceed
    }
}

impl X86Oracle for BochsOracle {
    fn backend_name(&self) -> &'static str {
        "bochs"
    }

    fn set_gpr(&mut self, n: u32, value: u64) {
        debug_assert!(n <= 15);
        let _g = lock();
        unsafe { oracle_bochs_set_gpr(n, value) }
    }

    fn get_gpr(&self, n: u32) -> u64 {
        debug_assert!(n <= 15);
        let _g = lock();
        unsafe { oracle_bochs_get_gpr(n) }
    }

    fn set_rip(&mut self, value: u64) {
        let _g = lock();
        unsafe { oracle_bochs_set_rip(value) }
    }

    fn get_rip(&self) -> u64 {
        let _g = lock();
        unsafe { oracle_bochs_get_rip() }
    }

    fn set_rflags(&mut self, value: u64) {
        let _g = lock();
        unsafe { oracle_bochs_set_rflags(value) }
    }

    /// Goes through Bochs' `read_eflags()`, which materializes the lazily
    /// evaluated arithmetic flags — reading the raw field returns stale
    /// CF/PF/AF/ZF/SF/OF.
    fn get_rflags(&self) -> u64 {
        let _g = lock();
        unsafe { oracle_bochs_get_rflags() }
    }

    fn set_zmm(&mut self, n: u32, data: &[u64]) {
        debug_assert!((n as usize) < ZMM_REGS && data.len() <= ZMM_CHUNKS);
        let _g = lock();
        for i in 0..ZMM_CHUNKS {
            let v = data.get(i).copied().unwrap_or(0);
            unsafe { oracle_bochs_set_zmm(n, i as u32, v) }
        }
    }

    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS] {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = lock();
        let mut out = [0u64; ZMM_CHUNKS];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = unsafe { oracle_bochs_get_zmm(n, i as u32) };
        }
        out
    }

    fn set_msr(&mut self, msr: u32, value: u64) {
        let _g = lock();
        unsafe { oracle_bochs_set_msr(msr, value) }
    }

    fn get_msr(&self, msr: u32) -> u64 {
        match msr {
            IA32_EFER | IA32_STAR | IA32_LSTAR | IA32_FMASK | IA32_FS_BASE | IA32_GS_BASE
            | IA32_KERNEL_GS_BASE | IA32_U_CET | IA32_S_CET => {
                let _g = lock();
                unsafe { oracle_bochs_get_msr(msr) }
            }
            // Unmodelled MSRs read as zero, per the trait contract.
            _ => 0,
        }
    }

    fn set_cr(&mut self, n: u32, value: u64) {
        let _g = lock();
        unsafe { oracle_bochs_set_cr(n, value) }
    }

    fn get_cr(&self, n: u32) -> u64 {
        let _g = lock();
        unsafe { oracle_bochs_get_cr(n) }
    }

    fn enable_shadow_stack(&mut self, base: u64, len: u64) -> Result<(), ShadowStackError> {
        let _g = lock();
        // 0 ok, 1 no CET in this build or CPU model, 2 range refused.
        match unsafe { oracle_bochs_enable_shadow_stack(base, len) } {
            0 => Ok(()),
            2 => Err(ShadowStackError::BadRange),
            _ => Err(ShadowStackError::Unsupported),
        }
    }

    fn set_ssp(&mut self, value: u64) {
        let _g = lock();
        unsafe { oracle_bochs_set_ssp(value) }
    }

    fn get_ssp(&self) -> u64 {
        let _g = lock();
        unsafe { oracle_bochs_get_ssp() }
    }

    fn set_seg_selector(&mut self, n: u32, value: u16) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_set_seg_selector(n, value as u64) }
    }

    fn get_seg_selector(&self, n: u32) -> u16 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_get_seg_selector(n) as u16 }
    }

    /// Bochs keeps the FS/GS base in the segment's descriptor cache, and
    /// `IA32_FS_BASE`/`IA32_GS_BASE` are views onto the same storage — so
    /// either API works, as the trait promises.
    fn set_seg_base(&mut self, n: u32, value: u64) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_set_seg_base(n, value) }
    }

    fn get_seg_base(&self, n: u32) -> u64 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_get_seg_base(n) }
    }

    fn set_seg_limit(&mut self, n: u32, value: u32) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_set_seg_limit(n, value as u64) }
    }

    fn get_seg_limit(&self, n: u32) -> u32 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        unsafe { oracle_bochs_get_seg_limit(n) as u32 }
    }

    fn read_mem_byte(&self, addr: u64) -> u8 {
        let _g = lock();
        unsafe { oracle_bochs_read_mem(addr) }
    }

    fn write_mem_byte(&mut self, addr: u64, byte: u8) {
        let _g = lock();
        unsafe { oracle_bochs_write_mem(addr, byte) }
    }

    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        let _g = lock();
        unsafe { oracle_bochs_read_mem_slice(addr, buf.as_mut_ptr(), buf.len() as u64) }
    }

    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        let _g = lock();
        unsafe { oracle_bochs_write_mem_slice(addr, data.as_ptr(), data.len() as u64) }
    }

    /// 4 KiB granularity (Bochs' page map), versus the Sail backend's 16 MiB
    /// blocks — so this is a hint, not a portable predicate.
    fn is_mapped(&self, addr: u64) -> bool {
        let _g = lock();
        unsafe { oracle_bochs_is_mapped(addr) != 0 }
    }

    fn step(&mut self) -> StepOutcome {
        let mut vector = 0u32;
        let mut error = 0u32;
        let status = {
            let _g = lock();
            unsafe { oracle_bochs_step(&mut vector, &mut error) }
        };
        match status {
            0 => {
                self.last_fault = None;
                StepOutcome::Retired
            }
            2 => {
                // HLT: the core stopped and there is nothing to wake it.
                self.last_fault = None;
                StepOutcome::Retired
            }
            _ => {
                self.last_fault = Some((vector, error));
                StepOutcome::Fault { kind: Self::classify(vector), msg: self.fault_msg() }
            }
        }
    }

    fn fault_msg(&self) -> String {
        match self.last_fault {
            None => String::new(),
            Some((v, 0)) => format!("{} (vector {v})", Self::vector_name(v)),
            Some((v, e)) => {
                format!("{} (vector {v}, error code {e:#x})", Self::vector_name(v))
            }
        }
    }
}
