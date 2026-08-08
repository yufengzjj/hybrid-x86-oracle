//! Runtime backend selection.
//!
//! [`X86Oracle`] is object-safe, so `Box<dyn X86Oracle>` works and is the
//! simplest way to pick a backend at runtime. [`AnyOracle`] adds the
//! convenience of an enumerable, `Backend`-tagged wrapper that is also handy
//! for iterating over "every backend compiled into this build":
//!
//! ```no_run
//! use x86_oracle::*;
//!
//! for backend in Backend::available() {
//!     let mut cpu = AnyOracle::new(backend);
//!     cpu.set_rip(DEFAULT_CODE_ADDR);
//!     cpu.set_gpr(RAX, 5);
//!     cpu.set_gpr(RBX, 7);
//!     let out = cpu.step_bytes(&[0x48, 0x01, 0xD8]);
//!     println!("{}: {out:?} rax={}", backend.name(), cpu.get_gpr(RAX));
//! }
//! ```

use crate::{CpuState, ShadowStackError, StepOutcome, X86Oracle, ZMM_CHUNKS};

/// Which reference implementation to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    /// The ACL2-derived Sail formal model (`sail` feature).
    Sail,
    /// The Bochs CPU emulator core (`bochs` feature).
    Bochs,
    /// The host CPU via KVM (`kvm` feature, Linux) — hardware ground truth.
    Kvm,
    /// The host CPU via the Windows Hypervisor Platform (`whp` feature,
    /// Windows) — the same ground truth, different doorway.
    Whp,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Sail => "sail",
            Backend::Bochs => "bochs",
            Backend::Kvm => "kvm",
            Backend::Whp => "whp",
        }
    }

    /// The hardware backend for *this* platform, if it is compiled in and usable:
    /// [`Kvm`](Backend::Kvm) on Linux, [`Whp`](Backend::Whp) on Windows. Use it
    /// to ask for "the real CPU" in portable code — the `hardware` cargo feature
    /// enables whichever applies.
    pub fn hardware() -> Option<Backend> {
        Self::available().into_iter().find(|b| matches!(b, Backend::Kvm | Backend::Whp))
    }

    /// Backends compiled into this build, in a stable order. Use this to run a
    /// diff campaign against everything available without `#[cfg]` in the test.
    pub fn available() -> Vec<Backend> {
        #[allow(unused_mut)] // empty on a platform with no usable backend
        let mut v = Vec::new();
        #[cfg(feature = "sail")]
        v.push(Backend::Sail);
        #[cfg(feature = "bochs")]
        v.push(Backend::Bochs);
        // Compiled in but unusable (no /dev/kvm access, WHP feature switched
        // off) counts as absent: a diff campaign should quietly run without it
        // rather than fail.
        #[cfg(backend_kvm)]
        if crate::KvmOracle::is_available() {
            v.push(Backend::Kvm);
        }
        #[cfg(backend_whp)]
        if crate::WhpOracle::is_available() {
            v.push(Backend::Whp);
        }
        v
    }

    /// Every unordered pair of available backends — the natural unit of a
    /// differential test.
    pub fn pairs() -> Vec<(Backend, Backend)> {
        let all = Self::available();
        let mut out = Vec::new();
        for (i, &a) in all.iter().enumerate() {
            for &b in &all[i + 1..] {
                out.push((a, b));
            }
        }
        out
    }
}

/// A CPU whose backend is chosen at runtime. Implements [`X86Oracle`] by
/// forwarding, so it is a drop-in for the generic code.
pub enum AnyOracle {
    #[cfg(feature = "sail")]
    Sail(crate::SailOracle),
    #[cfg(feature = "bochs")]
    Bochs(crate::BochsOracle),
    #[cfg(backend_kvm)]
    Kvm(crate::KvmOracle),
    #[cfg(backend_whp)]
    Whp(crate::WhpOracle),
    /// Never constructed — `Infallible` has no values. It exists so that a build
    /// with *no* usable backend (say `--features hardware` on macOS, where
    /// neither hypervisor applies) still has a well-formed enum for the matches
    /// below to be exhaustive over. Zero variants would not compile, and a build
    /// that simply has nothing to offer should say so at runtime rather than
    /// refuse to build.
    #[doc(hidden)]
    Never(std::convert::Infallible),
}

