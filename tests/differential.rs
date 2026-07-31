//! Cross-backend differential test — the reason this crate exists.
//!
//! Each case is run from a bit-identical starting state on every available
//! backend and the resulting architectural state is compared. Encodings a
//! backend does not implement are reported and skipped: `Unimplemented` says
//! nothing about semantics.
//!
//! Every **pair** of available backends is compared, so adding a backend
//! automatically diffs it against all the others. With all three features on
//! that is sail↔bochs, sail↔kvm and bochs↔kvm — and the KVM side is the host
//! CPU itself, so a disagreement there means a model is wrong.
//!
//! `cargo test --features bochs,kvm`
#![cfg(any(feature = "bochs", all(feature = "kvm", target_os = "linux")))]

use x86_oracle::*;

const CODE: u64 = DEFAULT_CODE_ADDR;
const DATA: u64 = 0x20_0000;
const STACK: u64 = 0x30_0000;

/// One differential case: a starting state plus the instruction bytes.
struct Case {
    name: &'static str,
    code: &'static [u8],
    /// (gpr index, value) pairs applied before the step.
    gprs: &'static [(u32, u64)],
    rflags: u64,
    /// Bytes seeded at `DATA`.
    data: &'static [u8],
    /// RFLAGS bits this instruction leaves **architecturally undefined**, so
    /// two correct implementations may differ there. Comparing them would
    /// report noise, not bugs.
    undef_flags: u64,
}

impl Case {
    const fn new(name: &'static str, code: &'static [u8]) -> Self {
        Case { name, code, gprs: &[], rflags: 0x2, data: &[], undef_flags: 0 }
    }
    const fn gprs(mut self, gprs: &'static [(u32, u64)]) -> Self {
        self.gprs = gprs;
        self
    }
    const fn rflags(mut self, f: u64) -> Self {
        self.rflags = f;
        self
    }
    const fn data(mut self, d: &'static [u8]) -> Self {
        self.data = d;
        self
    }
    const fn undef(mut self, flags: u64) -> Self {
        self.undef_flags = flags;
        self
    }
}

/// Bring a backend to this case's starting state and execute one instruction.
fn run(mut cpu: AnyOracle, case: &Case) -> (StepOutcome, CpuState, Vec<u8>) {
    cpu.set_rip(CODE);
    cpu.set_rflags(case.rflags);
    cpu.set_gpr(RSP, STACK);
    for &(n, v) in case.gprs {
        cpu.set_gpr(n, v);
    }
    if !case.data.is_empty() {
        cpu.write_mem(DATA, case.data);
    }
    let outcome = cpu.step_bytes(case.code);
    let state = cpu.snapshot();
    // Observed memory window: the seeded data plus the top of the stack, so
    // stores and pushes are diffed too.
    let mut mem = vec![0u8; 32];
    cpu.read_mem(DATA, &mut mem[..16]);
    cpu.read_mem(STACK - 16, &mut mem[16..]);
    (outcome, state, mem)
}

