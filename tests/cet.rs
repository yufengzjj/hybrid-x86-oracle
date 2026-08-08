//! CET shadow stacks: the opt-in reset state added by
//! [`X86Oracle::enable_shadow_stack`], and the instructions it unlocks.
//!
//! Bochs is the only backend that models CET, so these run there alone. On a
//! backend without it `enable_shadow_stack` returns false and each test skips —
//! the same contract a differential harness follows.

#![cfg(feature = "bochs")]

use x86_oracle::{BochsOracle, FaultKind, ShadowStackError, StepOutcome, X86Oracle, RAX, RBX};

/// A 2 MiB frame of its own, clear of the low addresses tests use for data.
const SHSTK: u64 = 0x40_0000;
const SHSTK_LEN: u64 = 2 << 20;
const CODE: u64 = 0x10_0000;

fn cet_cpu() -> Option<BochsOracle> {
    let mut cpu = BochsOracle::new();
    match cpu.enable_shadow_stack(SHSTK, SHSTK_LEN) {
        Ok(()) => {}
        // No CET in this build: skip, the same as an unimplemented instruction.
        // Say so — a silent skip here would turn every test in this file into a
        // vacuous pass, which is indistinguishable from the feature working.
        Err(ShadowStackError::Unsupported) => {
            eprintln!("SKIP: this Bochs build has no CET");
            return None;
        }
        // Anything else is this fixture getting its own range wrong, which must
        // fail loudly instead of quietly skipping every test in the file.
        Err(e) => panic!("the fixture's range was refused: {e}"),
    }
    cpu.set_rip(CODE);
    Some(cpu)
}

/// Assert *which* exception a step raised, not merely that it did not retire.
/// Every negative case here is about telling one fault from another: a `#PF`
/// where `#CP` was expected means the case set the machine up wrong and proved
/// nothing about CET.
#[track_caller]
fn assert_fault(out: StepOutcome, want: FaultKind) {
    assert_eq!(out.fault_kind(), Some(want), "{out:?}");
}

#[test]
fn rdsspq_reads_the_shadow_stack_pointer() {
    let Some(mut cpu) = cet_cpu() else { return };
    let top = SHSTK + 0x1000;
    cpu.set_ssp(top);
    // f3 48 0f 1e c8 — rdsspq rax
    assert!(cpu.step_bytes(&[0xf3, 0x48, 0x0f, 0x1e, 0xc8]).is_retired());
    assert_eq!(cpu.get_gpr(RAX), top, "with CET off this is a NOP; enabling it must make it a read");
}

/// `call rel32` with a NON-ZERO displacement: Bochs skips the shadow-stack push
/// when the displacement is 0 (`call $+5`, the PIC "get my own address" idiom),
/// so a zero-displacement call would silently prove nothing here.
const CALL_NEXT: [u8; 5] = [0xe8, 0x05, 0x00, 0x00, 0x00];

#[test]
fn call_and_ret_maintain_the_shadow_stack() {
    // The whole point of the feature: CALL pushes the return address to the
    // shadow stack as well, and RET pops and compares it.
    let Some(mut cpu) = cet_cpu() else { return };
    let top = SHSTK + 0x1000;
    cpu.set_ssp(top);
    cpu.set_gpr(x86_oracle::RSP, 0x8000);
    assert!(cpu.step_bytes(&CALL_NEXT).is_retired());
    assert_eq!(cpu.get_ssp(), top - 8, "CALL must push a shadow-stack frame");
    assert_eq!(cpu.read_mem_u64(top - 8), CODE + 5, "the return address goes on both stacks");
    cpu.set_rip(CODE + 10);
    assert!(cpu.step_bytes(&[0xc3]).is_retired(), "RET must accept a matching shadow-stack entry");
    assert_eq!(cpu.get_ssp(), top);
}

#[test]
fn ret_faults_when_the_return_address_was_forged() {
    // The attack CET exists to stop: the ordinary stack says one thing, the
    // shadow stack another, and RET raises #CP (vector 21).
    let Some(mut cpu) = cet_cpu() else { return };
    let top = SHSTK + 0x1000;
    cpu.set_ssp(top);
    cpu.set_gpr(x86_oracle::RSP, 0x8000);
    assert!(cpu.step_bytes(&CALL_NEXT).is_retired());
    cpu.write_mem_u64(0x8000 - 8, 0xdead_0000); // rewrite the ordinary return address
    cpu.set_rip(CODE + 10);
    // #CP specifically: a #PF here would mean the stacks were mis-set-up and the
    // shadow-stack comparison never happened.
    assert_fault(cpu.step_bytes(&[0xc3]), FaultKind::ControlProtection);
}

#[test]
fn wrss_writes_the_shadow_stack() {
    let Some(mut cpu) = cet_cpu() else { return };
    let at = SHSTK + 0x800;
    cpu.set_gpr(RBX, at);
    cpu.set_gpr(RAX, 0x1122_3344_5566_7788);
    // 48 0f 38 f6 03 — wrssq [rbx], rax
    assert!(cpu.step_bytes(&[0x48, 0x0f, 0x38, 0xf6, 0x03]).is_retired());
    assert_eq!(cpu.read_mem_u64(at), 0x1122_3344_5566_7788);
}

#[test]
fn an_ordinary_store_into_a_shadow_stack_faults() {
    // What the page attribute buys, and the reason WRSS has to exist. Its own
    // instance: this oracle has no IDT, so an unresolved fault leaves the CPU
    // unable to retire anything further — a fault is the END of a case.
    let Some(mut cpu) = cet_cpu() else { return };
    cpu.set_gpr(RBX, SHSTK + 0x800);
    cpu.set_gpr(RAX, 0x1122_3344_5566_7788);
    // #PF, not #CP: the page attribute is what refuses the store.
    assert_fault(cpu.step_bytes(&[0x48, 0x89, 0x03]), FaultKind::PageFault); // mov [rbx], rax
}