impl AnyOracle {
    /// Panics if `backend` cannot be created here — check [`Backend::available`]
    /// first, or use [`try_new`](Self::try_new), which says why.
    pub fn new(backend: Backend) -> Self {
        Self::try_new(backend)
            .unwrap_or_else(|e| panic!("could not create the {} backend: {e}", backend.name()))
    }

    /// The fallible form of [`new`](Self::new).
    ///
    /// A backend left out of this build reports [`ErrorKind::Unsupported`];
    /// anything else is the backend's own failure to start, passed through as it
    /// came. The two are worth telling apart: reporting both as "not compiled in"
    /// sends people to inspect their cargo features when the real answer was that
    /// the hypervisor refused, say, a memory mapping.
    ///
    /// [`ErrorKind::Unsupported`]: std::io::ErrorKind::Unsupported
    pub fn try_new(backend: Backend) -> std::io::Result<Self> {
        match backend {
            #[cfg(feature = "sail")]
            Backend::Sail => Ok(AnyOracle::Sail(crate::SailOracle::new())),
            #[cfg(feature = "bochs")]
            Backend::Bochs => Ok(AnyOracle::Bochs(crate::BochsOracle::new())),
            #[cfg(backend_kvm)]
            Backend::Kvm => crate::KvmOracle::try_new().map(AnyOracle::Kvm),
            #[cfg(backend_whp)]
            Backend::Whp => crate::WhpOracle::try_new().map(AnyOracle::Whp),
            #[allow(unreachable_patterns)]
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("not compiled in; enable the `{}` cargo feature", backend.name()),
            )),
        }
    }

    pub fn backend(&self) -> Backend {
        match self {
            #[cfg(feature = "sail")]
            AnyOracle::Sail(_) => Backend::Sail,
            #[cfg(feature = "bochs")]
            AnyOracle::Bochs(_) => Backend::Bochs,
            #[cfg(backend_kvm)]
            AnyOracle::Kvm(_) => Backend::Kvm,
            #[cfg(backend_whp)]
            AnyOracle::Whp(_) => Backend::Whp,
            AnyOracle::Never(never) => match *never {},
        }
    }
}

/// Forward every trait method to the active backend. The `dispatch!` macro
/// keeps this from being one arm per method per backend.
macro_rules! dispatch {
    ($self:ident, $inner:ident => $body:expr) => {
        match $self {
            #[cfg(feature = "sail")]
            AnyOracle::Sail($inner) => $body,
            #[cfg(feature = "bochs")]
            AnyOracle::Bochs($inner) => $body,
            #[cfg(backend_kvm)]
            AnyOracle::Kvm($inner) => $body,
            #[cfg(backend_whp)]
            AnyOracle::Whp($inner) => $body,
            AnyOracle::Never(never) => match *never {},
        }
    };
}

