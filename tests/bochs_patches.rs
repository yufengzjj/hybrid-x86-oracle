//! Regression cases for what this crate fixes in, or demands of, the vendored
//! Bochs — the things a re-vendor or a CPU-model change can silently undo.
//!
//! `docs/backend-differences.md` §2, §4g, §4i, §4j and §4k–§4r are the prose; these are the
//! pins. Bochs-only on purpose: no other backend here executes AVX-512 (Sail
//! has no vector ISA, and the CI hosts have no AVX-512 silicon), so there is
//! nothing to differ against — these assert against the SDM directly.
//!
//! One `BochsOracle` per test: the core is a process-wide singleton.
#![cfg(feature = "bochs")]

use x86_oracle::{BochsOracle, X86Oracle, ZMM_CHUNKS, RAX, RBX, RCX, RDI, RDX, RSI};

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

// ---------------------------------------------------------------------------
// §4j: the q-index d-element gathers must zero the destination above VL/2.
//
// `vpgatherqd` / `vgatherqps` return a result half the index width, so with a
// zmm index the destination is a ymm and bits 256..511 must read as zero
// (SDM: `DEST[MAXVL-1:VL/2] := 0`). Upstream derived that clear length with
// `len--`, which happens to work for a ymm index (VL256 → VL128) but yields
// a value `BX_CLEAR_AVX_REGZ` ignores for a zmm index, so the upper half kept
// whatever the register held. `patches/bochs/0003-vpgatherqd-zmm-index-upper-clear.patch`
// is what makes `q_index_d_element_gathers_zero_above_vl_half` pass.

const DATA: u64 = 0x20_0000;

/// Stale contents seeded into every chunk of the destination before the gather,
/// so "nothing was cleared" and "was cleared" read differently.
const STALE: u64 = 0xAAAA_AAAA_AAAA_AAAA;

/// EVEX.66.0F38.W0 `op` /r with a VSIB of `[rbx + zmm2*4]` (or `ymm2` at
/// VL256), destination register 1, mask k7. Only the L'L bits change between
/// the two vector lengths, so the encoding is assembled here rather than copied
/// from a disassembler for each form.
fn q_index_gather(op: u8, vl512: bool) -> [u8; 7] {
    // P2: z=0, L'L, b=0, V'=1 (index bit 4 clear), aaa=111 (k7).
    let p2 = if vl512 { 0x4F } else { 0x2F };
    // ModRM 0x0C: mod=00, reg=001, rm=100 (SIB). SIB 0x93: scale=4, index=2, base=rbx.
    [0x62, 0xF2, 0x7D, p2, op, 0x0C, 0x93]
}

/// Run one q-index gather: dword `n` of the window at `DATA` holds `0x1000 + n`,
/// index lane `n` of zmm2 is `n`, zmm1 starts as [`STALE`; 8] and k7 as `mask`.
/// Returns the final zmm1 and the final k7 (via `kmovw eax, k7`).
fn run_q_index_gather(op: u8, vl512: bool, mask: u16) -> ([u64; ZMM_CHUNKS], u64) {
    let mut cpu = BochsOracle::new();
    let window: Vec<u8> = (0..16u32).flat_map(|n| (0x1000 + n).to_le_bytes()).collect();
    cpu.write_mem(DATA, &window);
    cpu.set_gpr(RBX, DATA);
    cpu.set_gpr(RCX, u64::from(mask));
    cpu.set_zmm(1, &[STALE; ZMM_CHUNKS]);
    let index: [u64; ZMM_CHUNKS] = core::array::from_fn(|n| n as u64);
    cpu.set_zmm(2, &index);
    let insn = q_index_gather(op, vl512);
    let mut code = vec![0xC5, 0xF8, 0x92, 0xF9]; // kmovw k7, ecx
    code.extend_from_slice(&insn);
    code.extend_from_slice(&[0xC5, 0xF8, 0x93, 0xC7]); // kmovw eax, k7
    run(&mut cpu, &code, 3);
    (cpu.get_zmm(1), cpu.get_gpr(RAX))
}

/// The two rows that share `VGATHERQPS_MASK_VpsVSib`: `(mnemonic, opcode)`.
const Q_INDEX_D_ELEMENT: &[(&str, u8)] = &[("vpgatherqd", 0x91), ("vgatherqps", 0x93)];

/// §4j proper: with a zmm index the eight gathered dwords fill bits 0..255 and
/// the stale upper half must be gone. The mask leaves lanes 1 and 6 unselected
/// so the merge into the *live* half is checked in the same breath — a fix
/// that zeroed the whole register would fail here too.
#[test]
fn q_index_d_element_gathers_zero_above_vl_half() {
    for (name, op) in Q_INDEX_D_ELEMENT {
        let mask = 0b1011_1101u16;
        let (zmm1, k7) = run_q_index_gather(*op, true, mask);
        let lane = |n: usize| (zmm1[n / 2] >> (32 * (n % 2))) as u32;
        for n in 0..8 {
            let want = if mask & (1 << n) != 0 { 0x1000 + n as u32 } else { STALE as u32 };
            assert_eq!(lane(n), want, "{name} zmm-index: dword lane {n} of {zmm1:x?}");
        }
        assert_eq!(&zmm1[4..], &[0; 4], "{name} zmm-index: bits 256..511 of {zmm1:x?}");
        assert_eq!(k7, 0, "{name}: k7 is cleared once every lane is done");
    }
}