/// Compare one case across a pair of backends. Returns Err with a report on
/// divergence, Ok(false) if the case was skipped as unimplemented.
fn diff_case(case: &Case, a: Backend, b: Backend) -> Result<bool, String> {
    // One oracle at a time where possible, but a pair must coexist; that is
    // fine because each backend's single-instance limit (if any) is its own.
    let (sail_out, sail_state, sail_mem) = run(AnyOracle::new(a), case);
    let (bochs_out, bochs_state, bochs_mem) = run(AnyOracle::new(b), case);

    // Either side not implementing the encoding makes the case uninformative.
    if !sail_out.is_comparable() || !bochs_out.is_comparable() {
        let which = if !sail_out.is_comparable() { a.name() } else { b.name() };
        println!("  skip {:<28} unimplemented by {which}", case.name);
        return Ok(false);
    }

    let mut problems = Vec::new();

    fn describe(o: &StepOutcome) -> String {
        match o {
            StepOutcome::Retired => "retired".to_string(),
            StepOutcome::Syscall => "syscall".to_string(),
            StepOutcome::Fault { kind, msg } => format!("{kind:?} ({msg})"),
        }
    }

    let agree = match (&sail_out, &bochs_out) {
        (StepOutcome::Retired, StepOutcome::Retired) => true,
        (StepOutcome::Syscall, StepOutcome::Syscall) => true,
        (StepOutcome::Fault { kind: a, .. }, StepOutcome::Fault { kind: b, .. }) => a == b,
        _ => false,
    };
    if !agree {
        problems.push(format!(
            "outcome differs: {}={}, {}={}",
            a.name(),
            describe(&sail_out),
            b.name(),
            describe(&bochs_out)
        ));
    }

    // After a fault the two models commit different amounts of partial state
    // (no IDT vectoring on either, but "how far it got" is not architectural),
    // so only compare register state when both retired.
    if sail_out.is_retired() && bochs_out.is_retired() {
        for d in sail_state.diff_masked(&bochs_state, case.undef_flags) {
            problems.push(format!("state: {d}"));
        }
        if sail_mem != bochs_mem {
            problems.push(format!("memory: {sail_mem:x?} vs {bochs_mem:x?}"));
        }
    }

    if problems.is_empty() {
        Ok(true)
    } else {
        Err(format!("{}:\n    {}", case.name, problems.join("\n    ")))
    }
}