// With no backend compiled in, every forwarding parameter below goes nowhere —
// that is the whole point of the `Never` variant, not an oversight.
#[cfg_attr(
    not(any(feature = "sail", feature = "bochs", backend_kvm, backend_whp)),
    allow(unused_variables)
)]
impl X86Oracle for AnyOracle {
    fn backend_name(&self) -> &'static str {
        dispatch!(self, o => o.backend_name())
    }

    fn set_gpr(&mut self, n: u32, value: u64) {
        dispatch!(self, o => o.set_gpr(n, value))
    }
    fn get_gpr(&self, n: u32) -> u64 {
        dispatch!(self, o => o.get_gpr(n))
    }
    fn set_rip(&mut self, value: u64) {
        dispatch!(self, o => o.set_rip(value))
    }
    fn get_rip(&self) -> u64 {
        dispatch!(self, o => o.get_rip())
    }
    fn set_rflags(&mut self, value: u64) {
        dispatch!(self, o => o.set_rflags(value))
    }
    fn get_rflags(&self) -> u64 {
        dispatch!(self, o => o.get_rflags())
    }
    fn set_zmm(&mut self, n: u32, data: &[u64]) {
        dispatch!(self, o => o.set_zmm(n, data))
    }
    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS] {
        dispatch!(self, o => o.get_zmm(n))
    }
    fn vector_chunks(&self) -> usize {
        dispatch!(self, o => o.vector_chunks())
    }
    fn vector_regs(&self) -> usize {
        dispatch!(self, o => o.vector_regs())
    }
    fn set_msr(&mut self, msr: u32, value: u64) {
        dispatch!(self, o => o.set_msr(msr, value))
    }
    fn get_msr(&self, msr: u32) -> u64 {
        dispatch!(self, o => o.get_msr(msr))
    }
    fn set_cr(&mut self, n: u32, value: u64) {
        dispatch!(self, o => o.set_cr(n, value))
    }
    fn get_cr(&self, n: u32) -> u64 {
        dispatch!(self, o => o.get_cr(n))
    }
    fn enable_shadow_stack(&mut self, base: u64, len: u64) -> Result<(), ShadowStackError> {
        dispatch!(self, o => o.enable_shadow_stack(base, len))
    }
    fn set_ssp(&mut self, value: u64) {
        dispatch!(self, o => o.set_ssp(value))
    }
    fn get_ssp(&self) -> u64 {
        dispatch!(self, o => o.get_ssp())
    }
    fn set_seg_selector(&mut self, n: u32, value: u16) {
        dispatch!(self, o => o.set_seg_selector(n, value))
    }
    fn get_seg_selector(&self, n: u32) -> u16 {
        dispatch!(self, o => o.get_seg_selector(n))
    }
    fn set_seg_base(&mut self, n: u32, value: u64) {
        dispatch!(self, o => o.set_seg_base(n, value))
    }
    fn get_seg_base(&self, n: u32) -> u64 {
        dispatch!(self, o => o.get_seg_base(n))
    }
    fn set_seg_limit(&mut self, n: u32, value: u32) {
        dispatch!(self, o => o.set_seg_limit(n, value))
    }
    fn get_seg_limit(&self, n: u32) -> u32 {
        dispatch!(self, o => o.get_seg_limit(n))
    }
    fn read_mem_byte(&self, addr: u64) -> u8 {
        dispatch!(self, o => o.read_mem_byte(addr))
    }
    fn write_mem_byte(&mut self, addr: u64, byte: u8) {
        dispatch!(self, o => o.write_mem_byte(addr, byte))
    }
    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        dispatch!(self, o => o.read_mem(addr, buf))
    }
    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        dispatch!(self, o => o.write_mem(addr, data))
    }
    fn is_mapped(&self, addr: u64) -> bool {
        dispatch!(self, o => o.is_mapped(addr))
    }
    fn step(&mut self) -> StepOutcome {
        dispatch!(self, o => o.step())
    }
    fn fault_msg(&self) -> String {
        dispatch!(self, o => o.fault_msg())
    }
    fn snapshot(&self) -> CpuState {
        dispatch!(self, o => o.snapshot())
    }
    fn restore(&mut self, s: &CpuState) {
        dispatch!(self, o => o.restore(s))
    }
}

/// The half of backend selection that does not need a working backend to test.
#[cfg(test)]
mod tests {
    use super::*;

    /// A backend left out of the build must say *that*, and not be confused with
    /// one that is present but failed to start — the distinction is what tells a
    /// user whether to reach for their cargo features or their hypervisor.
    #[test]
    fn a_backend_that_is_not_compiled_in_says_so() {
        // Whichever hardware backend belongs to the *other* OS is never in this
        // build: each is gated on its own platform (see build.rs).
        let absent = if cfg!(backend_kvm) { Backend::Whp } else { Backend::Kvm };
        let err = AnyOracle::try_new(absent).err().expect("cannot be compiled in");
        assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
        assert!(
            err.to_string().contains(absent.name()),
            "the error should name the cargo feature to enable: {err}"
        );
    }
}