#[test]
fn rstorssp_saveprevssp_round_trip() {
    // The token dance a context switch performs, and the reference this crate
    // exists to provide: a restore token at A holds (A+8)|1, RSTORSSP replaces
    // it with a previous-SSP token (old|3), and SAVEPREVSSP pops that to plant a
    // fresh restore token (old|1) on the stack being left behind.
    let Some(mut cpu) = cet_cpu() else { return };
    let (a, old) = (SHSTK + 0x100, SHSTK + 0x1000);
    cpu.set_ssp(old);
    cpu.set_gpr(RBX, a);
    cpu.write_mem_u64(a, (a + 8) | 1);
    // f3 0f 01 2b — rstorssp [rbx]
    assert!(cpu.step_bytes(&[0xf3, 0x0f, 0x01, 0x2b]).is_retired());
    assert_eq!(cpu.get_ssp(), a);
    assert_eq!(cpu.read_mem_u64(a), old | 3);
    assert_eq!(cpu.get_rflags() & 1, 0, "CF reports a 4-byte alignment hole; there is none here");
    // f3 0f 01 ea — saveprevssp
    assert!(cpu.step_bytes(&[0xf3, 0x0f, 0x01, 0xea]).is_retired());
    assert_eq!(cpu.get_ssp(), a + 8);
    assert_eq!(cpu.read_mem_u64(old - 8), old | 1);
}

#[test]
fn rstorssp_rejects_a_token_that_does_not_name_its_own_slot() {
    let Some(mut cpu) = cet_cpu() else { return };
    let a = SHSTK + 0x100;
    cpu.set_ssp(SHSTK + 0x1000);
    cpu.set_gpr(RBX, a);
    cpu.write_mem_u64(a, (a + 16) | 1);
    // The self-reference check rejects it, and does so as #CP — the token is on
    // a perfectly valid shadow-stack page, so a #PF would be the wrong answer.
    assert_fault(cpu.step_bytes(&[0xf3, 0x0f, 0x01, 0x2b]), FaultKind::ControlProtection);
    assert_eq!(cpu.read_mem_u64(a), (a + 16) | 1, "a rejected switch must not have written");
}

#[test]
fn incssp_unwinds_by_slots_and_only_the_low_byte_counts() {
    let Some(mut cpu) = cet_cpu() else { return };
    let top = SHSTK + 0x1000;
    cpu.set_ssp(top);
    cpu.set_gpr(RAX, 0x102); // 2 slots, not 258
    // f3 48 0f ae e8 — incsspq rax
    assert!(cpu.step_bytes(&[0xf3, 0x48, 0x0f, 0xae, 0xe8]).is_retired());
    assert_eq!(cpu.get_ssp(), top + 16);
}

#[test]
fn a_shadow_stack_may_not_land_on_the_page_tables() {
    // The identity map's own tables live *inside* the mapped address space, so
    // the address-space limit alone does not exclude them. Accepting such a
    // range would put a shadow stack on top of PML4[0]: the first CALL
    // overwrites it, and the walk keeps hitting stale TLB entries, so the
    // machine runs on and dies somewhere unrelated much later.
    // One instance only — Bochs is a singleton and a second `new()` would block
    // until this one drops.
    let mut cpu = BochsOracle::new();
    let pt = BochsOracle::RESERVED_START;
    let bad = Err(ShadowStackError::BadRange);
    if cpu.enable_shadow_stack(pt, 0x1000) == Err(ShadowStackError::Unsupported) {
        return; // no CET in this build
    }
    // BadRange, not Unsupported: the backend has CET, it is the argument that is
    // wrong, and a caller must be able to tell those apart.
    assert_eq!(cpu.enable_shadow_stack(pt, 0x1000), bad);
    // Whole frames get converted, so a range that merely spills into theirs is
    // refused as well.
    assert_eq!(cpu.enable_shadow_stack(pt - 8, 16), bad);
    assert_eq!(cpu.enable_shadow_stack(BochsOracle::ADDRESS_LIMIT, 0x1000), bad);
    assert_eq!(cpu.enable_shadow_stack(SHSTK, 0), bad);
    assert_eq!(cpu.get_cr(4) & (1 << 23), 0, "a refused range must not have set CR4.CET");
    assert_eq!(cpu.enable_shadow_stack(SHSTK, SHSTK_LEN), Ok(()), "a good range still works");
}

#[test]
fn shadow_stack_stays_off_unless_asked() {
    // The reset-state contract: nothing above happens to a plain instance, so
    // every existing test keeps its call/ret behaviour.
    let mut cpu = BochsOracle::new();
    cpu.set_rip(CODE);
    cpu.set_gpr(RAX, 0x1234);
    assert!(cpu.step_bytes(&[0xf3, 0x48, 0x0f, 0x1e, 0xc8]).is_retired());
    assert_eq!(cpu.get_gpr(RAX), 0x1234, "with CET off RDSSPQ is architecturally a NOP");
    // Everything else in the group is #UD while CR4.CET is clear — the ISA's
    // answer for "disabled", which is only distinguishable from "this build has
    // no CET at all" because the extension IS enabled in the CPU model.
    assert_fault(cpu.step_bytes(&[0xf3, 0x0f, 0x01, 0xea]), FaultKind::UndefinedOpcode); // saveprevssp
    cpu.set_rip(CODE);
    cpu.set_gpr(RBX, 0x9000);
    assert_fault(cpu.step_bytes(&[0x48, 0x0f, 0x38, 0xf6, 0x03]), FaultKind::UndefinedOpcode); // wrssq
}
