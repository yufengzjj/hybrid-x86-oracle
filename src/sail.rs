//! Sail backend: the x86 model translated from the ACL2 **x86isa** formal
//! model, embedded via `csrc/shim.cpp` (`sail --cpp` → `class model::Model`).
//!
//! Strengths: the integer ISA semantics (including flag corner cases) are
//! machine-checked in ACL2 — the highest-confidence reference available.
//! Limits: the translated snapshot implements the 64-bit user-mode integer ISA
//! plus SSE data movement only; everything else reports
//! [`FaultKind::Unimplemented`].
//!
//! Each instance is an independent CPU (state + memory). The Sail C runtime is
//! not thread-safe, so every model call takes a process-wide mutex.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::{
    FaultKind, StepOutcome, X86Oracle, ZMM_CHUNKS, ZMM_REGS, IA32_EFER, IA32_FMASK, IA32_FS_BASE,
    IA32_GS_BASE, IA32_KERNEL_GS_BASE, IA32_LSTAR, IA32_STAR, SEG_COUNT, SEG_FS, SEG_GS,
};

extern "C" {
    fn oracle_new() -> *mut c_void;
    fn oracle_free(h: *mut c_void);
    fn oracle_set_gpr(h: *mut c_void, n: u32, value: u64);
    fn oracle_get_gpr(h: *mut c_void, n: u32) -> u64;
    fn oracle_set_rip(h: *mut c_void, value: u64);
    fn oracle_get_rip(h: *mut c_void) -> u64;
    fn oracle_set_rflags(h: *mut c_void, value: u64);
    fn oracle_get_rflags(h: *mut c_void) -> u64;
    fn oracle_step(h: *mut c_void) -> u64;
    fn oracle_set_zmm(h: *mut c_void, n: u32, chunk: u32, value: u64);
    fn oracle_get_zmm(h: *mut c_void, n: u32, chunk: u32) -> u64;
    fn oracle_set_msr(h: *mut c_void, idx: u32, value: u64);
    fn oracle_get_msr(h: *mut c_void, idx: u32) -> u64;
    fn oracle_set_ctr(h: *mut c_void, idx: u32, value: u64);
    fn oracle_get_ctr(h: *mut c_void, idx: u32) -> u64;
    fn oracle_set_seg_visible(h: *mut c_void, n: u32, value: u64);
    fn oracle_get_seg_visible(h: *mut c_void, n: u32) -> u64;
    fn oracle_set_seg_base(h: *mut c_void, n: u32, value: u64);
    fn oracle_get_seg_base(h: *mut c_void, n: u32) -> u64;
    fn oracle_set_seg_limit(h: *mut c_void, n: u32, value: u64);
    fn oracle_get_seg_limit(h: *mut c_void, n: u32) -> u64;
    fn oracle_set_seg_attr(h: *mut c_void, n: u32, value: u64);
    fn oracle_get_seg_attr(h: *mut c_void, n: u32) -> u64;
    fn oracle_get_fault_msg(h: *mut c_void, buf: *mut u8, cap: usize) -> usize;
    fn oracle_read_mem(h: *mut c_void, addr: u64) -> u8;
    fn oracle_write_mem(h: *mut c_void, addr: u64, byte: u8);
    fn oracle_is_mapped(h: *mut c_void, addr: u64) -> bool;
}

/// The model's compact MSR file (x86isa `concrete-state.lisp` order) — the
/// only MSRs this backend has storage for. Returns `None` for anything else,
/// which the trait contract says to ignore on write / read as 0.
fn msr_slot(msr: u32) -> Option<u32> {
    Some(match msr {
        IA32_EFER => 0,
        IA32_FS_BASE => 1,
        IA32_GS_BASE => 2,
        IA32_KERNEL_GS_BASE => 3,
        IA32_STAR => 4,
        IA32_LSTAR => 5,
        IA32_FMASK => 6,
        _ => return None,
    })
}

// Serializes every model call across ALL instances: per-instance state is
// fine, but the Sail C runtime keeps process-global scratch (GMP temporaries;
// memory lists swapped through globals) touched on every call.
static MODEL_LOCK: Mutex<()> = Mutex::new(());

fn model_lock() -> MutexGuard<'static, ()> {
    // Guarded sections run only C/C++ (no panics), so a poisoned lock cannot
    // mean torn model state — keep going.
    MODEL_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct SailOracle {
    h: *mut c_void,
}

