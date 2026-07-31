//! # hybrid-x86-oracle
//!
//! Pluggable **golden-reference x86-64 CPUs** for differential testing, behind
//! one API. Every backend implements [`X86Oracle`], so a diff harness is
//! written once and pointed at whichever reference you want:
//!
//! | Backend | Feature | What it is | Coverage |
//! |---|---|---|---|
//! | [`SailOracle`] | `sail` (default) | Sail model translated from the ACL2 **x86isa** formal model | 64-bit user-mode integer ISA + SSE moves; theorem-proved semantics |
//! | [`BochsOracle`] | `bochs` | The **Bochs** CPU emulator core, embedded directly | Essentially all user-mode x86-64: SSE/AVX/AVX-512, BMI, CET |
//! | [`KvmOracle`] | `kvm` | **The host CPU itself**, via KVM (Linux) — hardware ground truth | Whatever this machine implements |
//! | [`WhpOracle`] | `whp` | The same, via the Windows Hypervisor Platform | Whatever this machine implements |
//!
//! The first two are pure software and build anywhere. The last two are the same
//! backend wearing the hypervisor interface its OS provides: enable the `hardware`
//! feature to ask for "the host CPU" portably, and let [`Backend::hardware`] name
//! whichever one this platform has.
//!
//! ```
//! use x86_oracle::*;
//!
//! // Written once, valid for every backend.
//! fn check<O: X86Oracle>(mut cpu: O) {
//!     cpu.set_rip(DEFAULT_CODE_ADDR);
//!     cpu.set_gpr(RAX, 5);
//!     cpu.set_gpr(RBX, 7);
//!     assert!(cpu.step_bytes(&[0x48, 0x01, 0xD8]).is_retired()); // add rax, rbx
//!     assert_eq!(cpu.get_gpr(RAX), 12);
//! }
//!
//! # #[cfg(feature = "sail")]
//! check(SailOracle::new());
//! # #[cfg(feature = "bochs")]
//! check(BochsOracle::new());
//! ```
//!
//! For runtime (rather than compile-time) backend choice, use [`AnyOracle`].
//!
//! ## Shared semantic contract
//!
//! Backends are only useful as differential references if they agree on the
//! ground rules. Every backend guarantees:
//!
//! - **Reset state**: 64-bit long mode, CPL 0, flat address space
//!   (linear == the address you pass to the memory API — Sail via its
//!   `app_view`, Bochs via an identity-mapped page table), all GPRs/vector
//!   registers zero, `RFLAGS == 0x2`, `CR4.OSFXSR` set, RIP 0.
//! - **`step()` = exactly one instruction**, fetched from the instance's own
//!   memory at RIP. No interrupts, no timers, no asynchronous events.
//! - **Memory is per instance**, byte-addressed, little-endian; never-written
//!   addresses read as zero.
//! - **Faults do not vector**: a faulting instruction yields
//!   [`StepOutcome::Fault`] with committed-so-far state, not an IDT dispatch.
//!   The *message* is backend-specific (free text); the *classification* in
//!   [`FaultKind`] is normalized and comparable across backends.
//!
//! Where backends legitimately differ (undefined flags, unimplemented
//! opcodes), see `docs/backend-differences.md`.

#[cfg(feature = "bochs")]
mod bochs;
// `backend_kvm` / `backend_whp` come from build.rs: the hardware backends need
// both their cargo feature and the OS whose hypervisor interface they drive (and
// an x86-64 host to have a host CPU worth asking). See `emit_hardware_cfgs`.
#[cfg(any(backend_kvm, backend_whp))]
mod hw;
#[cfg(backend_kvm)]
mod kvm;
#[cfg(feature = "sail")]
mod sail;
#[cfg(backend_whp)]
mod whp;

#[cfg(feature = "bochs")]
pub use bochs::BochsOracle;
#[cfg(backend_kvm)]
pub use kvm::KvmOracle;
#[cfg(feature = "sail")]
pub use sail::SailOracle;
#[cfg(backend_whp)]
pub use whp::WhpOracle;

mod any;
pub use any::{AnyOracle, Backend};

// ------------------------------------------------------------------ registers

/// GPR indices (x86 ModRM / ACL2 x86isa encoding).
pub const RAX: u32 = 0;
pub const RCX: u32 = 1;
pub const RDX: u32 = 2;
pub const RBX: u32 = 3;
pub const RSP: u32 = 4;
pub const RBP: u32 = 5;
pub const RSI: u32 = 6;
pub const RDI: u32 = 7;
pub const R8: u32 = 8;
pub const R9: u32 = 9;
pub const R10: u32 = 10;
pub const R11: u32 = 11;
pub const R12: u32 = 12;
pub const R13: u32 = 13;
pub const R14: u32 = 14;
pub const R15: u32 = 15;

