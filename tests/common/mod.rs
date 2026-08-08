//! Backend-agnostic test suite.
//!
//! Every check here is written against the [`X86Oracle`] trait and the shared
//! semantic contract, so it must pass on *every* backend. `suite_for!` expands
//! it into a `mod` per backend; anything a single backend cannot honour lives
//! in that backend's own test file instead.

#![allow(dead_code)]

use x86_oracle::*;

pub const CODE: u64 = DEFAULT_CODE_ADDR;

/// Skip a check when the backend simply does not implement the encoding —
/// `Unimplemented` says nothing about semantics, so it is never a failure.
/// Returns false (and prints why) if the suite should stop checking.
#[macro_export]
macro_rules! skip_if_unimplemented {
    ($cpu:expr, $out:expr, $what:literal) => {
        if $out.fault_kind() == Some(x86_oracle::FaultKind::Unimplemented) {
            eprintln!(
                "  skip [{}] {}: {}",
                $cpu.backend_name(),
                $what,
                $cpu.fault_msg()
            );
            return;
        }
    };
}

/// The whole portable suite, generated for one backend type.
///
/// The three-argument form takes an availability predicate, for backends whose
/// usability depends on the environment (the KVM one needs `/dev/kvm` access).
/// When it is false every check reports a skip and passes, so `cargo test` on a
/// machine without that backend is quiet rather than red.
#[macro_export]
macro_rules! suite_for {
    ($name:ident, $ctor:expr) => {
        $crate::suite_for!($name, $ctor, true);
    };
    ($name:ident, $ctor:expr, $available:expr) => {
        mod $name {
            use x86_oracle::*;
            use $crate::common::CODE;
            use $crate::skip_if_unimplemented;

            /// False when this backend cannot run here; every test returns early.
            fn usable() -> bool {
                if $available {
                    return true;
                }
                eprintln!("  skip [{}] backend unavailable on this machine", stringify!($name));
                false
            }

            fn cpu() -> impl X86Oracle {
                let mut c = $ctor;
                c.set_rip(CODE);
                c
            }

            // ------------------------------------------------ reset contract

            #[test]
            fn reset_state_is_64bit_flat() {
                if !usable() {
                    return;
                }
                let c = $ctor;
                assert_ne!(c.get_msr(IA32_EFER) & EFER_LMA, 0, "not in long mode");
                assert_ne!(c.get_cr(4) & CR4_OSFXSR, 0, "SSE not enabled");
                assert_eq!(c.get_rflags() & 0x2, 0x2, "RFLAGS bit 1 must read as 1");
                assert_eq!(c.get_rip(), 0);
                for n in 0..16 {
                    assert_eq!(c.get_gpr(n), 0, "gpr {n} not zeroed");
                }
                for n in 0..ZMM_REGS as u32 {
                    assert_eq!(c.get_zmm(n), [0u64; ZMM_CHUNKS], "zmm{n} not zeroed");
                }
            }

            // ------------------------------------------------ ALU + flags

            #[test]
            fn add_reg_reg() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 5);
                c.set_gpr(RBX, 7);
                let out = c.step_bytes(&[0x48, 0x01, 0xD8]); // add rax, rbx
                assert!(out.is_retired(), "{out:?}");
                assert_eq!(c.get_gpr(RAX), 12);
                assert_eq!(c.get_rip(), CODE + 3);
                assert_eq!(c.get_rflags() & FLAG_CF, 0);
                assert_eq!(c.get_rflags() & FLAG_ZF, 0);
            }

            #[test]
            fn add_sets_carry_and_zero() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, u64::MAX);
                c.set_gpr(RBX, 1);
                assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                assert_eq!(c.get_gpr(RAX), 0);
                let fl = c.get_rflags();
                assert_ne!(fl & FLAG_CF, 0, "CF expected: rflags={fl:#x}");
                assert_ne!(fl & FLAG_ZF, 0, "ZF expected: rflags={fl:#x}");
            }

            #[test]
            fn add_sets_parity() {
                if !usable() {
                    return;
                }
                // 5 + 7 = 12 = 0b1100 -> two bits set -> even parity -> PF=1.
                // Catches backends that report stale/lazy flags.
                let mut c = cpu();
                c.set_gpr(RAX, 5);
                c.set_gpr(RBX, 7);
                assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                let fl = c.get_rflags();
                assert_ne!(fl & FLAG_PF, 0, "PF expected: rflags={fl:#x}");
            }

            #[test]
            fn add_sets_overflow_and_sign() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, i64::MAX as u64);
                c.set_gpr(RBX, 1);
                assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                assert_eq!(c.get_gpr(RAX), 1u64 << 63);
                let fl = c.get_rflags();
                assert_ne!(fl & FLAG_OF, 0, "OF expected: rflags={fl:#x}");
                assert_ne!(fl & FLAG_SF, 0, "SF expected: rflags={fl:#x}");
            }

            #[test]
            fn sub_borrow_and_sign() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 0);
                c.set_gpr(RBX, 1);
                assert!(c.step_bytes(&[0x48, 0x29, 0xD8]).is_retired()); // sub rax, rbx
                assert_eq!(c.get_gpr(RAX), u64::MAX);
                let fl = c.get_rflags();
                assert_ne!(fl & FLAG_SF, 0);
                assert_ne!(fl & FLAG_CF, 0, "borrow");
                assert_eq!(fl & FLAG_OF, 0);
            }

            #[test]
            fn adc_uses_carry_in() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 1);
                c.set_gpr(RBX, 1);
                c.set_rflags(0x2 | FLAG_CF);
                assert!(c.step_bytes(&[0x48, 0x11, 0xD8]).is_retired()); // adc rax, rbx
                assert_eq!(c.get_gpr(RAX), 3, "carry-in not honoured");
            }

            #[test]
            fn mov_imm64() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                let out =
                    c.step_bytes(&[0x48, 0xB8, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
                assert!(out.is_retired(), "{out:?}");
                assert_eq!(c.get_gpr(RAX), 0x1122_3344_5566_7788);
                assert_eq!(c.get_rip(), CODE + 10);
            }

            #[test]
            fn xor_zeroes_and_sets_zf() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 0xdead_beef);
                assert!(c.step_bytes(&[0x48, 0x31, 0xC0]).is_retired());
                assert_eq!(c.get_gpr(RAX), 0);
                assert_ne!(c.get_rflags() & FLAG_ZF, 0);
            }

            #[test]
            fn r8_to_r15_addressable() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(R9, 3);
                c.set_gpr(R10, 4);
                assert!(c.step_bytes(&[0x4D, 0x01, 0xD1]).is_retired()); // add r9, r10
                assert_eq!(c.get_gpr(R9), 7);
            }

            #[test]
            fn gpr_32bit_write_zero_extends() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, u64::MAX);
                c.set_gpr(RBX, 0x1_0000_0001);
                assert!(c.step_bytes(&[0x89, 0xD8]).is_retired()); // mov eax, ebx
                assert_eq!(c.get_gpr(RAX), 1, "upper 32 bits must clear");
            }

            #[test]
            fn imul_and_shift() {
                if !usable() {
                    return;
                }
                // NOTE the explicit scopes: a backend may allow only one live
                // oracle per process (Bochs does), so a portable test must drop
                // one before creating the next. Shadowing does NOT drop.
                {
                    let mut c = cpu();
                    c.set_gpr(RAX, 7);
                    c.set_gpr(RBX, 6);
                    let out = c.step_bytes(&[0x48, 0x0F, 0xAF, 0xC3]); // imul rax, rbx
                    skip_if_unimplemented!(c, out, "imul r64,r64");
                    assert!(out.is_retired(), "{out:?}");
                    assert_eq!(c.get_gpr(RAX), 42);
                }
                {
                    let mut c = cpu();
                    c.set_gpr(RAX, 1);
                    let out = c.step_bytes(&[0x48, 0xC1, 0xE0, 0x04]); // shl rax, 4
                    skip_if_unimplemented!(c, out, "shl r64,imm8");
                    assert!(out.is_retired(), "{out:?}");
                    assert_eq!(c.get_gpr(RAX), 0x10);
                }
            }

            // ------------------------------------------------ memory

            #[test]
            fn store_load_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 0xCAFE_F00D_1234_5678);
                c.set_gpr(RBX, 0x2_0000);
                assert!(c.step_bytes(&[0x48, 0x89, 0x03]).is_retired()); // mov [rbx], rax
                assert_eq!(c.read_mem_u64(0x2_0000), 0xCAFE_F00D_1234_5678);
                assert!(c.step_bytes(&[0x48, 0x8B, 0x0B]).is_retired()); // mov rcx, [rbx]
                assert_eq!(c.get_gpr(RCX), 0xCAFE_F00D_1234_5678);
            }

            #[test]
            fn direct_memory_access_matches_instructions() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.write_mem_u64(0x3_0000, 0x1111_2222_3333_4444);
                c.set_gpr(RBX, 0x3_0000);
                assert!(c.step_bytes(&[0x48, 0x8B, 0x03]).is_retired()); // mov rax, [rbx]
                assert_eq!(c.get_gpr(RAX), 0x1111_2222_3333_4444);
            }

            #[test]
            fn rewriting_code_at_an_executed_address_takes_effect() {
                // `step_bytes` writes code at RIP and steps, so reusing one
                // address for case after case is the normal idiom. A backend
                // that caches decoded instructions by address must notice the
                // host rewriting them, or every case after the first silently
                // re-runs the first one's opcode while memory shows the new
                // bytes.
                if !usable() {
                    return;
                }
                let mut c = cpu();
                let at = c.get_rip();
                assert!(c.step_bytes(&[0x48, 0xFF, 0xC0]).is_retired()); // inc rax
                assert_eq!(c.get_gpr(RAX), 1);
                c.set_rip(at);
                assert!(c.step_bytes(&[0x48, 0xFF, 0xC8]).is_retired()); // dec rax
                assert_eq!(c.get_gpr(RAX), 0, "the second encoding is what must have run");
            }

            #[test]
            fn byte_and_word_accesses() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.write_mem(0x3_1000, &[0x11, 0x22, 0x33, 0x44]);
                c.set_gpr(RBX, 0x3_1000);
                assert!(c.step_bytes(&[0x0F, 0xB6, 0x03]).is_retired()); // movzx eax, byte [rbx]
                assert_eq!(c.get_gpr(RAX), 0x11);
                let mut buf = [0u8; 4];
                c.read_mem(0x3_1000, &mut buf);
                assert_eq!(buf, [0x11, 0x22, 0x33, 0x44]);
            }

            #[test]
            fn push_pop_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RSP, 0x8_0000);
                c.set_gpr(RBX, 0xCAFE_BABE);
                assert!(c.step_bytes(&[0x53]).is_retired()); // push rbx
                assert_eq!(c.get_gpr(RSP), 0x8_0000 - 8);
                assert_eq!(c.read_mem_u64(0x8_0000 - 8), 0xCAFE_BABE);
                assert!(c.step_bytes(&[0x59]).is_retired()); // pop rcx
                assert_eq!(c.get_gpr(RCX), 0xCAFE_BABE);
                assert_eq!(c.get_gpr(RSP), 0x8_0000);
            }

            #[test]
            fn unwritten_memory_reads_zero() {
                if !usable() {
                    return;
                }
                let c = $ctor;
                assert_eq!(c.read_mem_u64(0x10_0000), 0);
            }

            #[test]
            fn fs_relative_addressing_uses_fs_base() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_seg_base(SEG_FS, 0x5_0000);
                c.write_mem_u64(0x5_0000, 0x1111_2222_3333_4444);
                // mov rax, fs:[0]
                let out = c.step_bytes(&[0x64, 0x48, 0x8B, 0x04, 0x25, 0, 0, 0, 0]);
                skip_if_unimplemented!(c, out, "fs-relative load");
                assert!(out.is_retired(), "{out:?}");
                assert_eq!(c.get_gpr(RAX), 0x1111_2222_3333_4444);
                assert_eq!(c.get_seg_base(SEG_FS), 0x5_0000, "base must read back");
            }

            // ------------------------------------------------ control flow

            #[test]
            fn conditional_branch_taken_and_not_taken() {
                if !usable() {
                    return;
                }
                // One oracle at a time — see the note in imul_and_shift.
                {
                    let mut c = cpu();
                    assert!(c.step_bytes(&[0x48, 0x31, 0xC0]).is_retired()); // xor -> ZF=1
                    assert!(c.step_bytes(&[0x74, 0x02]).is_retired()); // jz +2
                    assert_eq!(c.get_rip(), CODE + 3 + 2 + 2, "branch not taken");
                }
                {
                    let mut c = cpu();
                    c.set_gpr(RAX, 1);
                    c.set_gpr(RBX, 2);
                    assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired()); // ZF=0
                    assert!(c.step_bytes(&[0x74, 0x02]).is_retired());
                    assert_eq!(c.get_rip(), CODE + 3 + 2, "branch wrongly taken");
                }
            }

            #[test]
            fn call_ret_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RSP, 0x8_0000);
                // call +5 -> pushes CODE+5, jumps to CODE+10
                assert!(c.step_bytes(&[0xE8, 0x05, 0x00, 0x00, 0x00]).is_retired());
                assert_eq!(c.get_rip(), CODE + 10);
                assert_eq!(c.read_mem_u64(0x8_0000 - 8), CODE + 5);
                assert!(c.step_bytes(&[0xC3]).is_retired()); // ret
                assert_eq!(c.get_rip(), CODE + 5);
                assert_eq!(c.get_gpr(RSP), 0x8_0000);
            }

            #[test]
            fn jmp_rel32() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                assert!(c.step_bytes(&[0xE9, 0x10, 0x00, 0x00, 0x00]).is_retired());
                assert_eq!(c.get_rip(), CODE + 5 + 0x10);
            }

            // ------------------------------------------------ SSE

            #[test]
            fn movups_load_store() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.write_mem_u64(0x2_0000, 0x1122_3344_5566_7788);
                c.write_mem_u64(0x2_0008, 0x99AA_BBCC_DDEE_FF00);
                // movups xmm1, [0x20000]
                let out = c.step_bytes(&[0x0F, 0x10, 0x0C, 0x25, 0x00, 0x00, 0x02, 0x00]);
                skip_if_unimplemented!(c, out, "movups xmm,m128");
                assert!(out.is_retired(), "{out:?}");
                let z = c.get_zmm(1);
                assert_eq!(z[0], 0x1122_3344_5566_7788);
                assert_eq!(z[1], 0x99AA_BBCC_DDEE_FF00);
                // movups [0x40000], xmm1
                let out = c.step_bytes(&[0x0F, 0x11, 0x0C, 0x25, 0x00, 0x00, 0x04, 0x00]);
                assert!(out.is_retired(), "{out:?}");
                assert_eq!(c.read_mem_u64(0x4_0000), 0x1122_3344_5566_7788);
                assert_eq!(c.read_mem_u64(0x4_0008), 0x99AA_BBCC_DDEE_FF00);
            }

            #[test]
            fn zmm_accessors_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = $ctor;
                // Only the width this backend can hold: a hardware backend has
                // exactly the host CPU's registers, no more.
                let chunks = c.vector_chunks();
                let reg = (c.vector_regs() - 1) as u32;
                let mut pat = vec![0u64; ZMM_CHUNKS];
                for i in 0..chunks {
                    pat[i] = 0x0101_0101_0101_0101u64 * (i as u64 + 1);
                }
                c.set_zmm(reg, &pat);
                assert_eq!(c.get_zmm(reg).to_vec(), pat, "backend holds {chunks} chunks");
                // a short slice zeroes the remaining chunks
                c.set_zmm(reg, &[0xAA]);
                let z = c.get_zmm(reg);
                assert_eq!(z[0], 0xAA);
                assert!(z[1..].iter().all(|&x| x == 0));
            }

            // ------------------------------------------------ faults

            #[test]
            fn ud2_is_undefined_opcode() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                let out = c.step_bytes(&[0x0F, 0x0B]);
                assert_eq!(
                    out.fault_kind(),
                    Some(FaultKind::UndefinedOpcode),
                    "msg: {}",
                    c.fault_msg()
                );
            }

            #[test]
            fn div_by_zero_is_divide_error() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RAX, 100);
                c.set_gpr(RDX, 0);
                c.set_gpr(RBX, 0);
                let out = c.step_bytes(&[0x48, 0xF7, 0xF3]); // div rbx
                skip_if_unimplemented!(c, out, "div r64");
                assert_eq!(
                    out.fault_kind(),
                    Some(FaultKind::DivideError),
                    "msg: {}",
                    c.fault_msg()
                );
            }

            #[test]
            fn fault_msg_clears_after_a_retired_step() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                assert!(!c.step_bytes(&[0x0F, 0x0B]).is_retired()); // ud2
                assert!(!c.fault_msg().is_empty());
                c.set_rip(CODE + 0x100);
                assert!(c.step_bytes(&[0x90]).is_retired(), "nop after fault"); // nop
                assert_eq!(c.fault_msg(), "");
            }

            #[test]
            fn execution_continues_after_a_fault() {
                if !usable() {
                    return;
                }
                // A fault must leave the oracle usable — no latched error state.
                let mut c = cpu();
                assert!(!c.step_bytes(&[0x0F, 0x0B]).is_retired());
                c.set_rip(CODE + 0x200);
                c.set_gpr(RAX, 2);
                c.set_gpr(RBX, 3);
                assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                assert_eq!(c.get_gpr(RAX), 5);
            }

            // ------------------------------------------------ state API

            #[test]
            fn rip_accepts_canonical_addresses() {
                if !usable() {
                    return;
                }
                let mut c = $ctor;
                c.set_rip(0x0000_7FFF_FFFF_FFFF);
                assert_eq!(c.get_rip(), 0x0000_7FFF_FFFF_FFFF);
                c.set_rip(0xFFFF_8000_0000_0000);
                assert_eq!(c.get_rip(), 0xFFFF_8000_0000_0000);
            }

            #[test]
            fn snapshot_restore_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                c.set_gpr(RDI, 42);
                c.set_gpr(R15, 0xFFFF);
                c.set_zmm(3, &[1, 2, 3, 4, 5, 6, 7, 8]);
                let snap = c.snapshot();
                assert_eq!(snap.gpr[RDI as usize], 42);
                assert_eq!(snap.rip, CODE);

                // perturb, then restore
                c.set_gpr(RDI, 0);
                c.set_rip(CODE + 0x50);
                c.set_zmm(3, &[0]);
                c.restore(&snap);
                let back = c.snapshot();
                assert_eq!(back, snap, "restore did not reproduce the snapshot");
            }

            #[test]
            fn snapshot_diff_reports_changes() {
                if !usable() {
                    return;
                }
                let mut c = cpu();
                let before = c.snapshot();
                c.set_gpr(RAX, 5);
                c.set_gpr(RBX, 7);
                assert!(c.step_bytes(&[0x48, 0x01, 0xD8]).is_retired());
                let after = c.snapshot();
                let d = before.diff(&after);
                assert!(!d.is_empty());
                assert!(d.iter().any(|s| s.starts_with("rax")), "diff: {d:?}");
                assert!(!before.eq_integer(&after));
            }

            #[test]
            fn msr_roundtrip() {
                if !usable() {
                    return;
                }
                let mut c = $ctor;
                c.set_msr(IA32_LSTAR, 0xFFFF_8000_1234_5678);
                assert_eq!(c.get_msr(IA32_LSTAR), 0xFFFF_8000_1234_5678);
                // an MSR no backend models: writes ignored, reads zero
                c.set_msr(0x1234_5678, 0xDEAD);
                assert_eq!(c.get_msr(0x1234_5678), 0);
            }
        }
    };
}