// SAFETY: instance state is reachable only through this struct (&/&mut
// aliasing) and every FFI call runs under MODEL_LOCK.
unsafe impl Send for SailOracle {}
unsafe impl Sync for SailOracle {}

impl SailOracle {
    /// A fresh CPU in the reset state described at the crate level: 64-bit
    /// long mode, flat address space (the model's `app_view` — no paging),
    /// all registers zero, RFLAGS = 0x2, CR4.OSFXSR set.
    pub fn new() -> Self {
        let _g = model_lock();
        let h = unsafe { oracle_new() };
        assert!(!h.is_null(), "oracle_new returned null");
        SailOracle { h }
    }

    /// Hidden descriptor-cache attribute word. Sail-specific: CS.L (bit 9)
    /// lives here and `new()` sets it. The Bochs backend uses a different
    /// attribute encoding, so this is deliberately not on the trait.
    pub fn set_seg_attr(&mut self, n: u32, value: u16) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_set_seg_attr(self.h, n, value as u64) }
    }

    pub fn get_seg_attr(&self, n: u32) -> u16 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_get_seg_attr(self.h, n) as u16 }
    }

    /// Classify the model's free-text exception message. ACL2 faults carry the
    /// x86 mnemonic (`:UD`, `:DE`, ...); "Model error" / "Translation error"
    /// mean the *model* stopped short, not the CPU.
    fn classify(msg: &str) -> FaultKind {
        if msg.starts_with("Model error") || msg.starts_with("Translation error") {
            // The one exception: the model reports its own "unimplemented"
            // through the same channel as a genuine halt.
            return FaultKind::Unimplemented;
        }
        if msg.contains(":UD") || msg.contains("ILLEGAL-INSTRUCTION") || msg.contains("UD2") {
            FaultKind::UndefinedOpcode
        } else if msg.contains(":DE") {
            FaultKind::DivideError
        } else if msg.contains(":GP") || msg.contains("NON-CANONICAL") {
            FaultKind::GeneralProtection
        } else if msg.contains(":PF") || msg.contains("PAGE-FAULT") {
            FaultKind::PageFault
        } else if msg.contains("UNALIGNED") || msg.contains(":AC") {
            FaultKind::AlignmentCheck
        } else {
            FaultKind::OtherException
        }
    }
}

impl Default for SailOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SailOracle {
    fn drop(&mut self) {
        let _g = model_lock();
        unsafe { oracle_free(self.h) }
    }
}