/// The length the fix must NOT break: a ymm index gives an xmm result, and
/// upstream's arithmetic was right there. `len >>= 1` and `len--` agree at
/// VL256, so this only guards the boundary; the test above is the bug.
#[test]
fn q_index_d_element_gathers_zero_above_128_with_ymm_index() {
    for (name, op) in Q_INDEX_D_ELEMENT {
        let (zmm1, k7) = run_q_index_gather(*op, false, 0b1111);
        assert_eq!(
            &zmm1[..2],
            &[0x0000_1001_0000_1000, 0x0000_1003_0000_1002],
            "{name} ymm-index: gathered lanes of {zmm1:x?}"
        );
        assert_eq!(&zmm1[2..], &[0; 6], "{name} ymm-index: bits 128..511 of {zmm1:x?}");
        assert_eq!(k7, 0, "{name}: k7 is cleared once every lane is done");
    }
}

// ---------------------------------------------------------------------------
// §4k–§4o: AMX, enabled 2026-09-12, and the five upstream bugs that surfaced.
//
// Every test here starts with `tilerelease`, the architectural AMX init state.
// It is what a fresh machine presents anyway — the shim's reset clears the AMX
// unit, and the last section of this file pins that — but these tests are about
// the instructions, not the reset path, so they do not lean on it.
//
// Encodings are what GNU as 2.45 emits for the Intel-syntax mnemonic in the
// comment; all are VEX.128.0F38 with W0, so a decoder that asked for W1 on any
// of them (§4k) faults instead of retiring.

/// `sttilecfg` / `ldtilecfg` images, `tileloadd` sources and `tilestored`
/// destinations. Distinct 64 KiB windows so a stray write to one cannot be
/// mistaken for the expected write to another.
const CFG_IN: u64 = 0x21_0000;
const CFG_OUT: u64 = 0x21_1000;
const TILE_A: u64 = 0x22_0000;
const TILE_B: u64 = 0x23_0000;
const TILE_OUT: u64 = 0x24_0000;
/// XSAVE area: 64-byte aligned (XSAVE/XRSTOR #GP otherwise) and large enough
/// for the standard-format layout up to and including TILEDATA.
const XSAVE_AREA: u64 = 0x30_0000;
const XSAVE_AREA_LEN: usize = XSAVE_TILEDATA + 8192;
/// Standard-format offsets (`cpu/crregs.h`): XSTATE_BV in the header, then the
/// two AMX components after PKRU. The shim's XCR0 is 0x600E7, so RFBM = bits
/// 17 + 18 selects exactly these two.
const XSAVE_XSTATE_BV: usize = 512;
const XSAVE_TILECFG: usize = 2752;
const XSAVE_TILEDATA: usize = 2816;
const XCR0_TILECFG: u64 = 1 << 17;
const XCR0_TILEDATA: u64 = 1 << 18;
const RFBM_TILES: u64 = XCR0_TILECFG | XCR0_TILEDATA;

const TILERELEASE: [u8; 5] = [0xC4, 0xE2, 0x78, 0x49, 0xC0];
const LDTILECFG_RBX: [u8; 5] = [0xC4, 0xE2, 0x78, 0x49, 0x03]; // ldtilecfg [rbx]
const STTILECFG_RCX: [u8; 5] = [0xC4, 0xE2, 0x79, 0x49, 0x01]; // sttilecfg [rcx]
const TILEZERO_TMM0: [u8; 5] = [0xC4, 0xE2, 0x7B, 0x49, 0xC0];
const TILELOADD_TMM0_RDI_RSI: [u8; 6] = [0xC4, 0xE2, 0x7B, 0x4B, 0x04, 0x37]; // tileloadd tmm0, [rdi+rsi*1]
const TILELOADD_TMM1_RDI_RSI: [u8; 6] = [0xC4, 0xE2, 0x7B, 0x4B, 0x0C, 0x37]; // tileloadd tmm1, [rdi+rsi*1]
const TILELOADD_TMM2_RDX_RSI: [u8; 6] = [0xC4, 0xE2, 0x7B, 0x4B, 0x14, 0x32]; // tileloadd tmm2, [rdx+rsi*1]
const TILESTORED_RCX_RSI_TMM0: [u8; 6] = [0xC4, 0xE2, 0x7A, 0x4B, 0x04, 0x31]; // tilestored [rcx+rsi*1], tmm0
const TILESTORED_RDI_RSI_TMM0: [u8; 6] = [0xC4, 0xE2, 0x7A, 0x4B, 0x04, 0x37]; // tilestored [rdi+rsi*1], tmm0
const XSAVE_RCX: [u8; 3] = [0x0F, 0xAE, 0x21]; // xsave [rcx]
const XRSTOR_RCX: [u8; 3] = [0x0F, 0xAE, 0x29]; // xrstor [rcx]

/// A palette-1 TILECFG image: `tiles[n]` is `(rows, colsb)` for tmm`n`, the
/// rest unconfigured. Bytes 16..31 hold the eight 16-bit colsb, 48..55 the
/// eight row counts — the layout `ldtilecfg` reads and `sttilecfg` must write.
fn tilecfg(tiles: &[(u8, u16)]) -> [u8; 64] {
    let mut cfg = [0u8; 64];
    cfg[0] = 1; // palette_id
    for (n, (rows, colsb)) in tiles.iter().enumerate() {
        cfg[16 + 2 * n..16 + 2 * n + 2].copy_from_slice(&colsb.to_le_bytes());
        cfg[48 + n] = *rows;
    }
    cfg
}

