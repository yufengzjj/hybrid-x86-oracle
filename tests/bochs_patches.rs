//! Regression cases for what this crate fixes in, or demands of, the vendored
//! Bochs — the things a re-vendor or a CPU-model change can silently undo.
//!
//! `docs/backend-differences.md` §2, §4g and §4i are the prose; these are the
//! pins. Bochs-only on purpose: no other backend here executes AVX-512 (Sail
//! has no vector ISA, and the CI hosts have no AVX-512 silicon), so there is
//! nothing to differ against — these assert against the SDM directly.
//!
//! One `BochsOracle` per test: the core is a process-wide singleton.
#![cfg(feature = "bochs")]

use x86_oracle::{BochsOracle, X86Oracle, ZMM_CHUNKS, RAX, RBX, RCX, RDX};

const CODE: u64 = 0x10_0000;

/// Run a straight-line sequence one instruction at a time. `kmovw`-style
/// sequences need this: `step_bytes` executes a single instruction.
#[track_caller]
fn run(cpu: &mut BochsOracle, code: &[u8], steps: usize) {
    cpu.set_rip(CODE);
    cpu.write_mem(CODE, code);
    for n in 0..steps {
        let out = cpu.step();
        assert!(out.is_retired(), "step {n}: {out:?} ({})", cpu.fault_msg());
    }
}

/// §2: the CPU model must actually claim AVX-512 and CET. Asking for a model
/// name that is not in cpudb leaves `set_by_name` a no-op and the build default
/// (`corei7_haswell_4770`, no AVX-512) selected — which went unnoticed for
/// months, because every EVEX case then merely "skipped as unimplemented".
#[test]
fn the_cpu_model_claims_avx512_and_cet() {
    let mut cpu = BochsOracle::new();
    cpu.set_gpr(RAX, 7);
    cpu.set_gpr(RCX, 0);
    let out = cpu.step_bytes(&[0x0F, 0xA2]); // cpuid
    assert!(out.is_retired(), "cpuid: {out:?}");
    let (ebx, ecx, edx) = (
        cpu.get_gpr(RBX) as u32,
        cpu.get_gpr(RCX) as u32,
        cpu.get_gpr(RDX) as u32,
    );
    assert_ne!(ebx & (1 << 16), 0, "CPUID.7:EBX.AVX512F, ebx={ebx:#010x}");
    assert_ne!(ebx & (1 << 30), 0, "CPUID.7:EBX.AVX512BW, ebx={ebx:#010x}");
    assert_ne!(ecx & (1 << 7), 0, "CPUID.7:ECX.CET_SS, ecx={ecx:#010x}");
    assert_ne!(edx & (1 << 20), 0, "CPUID.7:EDX.CET_IBT, edx={edx:#010x}");
}

/// §4g: the SDM says `IF COUNT <= 15`. Upstream's `count < 15` returns 0 here;
/// `patches/bochs/0001-kshiftlw-kshiftrw-count-15.patch` is what makes this pass.
#[test]
fn kshiftlw_shifts_at_count_15() {
    let mut cpu = BochsOracle::new();
    cpu.set_gpr(RCX, 1);
    run(
        &mut cpu,
        &[
            0xC5, 0xF8, 0x92, 0xC1, // kmovw    k0, ecx
            0xC4, 0xE3, 0xF9, 0x32, 0xC8, 0x0F, // kshiftlw k1, k0, 15
            0xC5, 0xF8, 0x93, 0xC1, // kmovw    eax, k1
        ],
        3,
    );
    assert_eq!(cpu.get_gpr(RAX), 0x8000, "kshiftlw of bit 0 by 15");
}