impl X86Oracle for SailOracle {
    fn backend_name(&self) -> &'static str {
        "sail"
    }

    fn set_gpr(&mut self, n: u32, value: u64) {
        debug_assert!(n <= 15);
        let _g = model_lock();
        unsafe { oracle_set_gpr(self.h, n, value) }
    }

    fn get_gpr(&self, n: u32) -> u64 {
        debug_assert!(n <= 15);
        let _g = model_lock();
        unsafe { oracle_get_gpr(self.h, n) }
    }

    fn set_rip(&mut self, value: u64) {
        let _g = model_lock();
        unsafe { oracle_set_rip(self.h, value) }
    }

    fn get_rip(&self) -> u64 {
        let _g = model_lock();
        unsafe { oracle_get_rip(self.h) }
    }

    fn set_rflags(&mut self, value: u64) {
        let _g = model_lock();
        unsafe { oracle_set_rflags(self.h, value) }
    }

    fn get_rflags(&self) -> u64 {
        let _g = model_lock();
        unsafe { oracle_get_rflags(self.h) }
    }

    fn set_zmm(&mut self, n: u32, data: &[u64]) {
        debug_assert!((n as usize) < ZMM_REGS && data.len() <= ZMM_CHUNKS);
        let _g = model_lock();
        for i in 0..ZMM_CHUNKS {
            let v = data.get(i).copied().unwrap_or(0);
            unsafe { oracle_set_zmm(self.h, n, i as u32, v) }
        }
    }

    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS] {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = model_lock();
        let mut out = [0u64; ZMM_CHUNKS];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = unsafe { oracle_get_zmm(self.h, n, i as u32) };
        }
        out
    }

    fn set_msr(&mut self, msr: u32, value: u64) {
        if let Some(idx) = msr_slot(msr) {
            let _g = model_lock();
            unsafe { oracle_set_msr(self.h, idx, value) }
        }
    }

    fn get_msr(&self, msr: u32) -> u64 {
        match msr_slot(msr) {
            Some(idx) => {
                let _g = model_lock();
                unsafe { oracle_get_msr(self.h, idx) }
            }
            None => 0,
        }
    }

    fn set_cr(&mut self, n: u32, value: u64) {
        debug_assert!(n <= 16);
        let _g = model_lock();
        unsafe { oracle_set_ctr(self.h, n, value) }
    }

    fn get_cr(&self, n: u32) -> u64 {
        debug_assert!(n <= 16);
        let _g = model_lock();
        unsafe { oracle_get_ctr(self.h, n) }
    }

    fn set_seg_selector(&mut self, n: u32, value: u16) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_set_seg_visible(self.h, n, value as u64) }
    }

    fn get_seg_selector(&self, n: u32) -> u16 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_get_seg_visible(self.h, n) as u16 }
    }

    /// In 64-bit mode the model reads the FS/GS bases from
    /// `IA32_FS_BASE`/`IA32_GS_BASE`, so those are written too — the trait
    /// contract promises the two APIs stay in sync.
    fn set_seg_base(&mut self, n: u32, value: u64) {
        debug_assert!((n as usize) < SEG_COUNT);
        {
            let _g = model_lock();
            unsafe { oracle_set_seg_base(self.h, n, value) }
        }
        match n {
            SEG_FS => self.set_msr(IA32_FS_BASE, value),
            SEG_GS => self.set_msr(IA32_GS_BASE, value),
            _ => {}
        }
    }

    fn get_seg_base(&self, n: u32) -> u64 {
        debug_assert!((n as usize) < SEG_COUNT);
        match n {
            // the architecturally effective base in 64-bit mode
            SEG_FS => self.get_msr(IA32_FS_BASE),
            SEG_GS => self.get_msr(IA32_GS_BASE),
            _ => {
                let _g = model_lock();
                unsafe { oracle_get_seg_base(self.h, n) }
            }
        }
    }

    fn set_seg_limit(&mut self, n: u32, value: u32) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_set_seg_limit(self.h, n, value as u64) }
    }

    fn get_seg_limit(&self, n: u32) -> u32 {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = model_lock();
        unsafe { oracle_get_seg_limit(self.h, n) as u32 }
    }

    fn read_mem_byte(&self, addr: u64) -> u8 {
        let _g = model_lock();
        unsafe { oracle_read_mem(self.h, addr) }
    }

    fn write_mem_byte(&mut self, addr: u64, byte: u8) {
        let _g = model_lock();
        unsafe { oracle_write_mem(self.h, addr, byte) }
    }

    // Batched overrides: one lock acquisition for the whole slice.
    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        let _g = model_lock();
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = unsafe { oracle_read_mem(self.h, addr.wrapping_add(i as u64)) };
        }
    }

    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        let _g = model_lock();
        for (i, &byte) in data.iter().enumerate() {
            unsafe { oracle_write_mem(self.h, addr.wrapping_add(i as u64), byte) }
        }
    }

    fn is_mapped(&self, addr: u64) -> bool {
        let _g = model_lock();
        unsafe { oracle_is_mapped(self.h, addr) }
    }

    fn step(&mut self) -> StepOutcome {
        let status = {
            let _g = model_lock();
            unsafe { oracle_step(self.h) }
        };
        match status {
            0 => StepOutcome::Retired,
            2 => StepOutcome::Syscall,
            _ => {
                let msg = self.fault_msg();
                StepOutcome::Fault { kind: Self::classify(&msg), msg }
            }
        }
    }

    fn fault_msg(&self) -> String {
        let _g = model_lock();
        let len = unsafe { oracle_get_fault_msg(self.h, std::ptr::null_mut(), 0) };
        let mut buf = vec![0u8; len + 1];
        unsafe { oracle_get_fault_msg(self.h, buf.as_mut_ptr(), buf.len()) };
        buf.truncate(len);
        String::from_utf8_lossy(&buf).into_owned()
    }
}

/// Sanity checks on the reset state this backend promises.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CR4_OSFXSR, EFER_LMA};

    #[test]
    fn reset_state_matches_contract() {
        let cpu = SailOracle::new();
        assert_ne!(cpu.get_msr(IA32_EFER) & EFER_LMA, 0, "not in long mode");
        assert_ne!(cpu.get_cr(4) & CR4_OSFXSR, 0, "SSE not enabled");
        assert_eq!(cpu.get_rflags(), 0x2);
        assert_eq!(cpu.get_rip(), 0);
    }
}