/// A full 16×64-byte tile whose every dword names its own row and column, so a
/// row that was skipped, or landed in the wrong place, reads differently from
/// every other row and from the `STALE` fill.
fn tile_pattern() -> Vec<u8> {
    (0..16u32)
        .flat_map(|row| (0..16u32).map(move |col| 0x5000_0000 | row << 8 | col))
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn read_bytes(cpu: &BochsOracle, addr: u64, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    cpu.read_mem(addr, &mut buf);
    buf
}

/// §4k + §4l: what `ldtilecfg` loaded, `sttilecfg` must store back verbatim.
/// Upstream decoded `sttilecfg` at VEX.W1, so the real (W0) encoding raised #UD
/// — the `run` assertion; and its writer had rows and colsb swapped, so a
/// configuration that did decode came back transposed — the byte comparison.
/// Every configured tile has rows ≠ colsb so a swap cannot hide.
/// `patches/bochs/0004-sttilecfg-vex-w0.patch` and `0005-amx-xsave-tilecfg-tiledata.patch`.
#[test]
fn sttilecfg_stores_what_ldtilecfg_loaded() {
    let mut cpu = BochsOracle::new();
    let cfg = tilecfg(&[(2, 12), (16, 64), (5, 20), (1, 4)]);
    cpu.write_mem(CFG_IN, &cfg);
    cpu.write_mem(CFG_OUT, &[STALE as u8; 64]);
    cpu.set_gpr(RBX, CFG_IN);
    cpu.set_gpr(RCX, CFG_OUT);
    let code = [TILERELEASE.as_slice(), &LDTILECFG_RBX, &STTILECFG_RCX].concat();
    run(&mut cpu, &code, 3);
    let got = read_bytes(&cpu, CFG_OUT, 64);
    assert_eq!(got, cfg, "sttilecfg image differs from the ldtilecfg one");
}

/// The other half of the image writer: with no configuration loaded (palette
/// 0) `sttilecfg` stores 64 zero bytes, not the stale memory and not a palette
/// byte with garbage after it.
#[test]
fn sttilecfg_after_tilerelease_stores_zeroes() {
    let mut cpu = BochsOracle::new();
    cpu.write_mem(CFG_OUT, &[STALE as u8; 64]);
    cpu.set_gpr(RCX, CFG_OUT);
    let code = [TILERELEASE.as_slice(), &STTILECFG_RCX].concat();
    run(&mut cpu, &code, 2);
    assert_eq!(read_bytes(&cpu, CFG_OUT, 64), [0u8; 64], "init-state sttilecfg");
}

/// §4m (save side) + §4o: a `tileloadd` into a 16-row tile followed by
/// `xsave` with RFBM = TILECFG|TILEDATA. Upstream's saver looped over 8 rows
/// (`BX_TILE_REGISTERS` where `BX_TILE_MAX_ROWS` was meant), so rows 8..15 of
/// the save area kept their `STALE` fill; and its XINUSE test was inverted, so
/// XSTATE_BV[18] read 0 for a tile that had just been loaded.
/// `patches/bochs/0005-amx-xsave-tilecfg-tiledata.patch`.
#[test]
fn xsave_tiledata_saves_all_sixteen_rows_and_marks_tiles_in_use() {
    let mut cpu = BochsOracle::new();
    let pattern = tile_pattern();
    cpu.write_mem(CFG_IN, &tilecfg(&[(16, 64)]));
    cpu.write_mem(TILE_A, &pattern);
    cpu.write_mem(XSAVE_AREA, &vec![STALE as u8; XSAVE_AREA_LEN]);
    // Header: XSAVE merges XSTATE_BV bits outside RFBM from memory, so give
    // it a clean one rather than 0xAA..AA.
    cpu.write_mem(XSAVE_AREA + XSAVE_XSTATE_BV as u64, &[0u8; 64]);
    cpu.set_gpr(RBX, CFG_IN);
    cpu.set_gpr(RDI, TILE_A);
    cpu.set_gpr(RSI, 64); // tileloadd stride
    cpu.set_gpr(RCX, XSAVE_AREA);
    cpu.set_gpr(RAX, RFBM_TILES);
    cpu.set_gpr(RDX, 0);
    let code = [TILERELEASE.as_slice(), &LDTILECFG_RBX, &TILELOADD_TMM0_RDI_RSI, &XSAVE_RCX].concat();
    run(&mut cpu, &code, 4);

    let xstate_bv = cpu.read_mem_u64(XSAVE_AREA + XSAVE_XSTATE_BV as u64);
    assert_eq!(xstate_bv & XCR0_TILECFG, XCR0_TILECFG, "XSTATE_BV[17] after ldtilecfg: {xstate_bv:#x}");
    assert_eq!(xstate_bv & XCR0_TILEDATA, XCR0_TILEDATA, "XSTATE_BV[18] after tileloadd: {xstate_bv:#x}");

    let tile0 = read_bytes(&cpu, XSAVE_AREA + XSAVE_TILEDATA as u64, 1024);
    for row in 0..16 {
        assert_eq!(
            &tile0[row * 64..row * 64 + 64],
            &pattern[row * 64..row * 64 + 64],
            "tmm0 row {row} in the save area (rows 8..15 are the bug)"
        );
    }
    // tmm1..tmm7 are zero after tilerelease and must have been written as such
    // — all 16 rows of each, not just the first 8.
    let rest = read_bytes(&cpu, XSAVE_AREA + XSAVE_TILEDATA as u64 + 1024, 7 * 1024);
    assert!(rest.iter().all(|b| *b == 0), "tmm1..tmm7 must be saved as zeroes, not left stale");
}

/// §4o, the other direction: nothing loaded, so TILEDATA is not in use and
/// XSTATE_BV[18] must be 0. Upstream returned `tile_use_tracker == 0` here,
/// i.e. reported 1 on exactly this machine. TILECFG likewise.
#[test]
fn xsave_marks_tiledata_unused_after_tilerelease() {
    let mut cpu = BochsOracle::new();
    cpu.write_mem(XSAVE_AREA, &vec![0u8; XSAVE_AREA_LEN]);
    cpu.set_gpr(RCX, XSAVE_AREA);
    cpu.set_gpr(RAX, RFBM_TILES);
    cpu.set_gpr(RDX, 0);
    let code = [TILERELEASE.as_slice(), &XSAVE_RCX].concat();
    run(&mut cpu, &code, 2);
    let xstate_bv = cpu.read_mem_u64(XSAVE_AREA + XSAVE_XSTATE_BV as u64);
    assert_eq!(xstate_bv & RFBM_TILES, 0, "XSTATE_BV[17:18] on an AMX init state: {xstate_bv:#x}");
}

/// §4m (restore side): a hand-built standard-format image with XSTATE_BV =
/// TILECFG|TILEDATA, a 16×64 tmm0 configuration and the full 1 KiB of tmm0
/// data. After `xrstor`, `tilestored` must write back all 16 rows; upstream's
/// restorer stopped at row 8, leaving rows 8..15 as `tilerelease` had them.
#[test]
fn xrstor_tiledata_restores_all_sixteen_rows() {
    let mut cpu = BochsOracle::new();
    let pattern = tile_pattern();
    let mut image = vec![0u8; XSAVE_AREA_LEN];
    image[XSAVE_XSTATE_BV..XSAVE_XSTATE_BV + 8].copy_from_slice(&RFBM_TILES.to_le_bytes());
    image[XSAVE_TILECFG..XSAVE_TILECFG + 64].copy_from_slice(&tilecfg(&[(16, 64)]));
    image[XSAVE_TILEDATA..XSAVE_TILEDATA + 1024].copy_from_slice(&pattern);
    cpu.write_mem(XSAVE_AREA, &image);
    cpu.write_mem(TILE_OUT, &vec![STALE as u8; 1024]);
    cpu.set_gpr(RCX, XSAVE_AREA);
    cpu.set_gpr(RAX, RFBM_TILES);
    cpu.set_gpr(RDX, 0);
    cpu.set_gpr(RDI, TILE_OUT);
    cpu.set_gpr(RSI, 64);
    let code = [TILERELEASE.as_slice(), &XRSTOR_RCX, &TILESTORED_RDI_RSI_TMM0].concat();
    run(&mut cpu, &code, 3);
    let got = read_bytes(&cpu, TILE_OUT, 1024);
    for row in 0..16 {
        assert_eq!(
            &got[row * 64..row * 64 + 64],
            &pattern[row * 64..row * 64 + 64],
            "tmm0 row {row} after xrstor (rows 8..15 are the bug)"
        );
    }
}

/// The byte quadruples for the INT8 dot products, chosen so each signedness
/// combination has its own answer. Read as signed: A = [-1, -128, 2, 127],
/// B = [1, 2, -2, -128]; as unsigned: A = [255, 128, 2, 127], B = [1, 2, 254, 128].
const INT8_A: [u8; 4] = [0xFF, 0x80, 0x02, 0x7F];
const INT8_B: [u8; 4] = [0x01, 0x02, 0xFE, 0x80];

/// `(mnemonic, VEX pp byte, expected dot product)` for `tdpb??d tmm0, tmm1, tmm2`.
/// The pp byte selects the variant: F2 = ssd, F3 = sud, 66 = usd, none = uud.
/// Upstream sign-extended nothing, so all four returned the `uud` value 17275.
const INT8_DOT_PRODUCTS: &[(&str, u8, i32)] = &[
    ("tdpbssd", 0x6B, -16517), // (-1)(1) + (-128)(2) + (2)(-2) + (127)(-128)
    ("tdpbsud", 0x6A, 16507),  // (-1)(1) + (-128)(2) + (2)(254) + (127)(128)
    ("tdpbusd", 0x69, -15749), // (255)(1) + (128)(2) + (2)(-2) + (127)(-128)
    ("tdpbuud", 0x68, 17275),  // (255)(1) + (128)(2) + (2)(254) + (127)(128)
];

/// §4n: 1×1 tiles with K = 4 bytes, so tmm0 ends up holding exactly one
/// dword: the dot product of `INT8_A` and `INT8_B` under the instruction's
/// signedness. `patches/bochs/0006-amx-int8-byte-signedness.patch` is what
/// makes the three signed rows pass; `tdpbuud` is the control that was right
/// all along and must stay so.
#[test]
fn int8_dot_products_honor_byte_signedness() {
    for (name, pp, want) in INT8_DOT_PRODUCTS {
        let mut cpu = BochsOracle::new();
        // C = tmm0 (1 row × 4 bytes), A = tmm1 (1 × 4), B = tmm2 (1 row × 4 bytes)
        cpu.write_mem(CFG_IN, &tilecfg(&[(1, 4), (1, 4), (1, 4)]));
        cpu.write_mem(TILE_A, &INT8_A);
        cpu.write_mem(TILE_B, &INT8_B);
        cpu.write_mem(TILE_OUT, &(STALE as u32).to_le_bytes());
        cpu.set_gpr(RBX, CFG_IN);
        cpu.set_gpr(RDI, TILE_A);
        cpu.set_gpr(RDX, TILE_B);
        cpu.set_gpr(RCX, TILE_OUT);
        cpu.set_gpr(RSI, 64);
        let code = [
            TILERELEASE.as_slice(),
            &LDTILECFG_RBX,
            &TILEZERO_TMM0,
            &TILELOADD_TMM1_RDI_RSI,
            &TILELOADD_TMM2_RDX_RSI,
            &[0xC4, 0xE2, *pp, 0x5E, 0xC1], // tdpb??d tmm0, tmm1, tmm2
            &TILESTORED_RCX_RSI_TMM0,
        ]
        .concat();
        run(&mut cpu, &code, 7);
        let got = cpu.read_mem_u64(TILE_OUT) as u32 as i32;
        assert_eq!(got, *want, "{name} of A={INT8_A:02x?} · B={INT8_B:02x?}");
    }
}

// ---------------------------------------------------------------------------
// The reset contract across instances.
//
// `BochsOracle::new()` promises a fresh machine, but the core is a singleton,
// so everything `reset(BX_RESET_HARDWARE)` does not reset survives from the
// previous instance. Bochs resets x87, MXCSR, the zmm file and the opmask file
// (`cpu/init.cc`) but not the AMX unit, so the shim's `enter_long_mode` calls
// `amx->clear()` itself. These tests load state into one instance, drop it,
// and read it back from the next — a shim that stopped clearing, or a
// re-vendor that added a register file nobody resets, shows up as the previous
// test's data.

/// A `tileloadd`ed tile and its configuration must not survive into the next
/// oracle: `sttilecfg` reads 64 zero bytes and XSAVE says neither AMX component
/// is in use. No `tilerelease` here on purpose — that is what the tests above
/// use to protect themselves from exactly this leak.
#[test]
fn a_fresh_oracle_has_no_amx_state_from_its_predecessor() {
    {
        let mut cpu = BochsOracle::new();
        cpu.write_mem(CFG_IN, &tilecfg(&[(16, 64)]));
        cpu.write_mem(TILE_A, &tile_pattern());
        cpu.set_gpr(RBX, CFG_IN);
        cpu.set_gpr(RDI, TILE_A);
        cpu.set_gpr(RSI, 64);
        let code = [LDTILECFG_RBX.as_slice(), &TILELOADD_TMM0_RDI_RSI].concat();
        run(&mut cpu, &code, 2);
    }

    let mut cpu = BochsOracle::new();
    cpu.write_mem(CFG_OUT, &[STALE as u8; 64]);
    cpu.write_mem(XSAVE_AREA, &vec![0u8; XSAVE_AREA_LEN]);
    cpu.set_gpr(RCX, CFG_OUT);
    run(&mut cpu, &STTILECFG_RCX, 1);
    assert_eq!(read_bytes(&cpu, CFG_OUT, 64), [0u8; 64], "tile configuration leaked");

    cpu.set_gpr(RCX, XSAVE_AREA);
    cpu.set_gpr(RAX, RFBM_TILES);
    cpu.set_gpr(RDX, 0);
    run(&mut cpu, &XSAVE_RCX, 1);
    let xstate_bv = cpu.read_mem_u64(XSAVE_AREA + XSAVE_XSTATE_BV as u64);
    assert_eq!(xstate_bv & RFBM_TILES, 0, "tile data leaked: XSTATE_BV = {xstate_bv:#x}");
}

/// The control: the opmask file *is* reset by Bochs, so this passes without
/// any help from the shim and pins that a re-vendor keeps it that way.
#[test]
fn a_fresh_oracle_has_no_opmask_state_from_its_predecessor() {
    {
        let mut cpu = BochsOracle::new();
        cpu.set_gpr(RCX, 0xFFFF);
        run(&mut cpu, &[0xC5, 0xF8, 0x92, 0xF9], 1); // kmovw k7, ecx
    }
    let mut cpu = BochsOracle::new();
    run(&mut cpu, &[0xC5, 0xF8, 0x93, 0xC7], 1); // kmovw eax, k7
    assert_eq!(cpu.get_gpr(RAX), 0, "k7 leaked from the previous oracle");
}

// ---------------------------------------------------------------------------
// §4p: a store into code that already executed must retire the cached trace.
//
// Trace-cache entries are indexed by `pAddr ^ fetchModeMask`, and upstream's
// `handleSMC` only walked the 128 entries of the written 128-byte line, which
// is complete only while `fetchModeMask < 0x80`. `BX_FETCH_MODE_AMX_OK` is bit
// 7, so with XCR0's tile bits set every trace sat one line over and survived
// the store; `step_bytes`-style "rewrite the instruction at RIP" then replayed
// the old instruction. `patches/bochs/0007-icache-smc-walk-fetchmode-bit7.patch`
// is what makes this pass. It needs no AMX instruction at all — only XCR0 with
// the tile bits, which the shim sets for every instance.

/// Two `mov rax, imm32` at the same address, one step each. Without the patch
/// the second step re-executes the first instruction and RAX stays 1.
#[test]
fn rewriting_code_at_the_same_address_replaces_the_cached_trace() {
    let mut cpu = BochsOracle::new();
    run(&mut cpu, &[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00], 1); // mov rax, 1
    assert_eq!(cpu.get_gpr(RAX), 1);
    run(&mut cpu, &[0x48, 0xC7, 0xC0, 0x02, 0x00, 0x00, 0x00], 1); // mov rax, 2
    assert_eq!(cpu.get_gpr(RAX), 2, "the trace decoded from the first write was replayed");
}

/// The same, one instruction into the page: the written line is not line 0,
/// so the fix's upper bound (`mask << 1`) rather than any incidental walk from
/// line 0 is what covers the displaced entry.
#[test]
fn rewriting_code_in_a_later_page_line_replaces_the_cached_trace() {
    let mut cpu = BochsOracle::new();
    let addr = CODE + 0x300; // line 6 of the page
    for imm in [1u8, 2] {
        cpu.set_rip(addr);
        cpu.write_mem(addr, &[0x48, 0xC7, 0xC0, imm, 0x00, 0x00, 0x00]); // mov rax, imm
        let out = cpu.step();
        assert!(out.is_retired(), "mov rax, {imm}: {out:?}");
        assert_eq!(cpu.get_gpr(RAX), u64::from(imm), "line-6 trace was replayed");
    }
}

// ---------------------------------------------------------------------------
// §4q: the extensions layered onto sapphire_rapids with `add_features`.
//
// The CPUID bit is what gates decoding, so the real pin is "the encoding
// retires instead of #UD". One representative encoding per feature name in
// `cpu_added_features`, in the shim's order; every byte string is what GNU as
// 2.45 emits for the mnemonic in the comment. A name that fell out of the list
// (or a re-vendor that lost a handler) shows up as an UndefinedOpcode here.

/// Step one instruction at `CODE` and require it to retire; `rbx` points at
/// `DATA` for the memory forms and `rsi` is a tile stride.
#[track_caller]
fn retires(cpu: &mut BochsOracle, name: &str, code: &[u8]) {
    cpu.set_rip(CODE);
    cpu.write_mem(CODE, code);
    let out = cpu.step();
    assert!(out.is_retired(), "{name}: {out:?} ({})", cpu.fault_msg());
}

/// `(feature name, mnemonic, encoding)` for everything but the AMX group,
/// which needs a tile configuration first (below).
const ADDED_FEATURE_PROBES: &[(&str, &str, &[u8])] = &[
    ("avx10_2", "vminmaxps xmm0, xmm1, xmm2, 0", &[0x62, 0xF3, 0x75, 0x08, 0x52, 0xC2, 0x00]),
    ("avx10_2_movrs", "vmovrsb zmm0, [rbx]", &[0x62, 0xF5, 0x7F, 0x48, 0x6F, 0x03]),
    ("movrs", "movrs rax, [rbx]", &[0x48, 0x0F, 0x38, 0x8B, 0x03]),
    ("avx_ne_convert", "vbcstnebf162ps xmm0, [rbx]", &[0xC4, 0xE2, 0x7A, 0xB1, 0x03]),
    ("sha512", "vsha512msg1 ymm0, xmm1", &[0xC4, 0xE2, 0x7F, 0xCC, 0xC1]),
    ("sm3", "vsm3msg1 xmm0, xmm1, xmm2", &[0xC4, 0xE2, 0x70, 0xDA, 0xC2]),
    ("sm4", "vsm4key4 xmm0, xmm1, xmm2", &[0xC4, 0xE2, 0x72, 0xDA, 0xC2]),
    ("avx_ifma", "{vex} vpmadd52luq xmm0, xmm1, xmm2", &[0xC4, 0xE2, 0xF1, 0xB4, 0xC2]),
    ("avx_vnni_int8", "vpdpbssd xmm0, xmm1, xmm2", &[0xC4, 0xE2, 0x73, 0x50, 0xC2]),
    ("avx_vnni_int16", "vpdpwsud xmm0, xmm1, xmm2", &[0xC4, 0xE2, 0x72, 0xD2, 0xC2]),
    ("cmpccxadd", "cmpoxadd [rbx], rax, rcx", &[0xC4, 0xE2, 0xF1, 0xE0, 0x03]),
    ("avx512vp2intersect", "vp2intersectd k0, zmm0, zmm1", &[0x62, 0xF2, 0x7F, 0x48, 0x68, 0xC1]),
    ("xop", "vpcomltb xmm0, xmm1, xmm2", &[0x8F, 0xE8, 0x70, 0xCC, 0xC2, 0x00]),
    ("fma4", "vfmaddps xmm0, xmm1, xmm2, xmm3", &[0xC4, 0xE3, 0xF1, 0x68, 0xC3, 0x20]),
    ("tbm", "bextr rax, rbx, 0x1234", &[0x8F, 0xEA, 0xF8, 0x10, 0xC3, 0x34, 0x12, 0x00, 0x00]),
    ("sse4a", "extrq xmm0, 1, 2", &[0x66, 0x0F, 0x78, 0xC0, 0x01, 0x02]),
    ("3dnow", "pfadd mm0, mm1", &[0x0F, 0x0F, 0xC1, 0x9E]),
    ("3dnow_ext", "pswapd mm0, mm1", &[0x0F, 0x0F, 0xC1, 0xBB]),
];

/// §4q: every non-AMX feature in `cpu_added_features` decodes. `avx10_1` has
/// no instruction of its own here (it re-labels AVX-512), so `avx10_2` stands
/// for both.
#[test]
fn every_added_feature_decodes() {
    let mut cpu = BochsOracle::new();
    cpu.write_mem(DATA, &[0u8; 64]);
    cpu.set_gpr(RBX, DATA);
    for (feature, mnemonic, code) in ADDED_FEATURE_PROBES {
        retires(&mut cpu, &format!("{feature}: {mnemonic}"), code);
    }
}

/// The AMX half of §4q, after a 16×64 configuration for tmm0..tmm2 (the
/// TMUL shape checks need C = m×n, A = m×k, B = k×n; 16 rows × 16 dwords fits
/// all three). `tcvtrowd2ps` reads row `eax` = 0 of tmm0.
#[test]
fn every_added_amx_feature_decodes() {
    let mut cpu = BochsOracle::new();
    cpu.write_mem(CFG_IN, &tilecfg(&[(16, 64), (16, 64), (16, 64)]));
    cpu.write_mem(DATA, &[0u8; 1024]);
    cpu.set_gpr(RBX, CFG_IN);
    retires(&mut cpu, "ldtilecfg [rbx]", &LDTILECFG_RBX);
    cpu.set_gpr(RBX, DATA);
    cpu.set_gpr(RSI, 64);
    cpu.set_gpr(RAX, 0);
    for (feature, mnemonic, code) in [
        ("amx_fp16", "tdpfp16ps tmm0, tmm1, tmm2", &[0xC4, 0xE2, 0x6B, 0x5C, 0xC1][..]),
        ("amx_complex", "tcmmimfp16ps tmm0, tmm1, tmm2", &[0xC4, 0xE2, 0x69, 0x6C, 0xC1]),
        ("amx_fp8", "tdpbf8ps tmm0, tmm1, tmm2", &[0xC4, 0xE5, 0x68, 0xFD, 0xC1]),
        ("amx_avx512", "tcvtrowd2ps zmm0, tmm0, eax", &[0x62, 0xF2, 0x7E, 0x48, 0x4A, 0xC0]),
        ("amx_movrs", "tileloaddrs tmm0, [rbx+rsi*1]", &[0xC4, 0xE2, 0x7B, 0x4A, 0x04, 0x33]),
    ] {
        retires(&mut cpu, &format!("{feature}: {mnemonic}"), code);
    }
    retires(&mut cpu, "tilerelease", &TILERELEASE);
}

/// The boundary §4q draws: what this Bochs revision has no handler for stays
/// #UD, however the feature list grows. AVX512-ER (`vexp2ps`), Key Locker
/// (`encodekey128`) and AVX512-4FMAPS (`vp4dpwssd`) are the three families the
/// docs name as unenableable; a re-vendor that gains one of them should move
/// it into `cpu_added_features` and out of here.
#[test]
fn features_without_handlers_still_raise_ud() {
    use x86_oracle::FaultKind;
    let mut cpu = BochsOracle::new();
    cpu.write_mem(DATA, &[0u8; 64]);
    cpu.set_gpr(RBX, DATA);
    for (name, code) in [
        ("avx512er vexp2ps zmm0, zmm1", &[0x62, 0xF2, 0x7D, 0x48, 0xC8, 0xC1][..]),
        ("keylocker encodekey128 eax, ecx", &[0xF3, 0x0F, 0x38, 0xFA, 0xC1]),
        ("avx512_4fmaps vp4dpwssd zmm0, zmm1, [rbx]", &[0x62, 0xF2, 0x77, 0x48, 0x52, 0x03]),
    ] {
        cpu.set_rip(CODE);
        cpu.write_mem(CODE, code);
        let out = cpu.step();
        assert_eq!(out.fault_kind(), Some(FaultKind::UndefinedOpcode), "{name}: {out:?}");
    }
}

/// The CPUID side of §4q, for the leaves where the sapphire_rapids model
/// derives its bits from the ISA set rather than hard-coding them: leaf 7.1
/// EAX, leaf 7.0 EDX (VP2INTERSECT), leaf 0x80000001 ECX (the AMD VEX
/// extensions) and leaf 0x1E.1 EAX (the AMX extensions). Leaf 7.1 EDX, leaf
/// 0x24 and 0x80000001 EDX are hard-coded to zero in that model, so
/// AVX10, AVX-VNNI-INT8/16, AVX-NE-CONVERT, AMX-COMPLEX and 3DNow! do not show
/// in CPUID even though they decode — the tests above are the pin for those.
#[test]
fn the_cpu_model_reports_the_added_features_it_can() {
    let mut cpu = BochsOracle::new();
    let mut cpuid = |leaf: u64, subleaf: u64| {
        cpu.set_gpr(RAX, leaf);
        cpu.set_gpr(RCX, subleaf);
        let out = cpu.step_bytes(&[0x0F, 0xA2]);
        assert!(out.is_retired(), "cpuid {leaf:#x}.{subleaf}: {out:?}");
        (cpu.get_gpr(RAX) as u32, cpu.get_gpr(RCX) as u32, cpu.get_gpr(RDX) as u32)
    };
    let (eax7_1, _, _) = cpuid(7, 1);
    for (name, bit) in [("sha512", 0), ("sm3", 1), ("sm4", 2), ("cmpccxadd", 7), ("amx_fp16", 21), ("avx_ifma", 23), ("movrs", 31)] {
        assert_ne!(eax7_1 & (1 << bit), 0, "CPUID.7.1:EAX[{bit}] {name}, eax={eax7_1:#010x}");
    }
    let (_, _, edx7_0) = cpuid(7, 0);
    assert_ne!(edx7_0 & (1 << 8), 0, "CPUID.7.0:EDX[8] avx512vp2intersect, edx={edx7_0:#010x}");
    let (_, ecx_ext1, _) = cpuid(0x8000_0001, 0);
    for (name, bit) in [("sse4a", 6), ("xop", 11), ("fma4", 16), ("tbm", 21)] {
        assert_ne!(ecx_ext1 & (1 << bit), 0, "CPUID.80000001:ECX[{bit}] {name}, ecx={ecx_ext1:#010x}");
    }
    let (eax_amx, _, _) = cpuid(0x1E, 1);
    for (name, bit) in [("amx_complex", 2), ("amx_fp16", 3), ("amx_fp8", 4), ("amx_avx512", 7), ("amx_movrs", 8)] {
        assert_ne!(eax_amx & (1 << bit), 0, "CPUID.1E.1:EAX[{bit}] {name}, eax={eax_amx:#010x}");
    }
}

// ---------------------------------------------------------------------------
// §4r: the two-source fp16→fp8 converts must saturate on the S spelling only.
//
// All eight converts are EVEX.128 `op xmm0, xmm1, xmm2` (or `xmm0, xmm1` for
// the single-source four); the S forms differ from the plain ones only in the
// map/opcode byte. Lane 0 is +65504 (fp16 max) and lane 1 is −65504, both
// beyond fp8 range; lane 2 is 1.0 as the in-range control. For the two-source
// forms xmm2 feeds bytes 0..7 and xmm1 bytes 8..15, so xmm1 carries the two
// overflow lanes swapped to pin the placement as well.

const F16_MAX: u64 = 0x7BFF;
const F16_NEG_MAX: u64 = 0xFBFF;
const F16_ONE: u64 = 0x3C00;

/// `(mnemonic, encoding, expected dst bytes 0..15)`. BF8 is E5M2: Inf = 0x7C,
/// largest finite = 0x7B, 1.0 = 0x3C. HF8 is E4M3: NaN (there is no Inf) =
/// 0x7F, largest finite = 0x7E, 1.0 = 0x38.
const FP8_TWO_SOURCE: &[(&str, [u8; 6], [u8; 16])] = &[
    ("vcvt2ph2bf8", [0x62, 0xF2, 0x77, 0x08, 0x74, 0xC2],
        [0x7C, 0xFC, 0x3C, 0, 0, 0, 0, 0, 0xFC, 0x7C, 0x3C, 0, 0, 0, 0, 0]),
    ("vcvt2ph2bf8s", [0x62, 0xF5, 0x77, 0x08, 0x74, 0xC2],
        [0x7B, 0xFB, 0x3C, 0, 0, 0, 0, 0, 0xFB, 0x7B, 0x3C, 0, 0, 0, 0, 0]),
    ("vcvt2ph2hf8", [0x62, 0xF5, 0x77, 0x08, 0x18, 0xC2],
        [0x7F, 0xFF, 0x38, 0, 0, 0, 0, 0, 0xFF, 0x7F, 0x38, 0, 0, 0, 0, 0]),
    ("vcvt2ph2hf8s", [0x62, 0xF5, 0x77, 0x08, 0x1B, 0xC2],
        [0x7E, 0xFE, 0x38, 0, 0, 0, 0, 0, 0xFE, 0x7E, 0x38, 0, 0, 0, 0, 0]),
];

/// The single-source siblings, which keyed `saturate` correctly all along and
/// must keep doing so: `(mnemonic, encoding, expected dst bytes 0..7)`. Their
/// source is xmm1 = [−max, +max, 1.0], hence the sign order.
const FP8_ONE_SOURCE: &[(&str, [u8; 6], [u8; 8])] = &[
    ("vcvtph2bf8", [0x62, 0xF2, 0x7E, 0x08, 0x74, 0xC1], [0xFC, 0x7C, 0x3C, 0, 0, 0, 0, 0]),
    ("vcvtph2bf8s", [0x62, 0xF5, 0x7E, 0x08, 0x74, 0xC1], [0xFB, 0x7B, 0x3C, 0, 0, 0, 0, 0]),
    ("vcvtph2hf8", [0x62, 0xF5, 0x7E, 0x08, 0x18, 0xC1], [0xFF, 0x7F, 0x38, 0, 0, 0, 0, 0]),
    ("vcvtph2hf8s", [0x62, 0xF5, 0x7E, 0x08, 0x1B, 0xC1], [0xFE, 0x7E, 0x38, 0, 0, 0, 0, 0]),
];

fn fp8_convert(code: &[u8]) -> [u8; 16] {
    let mut cpu = BochsOracle::new();
    let mut xmm1 = [0u64; ZMM_CHUNKS];
    xmm1[0] = F16_NEG_MAX | F16_MAX << 16 | F16_ONE << 32;
    let mut xmm2 = [0u64; ZMM_CHUNKS];
    xmm2[0] = F16_MAX | F16_NEG_MAX << 16 | F16_ONE << 32;
    cpu.set_zmm(1, &xmm1);
    cpu.set_zmm(2, &xmm2);
    cpu.set_zmm(0, &[STALE; ZMM_CHUNKS]);
    run(&mut cpu, code, 1);
    let zmm0 = cpu.get_zmm(0);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&zmm0[0].to_le_bytes());
    bytes[8..].copy_from_slice(&zmm0[1].to_le_bytes());
    bytes
}

/// §4r proper: upstream keyed the two-source handlers' `saturate` on the PLAIN
/// opcode id, so `vcvt2ph2bf8` clamped and `vcvt2ph2bf8s` produced Inf.
/// `patches/bochs/0008-vcvt2ph2-fp8-saturate-opcode-key.patch` is what makes
/// this pass.
#[test]
fn two_source_fp8_converts_saturate_on_the_s_spelling_only() {
    for (name, code, want) in FP8_TWO_SOURCE {
        let got = fp8_convert(code);
        assert_eq!(&got, want, "{name} of [+max, -max, 1.0] from xmm2 then xmm1");
    }
}

/// The control: the single-source forms were keyed on the S id all along, so
/// a re-vendor that "fixed" all six the same way round would fail here.
#[test]
fn one_source_fp8_converts_still_saturate_on_the_s_spelling_only() {
    for (name, code, want) in FP8_ONE_SOURCE {
        let got = fp8_convert(code);
        assert_eq!(&got[..8], want, "{name} of xmm1 = [-max, +max, 1.0]");
    }
}