/// The mirror image: same off-by-one, other direction.
#[test]
fn kshiftrw_shifts_at_count_15() {
    let mut cpu = BochsOracle::new();
    cpu.set_gpr(RCX, 0x8000);
    run(
        &mut cpu,
        &[
            0xC5, 0xF8, 0x92, 0xC1, // kmovw    k0, ecx
            0xC4, 0xE3, 0xF9, 0x30, 0xC8, 0x0F, // kshiftrw k1, k0, 15
            0xC5, 0xF8, 0x93, 0xC1, // kmovw    eax, k1
        ],
        3,
    );
    assert_eq!(cpu.get_gpr(RAX), 1, "kshiftrw of bit 15 by 15");
}

/// The boundary the fix must NOT move: 16 is out of range and zeroes the
/// destination. Without this, "`count < 16`" and "no bound at all" both pass.
#[test]
fn kshiftlw_zeroes_at_count_16() {
    let mut cpu = BochsOracle::new();
    cpu.set_gpr(RCX, 1);
    run(
        &mut cpu,
        &[
            0xC5, 0xF8, 0x92, 0xC1, // kmovw    k0, ecx
            0xC4, 0xE3, 0xF9, 0x32, 0xC8, 0x10, // kshiftlw k1, k0, 16
            0xC5, 0xF8, 0x93, 0xC1, // kmovw    eax, k1
        ],
        3,
    );
    assert_eq!(cpu.get_gpr(RAX), 0, "count 16 is out of range");
}

/// §4h: fixed upstream, so this pins the *revision bump* rather than a local
/// patch — a re-vendor that went backwards would fail here. Each lane of the
/// result is a bitmap of the earlier lanes the source lane equals.
#[test]
fn vpconflictd_compares_against_every_earlier_lane() {
    let mut cpu = BochsOracle::new();
    let mut src = [0u64; ZMM_CHUNKS];
    src[0] = 0x0000_0003_0000_0007; // lanes 0,1 = 7,3
    src[1] = 0x0000_0007_0000_0007; // lanes 2,3 = 7,7
    cpu.set_zmm(1, &src);
    // vpconflictd zmm0, zmm1
    let out = cpu.step_bytes(&[0x62, 0xF2, 0x7D, 0x48, 0xC4, 0xC1]);
    assert!(out.is_retired(), "vpconflictd: {out:?} ({})", cpu.fault_msg());

    let dwords: Vec<u32> = cpu
        .get_zmm(0)
        .iter()
        .flat_map(|q| [*q as u32, (*q >> 32) as u32])
        .collect();
    // Lane 2 matches lane 0; lane 3 matches lanes 0 and 2; lanes 4.. are all
    // zero and so conflict with each other.
    assert_eq!(
        &dwords[..8],
        &[0, 0, 0x1, 0x5, 0, 0x10, 0x30, 0x70],
        "full result {dwords:x?}"
    );
}

// ---------------------------------------------------------------------------
// §4i: the truncating packed fp16 converts.
//
// Every one of these is an EVEX.128.MAP5.W0 `op xmm0{k1}, xmm1`, so the whole
// family differs only in the SSE prefix and the opcode byte. 0x62 0xF5 selects
// MAP5 with no high registers; byte 3 is W0 + vvvv=1111 + pp; byte 4 is
// L'L=128 plus the mask register; 0xC1 is the modrm for xmm0 <- xmm1.
const PP_NP: u8 = 0x7C;
const PP_66: u8 = 0x7D;
const PP_F3: u8 = 0x7E;

/// The source must have a fractional part, or the truncating and rounding
/// handlers agree and the assertion proves nothing. 367.5 is exactly halfway,
/// so round-to-nearest-even gives 368 where truncation gives 367 — and it is
/// representable in fp16, which values above 2048 no longer are.
const F16_POS_367_5: u16 = 0x5DBE;
const F16_NEG_367_5: u16 = 0xDDBE;