/// Segment register indices (x86isa order; also used by the Bochs backend).
pub const SEG_ES: u32 = 0;
pub const SEG_CS: u32 = 1;
pub const SEG_SS: u32 = 2;
pub const SEG_DS: u32 = 3;
pub const SEG_FS: u32 = 4;
pub const SEG_GS: u32 = 5;

/// MSRs, addressed by their **architectural MSR number** (not a backend index).
/// Only these are portable across backends; see [`X86Oracle::set_msr`].
pub const IA32_EFER: u32 = 0xC000_0080;
pub const IA32_STAR: u32 = 0xC000_0081;
pub const IA32_LSTAR: u32 = 0xC000_0082;
pub const IA32_FMASK: u32 = 0xC000_0084;
pub const IA32_FS_BASE: u32 = 0xC000_0100;
pub const IA32_GS_BASE: u32 = 0xC000_0101;
pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

/// RFLAGS bit masks.
pub const FLAG_CF: u64 = 1 << 0;
pub const FLAG_PF: u64 = 1 << 2;
pub const FLAG_AF: u64 = 1 << 4;
pub const FLAG_ZF: u64 = 1 << 6;
pub const FLAG_SF: u64 = 1 << 7;
pub const FLAG_TF: u64 = 1 << 8;
/// RF — resume flag (bit 16). Like TF, it belongs to single-stepping machinery
/// rather than to the program, so backends mask it out of `get_rflags`.
pub const FLAG_RF: u64 = 1 << 16;
pub const FLAG_IF: u64 = 1 << 9;
pub const FLAG_DF: u64 = 1 << 10;
pub const FLAG_OF: u64 = 1 << 11;

/// The status flags an ALU instruction is normally diffed on.
pub const ARITH_FLAGS: u64 = FLAG_CF | FLAG_PF | FLAG_AF | FLAG_ZF | FLAG_SF | FLAG_OF;

/// EFER.LMA — long mode active (bit 10).
pub const EFER_LMA: u64 = 1 << 10;
/// EFER.SCE — SYSCALL/SYSRET enable (bit 0).
pub const EFER_SCE: u64 = 1 << 0;
/// CR4.OSFXSR — SSE enable (bit 9).
pub const CR4_OSFXSR: u64 = 1 << 9;
/// CR4.OSXSAVE — XSAVE/AVX enable (bit 18).
pub const CR4_OSXSAVE: u64 = 1 << 18;

pub const ZMM_BITS: usize = 512;
pub const ZMM_CHUNKS: usize = ZMM_BITS / 64;
pub const ZMM_REGS: usize = 32;
pub const SEG_COUNT: usize = 6;

/// Where a backend puts the code it executes in the examples and tests. Any
/// canonical address works; this one is simply out of the way.
pub const DEFAULT_CODE_ADDR: u64 = 0x40_0000;

// -------------------------------------------------------------------- outcome

/// Backend-independent classification of a faulting step. The exact x86
/// exception vector is not always recoverable from every backend, so this is
/// deliberately coarse — enough to tell "we both rejected this encoding" from
/// "one of us executed it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultKind {
    /// #UD — invalid/undefined opcode, or an encoding this CPU rejects.
    UndefinedOpcode,
    /// #DE — divide error.
    DivideError,
    /// #PF — page fault / non-canonical or unmapped address.
    PageFault,
    /// #GP — general protection (incl. non-canonical operands).
    GeneralProtection,
    /// #AC — alignment check.
    AlignmentCheck,
    /// Another architectural exception the backend could name but this enum
    /// does not model separately.
    OtherException,
    /// **Not an architectural fault**: the backend does not implement this
    /// instruction. Exclude these encodings from a diff campaign — they say
    /// nothing about the instruction's semantics.
    Unimplemented,
}

impl FaultKind {
    /// True for real architectural faults — i.e. this outcome is a meaningful
    /// diff target, unlike [`FaultKind::Unimplemented`].
    pub fn is_architectural(self) -> bool {
        self != FaultKind::Unimplemented
    }
}

/// What happened during one [`X86Oracle::step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The instruction retired; RIP has advanced (or branched).
    Retired,
    /// The instruction raised a fault. `kind` is normalized across backends;
    /// `msg` is the backend's own free-text description (do not diff on it).
    /// No IDT vectoring happened: state is what was committed before the
    /// fault.
    Fault { kind: FaultKind, msg: String },
    /// A SYSCALL/SYSENTER reached the boundary of what the oracle models
    /// (Sail: its app-view stub; Bochs: executed into kernel entry, which we
    /// stop at). Emulate the syscall yourself.
    Syscall,
}

