//! Regression cases for what this crate fixes in, or demands of, the vendored
//! Bochs — the things a re-vendor or a CPU-model change can silently undo.
//!
//! `docs/backend-differences.md` §2 and §4g are the prose; these are the pins.
//! Bochs-only on purpose: no other backend here executes AVX-512 (Sail has no
//! vector ISA, and the CI hosts have no AVX-512 silicon), so there is nothing
//! to differ against — these assert against the SDM directly.
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