/// Integer ALU semantics, including the flag corner cases the ACL2 model is
/// strongest at.
static ARITH: &[Case] = &[
    Case::new("add rax,rbx", &[0x48, 0x01, 0xD8]).gprs(&[(RAX, 5), (RBX, 7)]),
    Case::new("add rax,rbx (carry)", &[0x48, 0x01, 0xD8]).gprs(&[(RAX, u64::MAX), (RBX, 1)]),
    Case::new("add rax,rbx (signed ovf)", &[0x48, 0x01, 0xD8])
        .gprs(&[(RAX, i64::MAX as u64), (RBX, 1)]),
    Case::new("add rax,rbx (half-carry)", &[0x48, 0x01, 0xD8]).gprs(&[(RAX, 0xF), (RBX, 1)]),
    Case::new("sub rax,rbx (borrow)", &[0x48, 0x29, 0xD8]).gprs(&[(RAX, 0), (RBX, 1)]),
    Case::new("sub rax,rbx (equal->ZF)", &[0x48, 0x29, 0xD8]).gprs(&[(RAX, 42), (RBX, 42)]),
    Case::new("adc rax,rbx (CF in)", &[0x48, 0x11, 0xD8])
        .gprs(&[(RAX, 1), (RBX, 1)])
        .rflags(0x2 | FLAG_CF),
    Case::new("sbb rax,rbx (CF in)", &[0x48, 0x19, 0xD8])
        .gprs(&[(RAX, 5), (RBX, 1)])
        .rflags(0x2 | FLAG_CF),
    Case::new("and rax,rbx", &[0x48, 0x21, 0xD8]).gprs(&[(RAX, 0xF0F0), (RBX, 0xFF00)]),
    Case::new("or rax,rbx", &[0x48, 0x09, 0xD8]).gprs(&[(RAX, 0xF0F0), (RBX, 0x0F0F)]),
    Case::new("xor rax,rbx", &[0x48, 0x31, 0xD8]).gprs(&[(RAX, 0xFFFF), (RBX, 0xFF00)]),
    Case::new("cmp rax,rbx", &[0x48, 0x39, 0xD8]).gprs(&[(RAX, 1), (RBX, 2)]),
    Case::new("test rax,rax (zero)", &[0x48, 0x85, 0xC0]).gprs(&[(RAX, 0)]),
    Case::new("neg rax", &[0x48, 0xF7, 0xD8]).gprs(&[(RAX, 1)]),
    Case::new("not rax", &[0x48, 0xF7, 0xD0]).gprs(&[(RAX, 0)]),
    Case::new("inc rax (no CF)", &[0x48, 0xFF, 0xC0]).gprs(&[(RAX, u64::MAX)]).rflags(0x2),
    Case::new("dec rax", &[0x48, 0xFF, 0xC8]).gprs(&[(RAX, 0)]),
    Case::new("mov eax,ebx (zero-ext)", &[0x89, 0xD8]).gprs(&[(RAX, u64::MAX), (RBX, 0x1_0000_0001)]),
    Case::new("movsxd rax,ebx", &[0x48, 0x63, 0xC3]).gprs(&[(RBX, 0xFFFF_FFFF)]),
    Case::new("movzx eax,bl", &[0x0F, 0xB6, 0xC3]).gprs(&[(RBX, 0xFF80)]),
    Case::new("imul rax,rbx", &[0x48, 0x0F, 0xAF, 0xC3]).gprs(&[(RAX, 7), (RBX, 6)]),
    // Two-operand IMUL defines CF/OF; SF, ZF, AF and PF are undefined.
    Case::new("imul rax,rbx (neg)", &[0x48, 0x0F, 0xAF, 0xC3])
        .gprs(&[(RAX, (-7i64) as u64), (RBX, 6)])
        .undef(FLAG_SF | FLAG_ZF | FLAG_AF | FLAG_PF),
    // MUL defines CF/OF from the upper half; SF, ZF, AF and PF are undefined.
    Case::new("mul rbx (wide)", &[0x48, 0xF7, 0xE3])
        .gprs(&[(RAX, u64::MAX), (RBX, 3), (RDX, 0)])
        .undef(FLAG_SF | FLAG_ZF | FLAG_AF | FLAG_PF),
    Case::new("div rbx", &[0x48, 0xF7, 0xF3]).gprs(&[(RAX, 100), (RDX, 0), (RBX, 7)]),
    Case::new("div rbx (by zero)", &[0x48, 0xF7, 0xF3]).gprs(&[(RAX, 100), (RDX, 0), (RBX, 0)]),
    Case::new("idiv rbx (neg)", &[0x48, 0xF7, 0xFB])
        .gprs(&[(RAX, (-100i64) as u64), (RDX, u64::MAX), (RBX, 7)]),
    Case::new("shl rax,4", &[0x48, 0xC1, 0xE0, 0x04]).gprs(&[(RAX, 1)]),
    Case::new("shr rax,1", &[0x48, 0xD1, 0xE8]).gprs(&[(RAX, 3)]),
    Case::new("sar rax,1", &[0x48, 0xD1, 0xF8]).gprs(&[(RAX, u64::MAX)]),
    Case::new("rol rax,1", &[0x48, 0xD1, 0xC0]).gprs(&[(RAX, 1u64 << 63)]),
    Case::new("rcr rax,1 (CF in)", &[0x48, 0xD1, 0xD8])
        .gprs(&[(RAX, 1)])
        .rflags(0x2 | FLAG_CF),
    // Rotates by more than 1 take a different path than rotate-by-1 in every
    // implementation, and the SDM leaves OF undefined there (AF always). CF is
    // still defined: ROL takes it from the result's LSB, ROR from its MSB.
    // Each operand below is chosen so those two bits disagree, which is what
    // makes the case discriminating — see docs/backend-differences.md §4b.
    Case::new("rol rax,11", &[0x48, 0xC1, 0xC0, 0x0B])
        .gprs(&[(RAX, 1u64 << 53)])
        .undef(FLAG_OF | FLAG_AF),
    Case::new("ror rax,11", &[0x48, 0xC1, 0xC8, 0x0B])
        .gprs(&[(RAX, 0x800)])
        .undef(FLAG_OF | FLAG_AF),
    Case::new("ror eax,5", &[0xC1, 0xC8, 0x05]).gprs(&[(RAX, 0x20)]).undef(FLAG_OF | FLAG_AF),
    Case::new("ror ax,11", &[0x66, 0xC1, 0xC8, 0x0B])
        .gprs(&[(RAX, 0x800)])
        .undef(FLAG_OF | FLAG_AF),
    Case::new("ror al,3", &[0xC0, 0xC8, 0x03]).gprs(&[(RAX, 0x08)]).undef(FLAG_OF | FLAG_AF),
    // RCL/RCR by more than 1, for the same reason — these rotate through CF, so
    // they take a different route to it than ROL/ROR and need their own cases.
    Case::new("rcl rax,11 (CF in)", &[0x48, 0xC1, 0xD0, 0x0B])
        .gprs(&[(RAX, 0x1F_FFFF_FFFF_FFFF)])
        .rflags(0x2 | FLAG_CF)
        .undef(FLAG_OF | FLAG_AF),
    Case::new("rcr rax,11 (CF in)", &[0x48, 0xC1, 0xD8, 0x0B])
        .gprs(&[(RAX, 0x1F_FFFF_FFFF_FFFF)])
        .rflags(0x2 | FLAG_CF)
        .undef(FLAG_OF | FLAG_AF),
    Case::new("rcr eax,5", &[0xC1, 0xD8, 0x05]).gprs(&[(RAX, 0x8000_0021)]).undef(FLAG_OF | FLAG_AF),
    Case::new("rol al,3", &[0xC0, 0xC0, 0x03]).gprs(&[(RAX, 0x91)]).undef(FLAG_OF | FLAG_AF),
    // SHLD leaves OF undefined for shift counts > 1, and AF undefined always.
    Case::new("shld rax,rbx,4", &[0x48, 0x0F, 0xA4, 0xD8, 0x04])
        .gprs(&[(RAX, 0xF000_0000_0000_0000), (RBX, 0xABCD)])
        .undef(FLAG_OF | FLAG_AF),
    Case::new("bt rax,3", &[0x48, 0x0F, 0xBA, 0xE0, 0x03]).gprs(&[(RAX, 0x8)]),
    Case::new("bsf rax,rbx", &[0x48, 0x0F, 0xBC, 0xC3]).gprs(&[(RBX, 0x100)]),
    Case::new("bsr rax,rbx", &[0x48, 0x0F, 0xBD, 0xC3]).gprs(&[(RBX, 0x100)]),
    Case::new("cmovz rax,rbx", &[0x48, 0x0F, 0x44, 0xC3])
        .gprs(&[(RAX, 1), (RBX, 9)])
        .rflags(0x2 | FLAG_ZF),
    Case::new("setz al", &[0x0F, 0x94, 0xC0]).rflags(0x2 | FLAG_ZF),
    Case::new("cqo", &[0x48, 0x99]).gprs(&[(RAX, u64::MAX)]),
    Case::new("cdqe", &[0x48, 0x98]).gprs(&[(RAX, 0xFFFF_FFFF)]),
    Case::new("xchg rax,rbx", &[0x48, 0x87, 0xD8]).gprs(&[(RAX, 1), (RBX, 2)]),
    Case::new("nop", &[0x90]),
    Case::new("ud2", &[0x0F, 0x0B]),
];