/// Run one convert against `src` in xmm1 and return the low 64 bits of xmm0.
/// The `kmovw` runs either way: for the unmasked encodings it is dead, and
/// keeping it makes the masked and unmasked paths the same two steps.
fn convert_fp16(src: u16, pp: u8, op: u8, masked: bool) -> u64 {
    let mut cpu = BochsOracle::new();
    let mut xmm1 = [0u64; ZMM_CHUNKS];
    xmm1[0] = u64::from(src);
    cpu.set_zmm(1, &xmm1);
    cpu.set_gpr(RCX, 1);
    run(
        &mut cpu,
        &[
            0xC5, 0xF8, 0x92, 0xC9, // kmovw k1, ecx
            0x62, 0xF5, pp, if masked { 0x09 } else { 0x08 }, op, 0xC1,
        ],
        2,
    );
    cpu.get_zmm(0)[0]
}

/// `(mnemonic, prefix, opcode)` for the four truncating rows, in the order
/// they appear in `ia_opcodes_evex.def`. Each is tested twice, unmasked and
/// `{k1}` — those are separate rows in that table, and the bug hit both.
const TRUNCATING: &[(&str, u8, u8)] = &[
    ("vcvttph2dq", PP_F3, 0x5B),
    ("vcvttph2udq", PP_NP, 0x78),
    ("vcvttph2qq", PP_66, 0x7A),
    ("vcvttph2uqq", PP_66, 0x78),
];

/// The non-truncating twin of each row above: same table, same handlers up to
/// the rounding mode. These are what the truncating rows were wrongly bound to.
const ROUNDING: &[(&str, u8, u8)] = &[
    ("vcvtph2dq", PP_66, 0x5B),
    ("vcvtph2udq", PP_NP, 0x79),
    ("vcvtph2qq", PP_66, 0x7B),
    ("vcvtph2uqq", PP_66, 0x79),
];

/// §4i: all eight truncating rows named the truncating instruction but bound
/// the *rounding* execute method, so `vcvttph2dq` of 367.5 returned 368.
/// `patches/bochs/0002-vcvttph2-truncating-handlers.patch` is what makes this
/// pass. Only lane 0 of the source is non-zero, so the result is 367 whether
/// the destination lane is a dword or a qword.
#[test]
fn truncating_fp16_converts_do_not_round() {
    for (name, pp, op) in TRUNCATING {
        for masked in [false, true] {
            let got = convert_fp16(F16_POS_367_5, *pp, *op, masked);
            let form = if masked { "{k1}" } else { "" };
            assert_eq!(got, 367, "{name}{form} of 367.5 (rounding would give 368)");
        }
    }
}

/// The control, and the reason 367.5 is the input: it only separates the two
/// handlers because the rounding ones really do round it up. Without this, a
/// re-vendor that bound *both* families to the truncating method would leave
/// the test above green.
#[test]
fn non_truncating_fp16_converts_still_round() {
    for (name, pp, op) in ROUNDING {
        for masked in [false, true] {
            let got = convert_fp16(F16_POS_367_5, *pp, *op, masked);
            let form = if masked { "{k1}" } else { "" };
            assert_eq!(got, 368, "{name}{form} of 367.5 under round-to-nearest");
        }
    }
}

/// Truncation is toward zero, not toward minus infinity — a distinction 367.5
/// cannot make, since floor and round-to-nearest-even agree there. On the
/// negative side they still agree with each other (-368) and disagree with
/// truncation (-367), so this pins the direction. Signed rows only: the
/// unsigned converts return the integer indefinite for any negative input.
#[test]
fn fp16_truncation_is_toward_zero() {
    for (name, pp, op, dword) in [
        ("vcvttph2dq", PP_F3, 0x5Bu8, true),
        ("vcvttph2qq", PP_66, 0x7A, false),
    ] {
        let raw = convert_fp16(F16_NEG_367_5, pp, op, false);
        // Lane 1 of the source is 0.0, which converts to 0, so a dword result
        // sign-extends only within its own lane.
        let got = if dword { raw as u32 as i32 as i64 } else { raw as i64 };
        assert_eq!(got, -367, "{name} of -367.5 (floor and RN both give -368)");
    }
}
