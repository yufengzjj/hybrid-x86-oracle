//! The descriptor-table contract: every backend reports the same GDTR/IDTR
//! bases and limits, the same TR selector, and a null LDTR — so a differential
//! harness can diff `SGDT`/`SIDT`/`STR`/`SLDT` against all of them with one
//! seed. On the hardware backends the tables are real (interrupt delivery
//! walks them); Bochs mirrors only the register values.

use x86_oracle::{
    AnyOracle, Backend, FaultKind, X86Oracle, GDT_ADDR, GDT_LIMIT, IDT_ADDR, IDT_LIMIT, RAX, RBX,
    SEL_TSS,
};

const CODE: u64 = 0x10_0000;
const DATA: u64 = 0x8000;

/// `Ok(true)` retired, `Ok(false)` the backend does not implement the
/// instruction (skip — Sail has none of this group), anything else fails.
#[track_caller]
fn steps(cpu: &mut AnyOracle, name: &str, code: &[u8]) -> bool {
    cpu.set_rip(CODE);
    match cpu.step_bytes(code) {
        out if out.is_retired() => true,
        out
            if matches!(
                out.fault_kind(),
                Some(FaultKind::Unimplemented | FaultKind::UndefinedOpcode)
            ) =>
        {
            println!("{name}: not implemented — skipped");
            false
        }
        out => panic!("{name} did not retire: {out:?}"),
    }
}

/// Fails when nothing was verified even though a backend that implements the
/// group is compiled in — a Sail-only build legitimately skips everything, and
/// this keeps that from silently extending to builds that should not.
#[track_caller]
fn guard_against_vacuous_pass(verified: usize) {
    if cfg!(feature = "bochs") {
        assert!(verified > 0, "Bochs is compiled in, yet no backend verified the contract");
    } else if verified == 0 {
        println!("warning: no available backend implements the group; nothing was verified");
    }
}

#[test]
fn descriptor_table_registers_agree_across_backends() {
    let mut verified = 0;
    for b in Backend::available() {
        let mut cpu = AnyOracle::new(b);
        let name = b.name();

        cpu.set_gpr(RBX, DATA);
        // 0f 01 03 — sgdt [rbx]: limit word at [m], base qword at [m+2].
        if steps(&mut cpu, name, &[0x0f, 0x01, 0x03]) {
            assert_eq!(cpu.read_mem_u64(DATA) & 0xFFFF, GDT_LIMIT as u64, "{name}: GDT limit");
            assert_eq!(cpu.read_mem_u64(DATA + 2), GDT_ADDR, "{name}: GDT base");
            verified += 1;
        }
        // 0f 01 0b — sidt [rbx]
        if steps(&mut cpu, name, &[0x0f, 0x01, 0x0b]) {
            assert_eq!(cpu.read_mem_u64(DATA) & 0xFFFF, IDT_LIMIT as u64, "{name}: IDT limit");
            assert_eq!(cpu.read_mem_u64(DATA + 2), IDT_ADDR, "{name}: IDT base");
        }
        // 0f 00 c8 — str eax
        cpu.set_gpr(RAX, u64::MAX);
        if steps(&mut cpu, name, &[0x0f, 0x00, 0xc8]) {
            assert_eq!(cpu.get_gpr(RAX), SEL_TSS as u64, "{name}: TR selector");
        }
        // 0f 00 c0 — sldt eax
        cpu.set_gpr(RAX, u64::MAX);
        if steps(&mut cpu, name, &[0x0f, 0x00, 0xc0]) {
            assert_eq!(cpu.get_gpr(RAX), 0, "{name}: LDTR must be the null selector");
        }
        // 48 0f 01 e0 — smsw rax: CR0, which every backend boots identically.
        if steps(&mut cpu, name, &[0x48, 0x0f, 0x01, 0xe0]) {
            assert_eq!(cpu.get_gpr(RAX), 0x8005_0033, "{name}: CR0");
        }
    }
    guard_against_vacuous_pass(verified);
}

/// The alignment happens at init and nothing disturbs it afterwards: a fault —
/// whose aborted delivery walks the unbacked IDT on Bochs — must not move the
/// tables, and a fresh instance mid-suite must re-establish the contract.
#[test]
fn contract_survives_faults_and_reinit() {
    let mut verified = 0;
    for b in Backend::available() {
        let mut cpu = AnyOracle::new(b);
        let name = b.name();
        cpu.set_gpr(RBX, DATA);
        if !steps(&mut cpu, name, &[0x0f, 0x01, 0x03]) {
            continue;
        }
        verified += 1;
        let first = cpu.read_mem_u64(DATA + 2);

        // 0f 0b — ud2, then read the base back.
        cpu.set_rip(CODE);
        let out = cpu.step_bytes(&[0x0f, 0x0b]);
        assert!(!out.is_retired(), "{name}: ud2 retired: {out:?}");
        cpu.set_gpr(RBX, DATA);
        assert!(steps(&mut cpu, name, &[0x0f, 0x01, 0x03]));
        assert_eq!(cpu.read_mem_u64(DATA + 2), first, "{name}: a fault moved the tables");

        drop(cpu);
        let mut cpu = AnyOracle::new(b);
        cpu.set_gpr(RBX, DATA);
        assert!(steps(&mut cpu, name, &[0x0f, 0x01, 0x03]));
        assert_eq!(cpu.read_mem_u64(DATA + 2), first, "{name}: a fresh instance lost the alignment");
    }
    guard_against_vacuous_pass(verified);
}