/// Memory operands, addressing modes and the stack.
static MEMORY: &[Case] = &[
    Case::new("mov [rbx],rax", &[0x48, 0x89, 0x03])
        .gprs(&[(RAX, 0x1122_3344_5566_7788), (RBX, DATA)]),
    Case::new("mov rax,[rbx]", &[0x48, 0x8B, 0x03])
        .gprs(&[(RBX, DATA)])
        .data(&[1, 2, 3, 4, 5, 6, 7, 8]),
    Case::new("mov byte [rbx],al", &[0x88, 0x03]).gprs(&[(RAX, 0xAB), (RBX, DATA)]),
    Case::new("mov ax,[rbx]", &[0x66, 0x8B, 0x03])
        .gprs(&[(RBX, DATA)])
        .data(&[0x34, 0x12]),
    Case::new("mov rax,[rbx+rcx*4+8]", &[0x48, 0x8B, 0x44, 0x8B, 0x08])
        .gprs(&[(RBX, DATA), (RCX, 1)])
        .data(&[0; 16]),
    Case::new("add [rbx],rax", &[0x48, 0x01, 0x03])
        .gprs(&[(RAX, 1), (RBX, DATA)])
        .data(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    Case::new("push rbx", &[0x53]).gprs(&[(RBX, 0xCAFE_BABE)]),
    Case::new("pop rcx", &[0x59]).data(&[]),
    Case::new("xchg [rbx],rax", &[0x48, 0x87, 0x03])
        .gprs(&[(RAX, 0xAA), (RBX, DATA)])
        .data(&[0xBB, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("cmpxchg [rbx],rcx", &[0x48, 0x0F, 0xB1, 0x0B])
        .gprs(&[(RAX, 0xBB), (RCX, 0xCC), (RBX, DATA)])
        .data(&[0xBB, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("lea rax,[rbx+8]", &[0x48, 0x8D, 0x43, 0x08]).gprs(&[(RBX, DATA)]),
];

/// String operations and compare-exchange — three separate ACL2->Sail
/// translation errors lived here, all of them patched in the vendored model.
/// See `docs/backend-differences.md` §4 and §4c.
///
/// Every `REP` case below either does nothing or finishes in one step. That is
/// deliberate: hardware exposes no per-iteration flag state when the
/// single-step trap lands *between* iterations of a `REPE CMPS`, while both
/// models do, so a mid-loop CMPS is a legitimate backend difference rather than
/// a finding. Mid-loop `MOVS`/`STOS` set no flags and so are safe to compare.
static STRING: &[Case] = &[
    // CMPS subtracts [rDI] from [rSI]. Getting that backwards leaves ZF right
    // and CF/SF/AF/PF wrong, so only unequal operands can see it.
    Case::new("cmpsb (src > dst)", &[0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8)])
        .data(&[5, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("cmpsb (src < dst)", &[0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8)])
        .data(&[3, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("cmps qword", &[0x48, 0xA7])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8)])
        .data(&[5, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]),
    // CMPXCHG compares rAX with the destination, not the other way round —
    // again invisible when they are equal, which is all the suite used to have.
    Case::new("cmpxchg [rbx],rcx (ne)", &[0x48, 0x0F, 0xB1, 0x0B])
        .gprs(&[(RAX, 0xBB), (RCX, 0xCC), (RBX, DATA)])
        .data(&[0x11, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("cmpxchg rbx,rcx (ne)", &[0x48, 0x0F, 0xB1, 0xCB])
        .gprs(&[(RAX, 0xBB), (RCX, 0xCC), (RBX, 0x11)]),
    Case::new("cmpxchg bl,cl (ne)", &[0x0F, 0xB0, 0xCB])
        .gprs(&[(RAX, 0xBB), (RCX, 0xCC), (RBX, 0x11)]),
    // rCX == 0 on entry: no iteration at all, and RIP moves on. rCX == 1: one
    // iteration, then RIP moves on. Above that RIP parks on the instruction.
    Case::new("rep movsb (rcx=0)", &[0xF3, 0xA4])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 0)])
        .data(&[0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0]),
    Case::new("rep movsb (rcx=1)", &[0xF3, 0xA4])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 1)])
        .data(&[0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0]),
    Case::new("rep movsb (rcx=2)", &[0xF3, 0xA4])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 2)])
        .data(&[0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0]),
    // 0xF2 on MOVS and STOS is REP as well; ZF plays no part in either.
    Case::new("repne movsb (rcx=1)", &[0xF2, 0xA4])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 1)])
        .rflags(0x2 | FLAG_ZF)
        .data(&[0xAA, 0xBB, 0xCC, 0, 0, 0, 0, 0]),
    Case::new("rep stosb (rcx=0)", &[0xF3, 0xAA]).gprs(&[(RAX, 0x5A), (RDI, DATA), (RCX, 0)]),
    Case::new("rep stosb (rcx=1)", &[0xF3, 0xAA]).gprs(&[(RAX, 0x5A), (RDI, DATA), (RCX, 1)]),
    Case::new("rep stosb (rcx=2)", &[0xF3, 0xAA]).gprs(&[(RAX, 0x5A), (RDI, DATA), (RCX, 2)]),
    Case::new("repne stosb (rcx=1)", &[0xF2, 0xAA])
        .gprs(&[(RAX, 0x5A), (RDI, DATA), (RCX, 1)])
        .rflags(0x2 | FLAG_ZF),
    // REPE stops on rCX == 0 or ZF == 0, REPNE on rCX == 0 or ZF == 1.
    Case::new("repe cmpsb (rcx=0)", &[0xF3, 0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 0)])
        .data(&[7, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("repe cmpsb (rcx=1, eq)", &[0xF3, 0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 1)])
        .data(&[7, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("repe cmpsb (rcx=2, ne stops)", &[0xF3, 0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 2)])
        .data(&[7, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0]),
    Case::new("repne cmpsb (rcx=2, eq stops)", &[0xF2, 0xA6])
        .gprs(&[(RSI, DATA), (RDI, DATA + 8), (RCX, 2)])
        .data(&[7, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]),
];

/// Control transfer.
static BRANCH: &[Case] = &[
    Case::new("jmp rel32", &[0xE9, 0x10, 0x00, 0x00, 0x00]),
    Case::new("jz rel8 (taken)", &[0x74, 0x02]).rflags(0x2 | FLAG_ZF),
    Case::new("jz rel8 (not taken)", &[0x74, 0x02]),
    Case::new("jnz rel8 (taken)", &[0x75, 0x02]),
    Case::new("call rel32", &[0xE8, 0x05, 0x00, 0x00, 0x00]),
    Case::new("jmp rax", &[0xFF, 0xE0]).gprs(&[(RAX, CODE + 0x40)]),
    Case::new("loop rel8", &[0xE2, 0x02]).gprs(&[(RCX, 3)]),
];

/// Run one group of cases across every available pair of backends.
fn run_group(label: &str, cases: &[Case]) {
    let pairs = Backend::pairs();
    if pairs.is_empty() {
        println!("{label}: only one backend available, nothing to diff");
        return;
    }

    let mut failures = Vec::new();
    for (a, b) in pairs {
        let (mut compared, mut skipped) = (0, 0);
        for case in cases {
            match diff_case(case, a, b) {
                Ok(true) => compared += 1,
                Ok(false) => skipped += 1,
                Err(report) => failures.push(format!("[{} vs {}] {report}", a.name(), b.name())),
            }
        }
        println!(
            "{label} {} vs {}: {compared} agreed, {skipped} skipped",
            a.name(),
            b.name()
        );
    }

    assert!(
        failures.is_empty(),
        "{} divergence(s) in the {label} group:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn arithmetic_agrees_across_backends() {
    run_group("arith", ARITH);
}

#[test]
fn memory_agrees_across_backends() {
    run_group("memory", MEMORY);
}

#[test]
fn branches_agree_across_backends() {
    run_group("branch", BRANCH);
}

#[test]
fn string_ops_agree_across_backends() {
    run_group("string", STRING);
}

/// The backends must agree on the *starting* state too, or every later
/// comparison is meaningless.
#[test]
fn reset_states_agree() {
    for (a, b) in Backend::pairs() {
        let sa = AnyOracle::new(a).snapshot();
        let sb = AnyOracle::new(b).snapshot();
        let d = sa.diff(&sb);
        assert!(d.is_empty(), "{} vs {} reset states differ: {d:?}", a.name(), b.name());
    }
}

/// Sanity check on the harness itself: a deliberately different starting state
/// must be reported as a divergence, so a silent no-op suite cannot pass.
#[test]
fn harness_detects_divergence() {
    for (a, b) in Backend::pairs() {
        let mut x = AnyOracle::new(a);
        x.set_gpr(RAX, 1);
        let y = AnyOracle::new(b);
        assert!(
            !x.snapshot().diff(&y.snapshot()).is_empty(),
            "the diff harness failed to notice a known difference between {} and {}",
            a.name(),
            b.name()
        );
    }
}

/// What each available backend can hold, for the record — a hardware backend is
/// limited to the host CPU's actual registers.
#[test]
fn report_backend_capabilities() {
    for backend in Backend::available() {
        let cpu = AnyOracle::new(backend);
        println!(
            "  {:<6} vector: {} regs x {} bits",
            backend.name(),
            cpu.vector_regs(),
            cpu.vector_chunks() * 64
        );
    }
}