impl StepOutcome {
    pub fn is_retired(&self) -> bool {
        matches!(self, StepOutcome::Retired)
    }

    pub fn fault_kind(&self) -> Option<FaultKind> {
        match self {
            StepOutcome::Fault { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    /// True if this outcome is comparable across backends. Unimplemented
    /// instructions are not: one backend refusing to execute an encoding says
    /// nothing about what the encoding *means*.
    pub fn is_comparable(&self) -> bool {
        !matches!(
            self,
            StepOutcome::Fault { kind: FaultKind::Unimplemented, .. }
        )
    }

    /// Backend-independent projection: two backends agree iff this matches.
    pub fn classify(&self) -> Option<FaultKind> {
        self.fault_kind()
    }
}

// ---------------------------------------------------------------------- state

/// A comparable snapshot of the architectural state a diff harness normally
/// checks. Deliberately excludes anything backend-specific.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuState {
    /// ModRM order: RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI, R8..R15.
    pub gpr: [u64; 16],
    pub rip: u64,
    pub rflags: u64,
    /// ZMM0..ZMM31, each as 8 little-endian 64-bit chunks.
    pub zmm: [[u64; ZMM_CHUNKS]; ZMM_REGS],
}

impl CpuState {
    /// Compare only GPRs, RIP and the arithmetic flags — the usual diff for
    /// integer instructions, ignoring vector state and non-status flags.
    pub fn eq_integer(&self, other: &Self) -> bool {
        self.gpr == other.gpr
            && self.rip == other.rip
            && (self.rflags & ARITH_FLAGS) == (other.rflags & ARITH_FLAGS)
    }

    /// Human-readable list of the fields that differ, for test failure
    /// messages. Empty iff the states are identical.
    pub fn diff(&self, other: &Self) -> Vec<String> {
        self.diff_masked(other, 0)
    }

    /// Like [`diff`](Self::diff) but ignoring the RFLAGS bits in
    /// `ignore_flags`. Use it for instructions whose ISA definition leaves
    /// flags **undefined** (IMUL/MUL leave SF/ZF/AF/PF undefined, SHLD leaves
    /// OF undefined for counts > 1, ...): two correct implementations may
    /// legitimately differ there, so comparing those bits produces noise
    /// rather than findings.
    pub fn diff_masked(&self, other: &Self, ignore_flags: u64) -> Vec<String> {
        const NAMES: [&str; 16] = [
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ];
        let mut out = Vec::new();
        for i in 0..16 {
            if self.gpr[i] != other.gpr[i] {
                out.push(format!("{}: {:#x} vs {:#x}", NAMES[i], self.gpr[i], other.gpr[i]));
            }
        }
        if self.rip != other.rip {
            out.push(format!("rip: {:#x} vs {:#x}", self.rip, other.rip));
        }
        let mask = !ignore_flags;
        if (self.rflags & mask) != (other.rflags & mask) {
            out.push(format!(
                "rflags: {:#x} vs {:#x}{}",
                self.rflags,
                other.rflags,
                if ignore_flags != 0 {
                    format!(" (ignoring {ignore_flags:#x})")
                } else {
                    String::new()
                }
            ));
        }
        for i in 0..ZMM_REGS {
            if self.zmm[i] != other.zmm[i] {
                out.push(format!("zmm{i}: {:x?} vs {:x?}", self.zmm[i], other.zmm[i]));
            }
        }
        out
    }
}

impl Default for CpuState {
    fn default() -> Self {
        CpuState {
            gpr: [0; 16],
            rip: 0,
            rflags: 0,
            zmm: [[0; ZMM_CHUNKS]; ZMM_REGS],
        }
    }
}

// ---------------------------------------------------------------------- trait

/// One independent golden-reference x86-64 CPU.
///
/// Implementors start in the reset state described at the [crate] level:
/// 64-bit long mode, flat (identity) address space, everything zeroed. All
/// backends are `Send + Sync`; some serialize internally, so threads buy
/// safety rather than throughput — use process-per-test for parallelism.
///
/// Only [`step`](Self::step) and the accessors below are portable. Anything a
/// single backend can do extra lives on that backend's concrete type.
pub trait X86Oracle: Send + Sync {
    /// Human-readable backend name, for test output ("sail", "bochs").
    fn backend_name(&self) -> &'static str;

    // -- general purpose registers, ModRM order (see the `RAX`..`R15` consts)

    fn set_gpr(&mut self, n: u32, value: u64);
    fn get_gpr(&self, n: u32) -> u64;

    /// Canonical 48-bit addresses only, on every backend.
    fn set_rip(&mut self, value: u64);
    fn get_rip(&self) -> u64;

    /// Bit 1 reads back as 1 on real hardware and on both backends; the Sail
    /// model additionally keeps RFLAGS to 32 bits.
    fn set_rflags(&mut self, value: u64);
    fn get_rflags(&self) -> u64;

    // -- vector registers: 8 little-endian u64 chunks (chunk 0 = XMM low half)

    fn set_zmm(&mut self, n: u32, data: &[u64]);
    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS];

    /// How many of those chunks this backend can actually hold: 8 for 512-bit
    /// ZMM, 4 for YMM, 2 for XMM. Chunks beyond this read as zero and writes to
    /// them are dropped — which is not a bug for a hardware backend, it is what
    /// the CPU has. Diff harnesses should compare `min` of the two backends'
    /// widths, and skip encodings that need more.
    fn vector_chunks(&self) -> usize {
        ZMM_CHUNKS
    }

    /// How many vector registers are addressable: 32 with AVX-512, else 16.
    fn vector_regs(&self) -> usize {
        ZMM_REGS
    }

    // -- control / model-specific registers

    /// By architectural MSR number (e.g. [`IA32_EFER`]). Backends implement
    /// the subset in the `IA32_*` constants; writing an MSR a backend does not
    /// model is ignored, and reading one returns 0.
    fn set_msr(&mut self, msr: u32, value: u64);
    fn get_msr(&self, msr: u32) -> u64;

    /// Control register by number: 0, 2, 3, 4, 8.
    fn set_cr(&mut self, n: u32, value: u64);
    fn get_cr(&self, n: u32) -> u64;

    // -- segmentation (`SEG_ES`..`SEG_GS`)

    fn set_seg_selector(&mut self, n: u32, value: u16);
    fn get_seg_selector(&self, n: u32) -> u16;
    /// In 64-bit mode only FS/GS bases affect addressing. Backends keep this
    /// in sync with `IA32_FS_BASE`/`IA32_GS_BASE`, so either API works.
    fn set_seg_base(&mut self, n: u32, value: u64);
    fn get_seg_base(&self, n: u32) -> u64;
    fn set_seg_limit(&mut self, n: u32, value: u32);
    fn get_seg_limit(&self, n: u32) -> u32;

    // -- memory: per instance, byte-addressed, little-endian, flat

    fn read_mem_byte(&self, addr: u64) -> u8;
    fn write_mem_byte(&mut self, addr: u64, byte: u8);

    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = self.read_mem_byte(addr.wrapping_add(i as u64));
        }
    }

    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        for (i, &byte) in data.iter().enumerate() {
            self.write_mem_byte(addr.wrapping_add(i as u64), byte);
        }
    }

    fn read_mem_u64(&self, addr: u64) -> u64 {
        let mut b = [0u8; 8];
        self.read_mem(addr, &mut b);
        u64::from_le_bytes(b)
    }

    fn write_mem_u64(&mut self, addr: u64, value: u64) {
        self.write_mem(addr, &value.to_le_bytes());
    }

    /// Whether the address's region was ever written — the only way to tell
    /// "wrote 0" from "never touched". Granularity is backend-specific
    /// (Sail: 16 MiB blocks; Bochs: 4 KiB pages), so treat it as a hint.
    fn is_mapped(&self, addr: u64) -> bool;

    // -- execution

    /// Execute exactly one instruction, fetched from this instance's memory at
    /// RIP.
    fn step(&mut self) -> StepOutcome;

    /// Write `code` at the current RIP, then [`step`](Self::step). The usual
    /// entry point: one call = one instruction.
    fn step_bytes(&mut self, code: &[u8]) -> StepOutcome {
        let rip = self.get_rip();
        self.write_mem(rip, code);
        self.step()
    }

    /// Free-text description of the last fault ("" if the last step retired).
    fn fault_msg(&self) -> String;

    // -- snapshots

    fn snapshot(&self) -> CpuState {
        let mut gpr = [0u64; 16];
        for (i, slot) in gpr.iter_mut().enumerate() {
            *slot = self.get_gpr(i as u32);
        }
        let mut zmm = [[0u64; ZMM_CHUNKS]; ZMM_REGS];
        for (i, slot) in zmm.iter_mut().enumerate() {
            *slot = self.get_zmm(i as u32);
        }
        CpuState { gpr, rip: self.get_rip(), rflags: self.get_rflags(), zmm }
    }

    /// Bulk-load a snapshot (everything [`snapshot`](Self::snapshot) captures).
    /// Use this to put two backends into a bit-identical starting state.
    fn restore(&mut self, s: &CpuState) {
        for i in 0..16 {
            self.set_gpr(i as u32, s.gpr[i]);
        }
        self.set_rip(s.rip);
        self.set_rflags(s.rflags);
        for i in 0..ZMM_REGS {
            self.set_zmm(i as u32, &s.zmm[i]);
        }
    }
}
