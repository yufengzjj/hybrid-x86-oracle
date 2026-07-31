//! Tests for behaviour specific to the Sail backend: multi-instance
//! independence (the Bochs core is one CPU per process), the model's own
//! quirks, and its `Unimplemented` reporting.
#![cfg(feature = "sail")]

use x86_oracle::*;

const CODE: u64 = DEFAULT_CODE_ADDR;

fn cpu() -> SailOracle {
    let mut c = SailOracle::new();
    c.set_rip(CODE);
    c
}

#[test]
fn two_oracles_are_independent() {
    let mut a = cpu();
    let mut b = cpu();
    a.set_gpr(RAX, 1);
    a.set_gpr(RBX, 2);
    b.set_gpr(RAX, 100);
    b.set_gpr(RBX, 200);
    assert!(a.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
    assert!(b.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
    assert_eq!(a.get_gpr(RAX), 3);
    assert_eq!(b.get_gpr(RAX), 300);
}

#[test]
fn memory_is_per_instance() {
    let mut a = SailOracle::new();
    let b = SailOracle::new();
    a.write_mem_u64(0x6_0000, 0x1234);
    assert_eq!(a.read_mem_u64(0x6_0000), 0x1234);
    assert_eq!(b.read_mem_u64(0x6_0000), 0);
    assert!(a.is_mapped(0x6_0000));
    assert!(!b.is_mapped(0x6_0000));
}

#[test]
fn create_use_drop_interleaved() {
    let mut a = cpu();
    assert!(a.step_bytes(&[0x90]).is_retired());
    {
        let mut b = cpu();
        assert!(b.step_bytes(&[0x90]).is_retired());
    }
    assert!(a.step_bytes(&[0x90]).is_retired());
    assert_eq!(a.get_rip(), CODE + 2);
}

#[test]
fn oracles_usable_across_threads() {
    use std::sync::Arc;

    let mut c = cpu();
    c.set_gpr(RAX, 5);
    let handle = std::thread::spawn(move || {
        c.set_gpr(RBX, 7);
        assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
        c.get_gpr(RAX)
    });
    assert_eq!(handle.join().unwrap(), 12);

    let shared = Arc::new(cpu());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&shared);
            std::thread::spawn(move || c.get_rip())
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), CODE);
    }

    let threads: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let mut c = cpu();
                c.set_gpr(RAX, i);
                c.set_gpr(RBX, 1);
                for _ in 0..10 {
                    assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                    c.set_rip(CODE);
                }
                c.get_gpr(RAX)
            })
        })
        .collect();
    for (i, t) in threads.into_iter().enumerate() {
        assert_eq!(t.join().unwrap(), i as u64 + 10);
    }
}

#[test]
fn unmapped_vs_wrote_zero() {
    let mut c = SailOracle::new();
    assert!(!c.is_mapped(0x7_0000_0000));
    c.write_mem_byte(0x7_0000_0000, 0);
    assert!(c.is_mapped(0x7_0000_0000));
}

#[test]
fn cs_l_bit_set_at_reset() {
    // CS.L lives in the hidden attribute word in this model; init sets it.
    let c = SailOracle::new();
    assert_ne!(c.get_seg_attr(SEG_CS) & (1 << 9), 0);
}

#[test]
fn syscall_needs_efer_sce() {
    // Without EFER.SCE, SYSCALL #UDs (architecturally correct)…
    let mut c = cpu();
    let out = c.step_bytes(&[0x0F, 0x05]);
    assert_eq!(out.fault_kind(), Some(FaultKind::UndefinedOpcode), "{out:?}");

    // …with it, the model reaches its application-view SYSCALL stub.
    let mut c = cpu();
    let efer = c.get_msr(IA32_EFER);
    c.set_msr(IA32_EFER, efer | EFER_SCE);
    assert_eq!(c.step_bytes(&[0x0F, 0x05]), StepOutcome::Syscall);
}

#[test]
fn movsb_copies_and_advances() {
    let mut c = cpu();
    c.set_gpr(RSI, 0x2_0000);
    c.set_gpr(RDI, 0x3_0000);
    c.write_mem(0x2_0000, &[0xAB]);
    assert!(c.step_bytes(&[0xA4]).is_retired());
    assert_eq!(c.read_mem_byte(0x3_0000), 0xAB);
    assert_eq!(c.get_gpr(RSI), 0x2_0001);
    assert_eq!(c.get_gpr(RDI), 0x3_0001);
    assert_eq!(c.get_rip(), CODE + 1);
}

#[test]
fn rep_movsb_runs_one_iteration_per_step() {
    // The ACL2 model executes one REP iteration per step, RIP parked on the
    // instruction. KNOWN MODEL QUIRK (faithfully oracled): the translated
    // termination test reads rflags.ZF where upstream ACL2 uses
    // `(zf-spec counter)`, so with ZF==0 RIP never advances and RCX wraps past
    // zero. Drive REP loops per iteration and stop on RCX==0 yourself.
    let mut c = cpu();
    c.set_gpr(RSI, 0x2_0000);
    c.set_gpr(RDI, 0x3_0000);
    c.set_gpr(RCX, 4);
    c.write_mem(0x2_0000, &[1, 2, 3, 4]);
    c.write_mem(CODE, &[0xF3, 0xA4]); // rep movsb
    for remaining in (0..4u64).rev() {
        assert!(c.step().is_retired());
        assert_eq!(c.get_gpr(RCX), remaining);
        assert_eq!(c.get_rip(), CODE, "RIP parked during REP");
    }
    let mut dst = [0u8; 4];
    c.read_mem(0x3_0000, &mut dst);
    assert_eq!(dst, [1, 2, 3, 4]);
}

#[test]
fn unimplemented_opcodes_are_flagged_not_faulted() {
    // The translated snapshot has no CPUID and no integer SIMD. These must
    // report Unimplemented so diff campaigns can exclude them, rather than
    // masquerading as #UD.
    for (name, code) in [
        ("cpuid", vec![0x0F, 0xA2]),
        ("pxor xmm0,xmm0", vec![0x66, 0x0F, 0xEF, 0xC0]),
    ] {
        let mut c = cpu();
        let out = c.step_bytes(&code);
        assert_eq!(
            out.fault_kind(),
            Some(FaultKind::Unimplemented),
            "{name}: {out:?}"
        );
        assert!(!out.is_comparable(), "{name} must not be diffed");
    }
}

#[test]
fn rflags_upper_bits_ignored() {
    let mut c = SailOracle::new();
    c.set_rflags(0xFFFF_FFFF_0000_0002);
    assert_eq!(c.get_rflags() >> 32, 0, "model keeps RFLAGS 32-bit");
}
