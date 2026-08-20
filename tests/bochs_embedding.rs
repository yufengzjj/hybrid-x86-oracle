//! The emulator layer `csrc/bochs_shim.cpp` fakes, exercised from guest code.
//!
//! Bochs' CPU expects a whole machine around it — devices, a GUI, a plugin
//! system — and the shim supplies just enough of that for a CPU-only oracle.
//! The failure mode of getting it wrong is not a wrong answer but a **segfault
//! inside the oracle**, because the CPU reaches these objects through raw
//! pointers it never null-checks. That is invisible to every other test in this
//! repo: they only run instructions that stay inside the CPU.
//!
//! So this file drives the guest instructions that *leave* the CPU. Today that
//! is the x87 FERR# line: every state-management instruction pokes
//! `bx_devices.pluginExtFpuIRQ`, which was NULL until the constructor in the
//! shim started mirroring Bochs' own `init_stubs()`. A two-byte FNCLEX crashed
//! the process. When re-vendoring adds a new `DEV_*` call, add it here.
//!
//! Bochs-only: no other backend has an emulator layer to fake. One
//! `BochsOracle` per test — the core is a process-wide singleton.
#![cfg(feature = "bochs")]

use x86_oracle::{BochsOracle, X86Oracle, RAX, RDI};

const CODE: u64 = 0x10_0000;
/// 16-byte aligned: FXRSTOR/XRSTOR #GP on a misaligned operand, which would
/// make these tests pass without ever reaching the device layer.
const DATA: u64 = 0x20_0000;

/// x87 status-word bits (cpu/fpu/status_w.h).
const SW_ZERO_DIV: u16 = 0x0004;
const SW_SUMMARY: u16 = 0x0080; // ES
const SW_BACKWARD: u16 = 0x8000; // B

/// `fnstsw ax` — the only way to read the status word through the oracle ABI,
/// and deliberately a *no-wait* instruction: it reports a pending unmasked
/// exception instead of raising #MF on it.
const FNSTSW_AX: [u8; 2] = [0xDF, 0xE0];

fn fresh() -> BochsOracle {
    let mut cpu = BochsOracle::new();
    // Map and clear the operand area. Also the FXRSTOR image used below:
    // all-zero is a legal one (MXCSR = 0 has no reserved bits set).
    cpu.write_mem(DATA, &[0u8; 512]);
    cpu.set_gpr(RDI, DATA);
    cpu
}

/// Execute one instruction at CODE and require it to retire.
#[track_caller]
fn step_one(cpu: &mut BochsOracle, what: &str, code: &[u8]) {
    cpu.set_rip(CODE);
    cpu.write_mem(CODE, code);
    let out = cpu.step();
    assert!(out.is_retired(), "{what}: {out:?} ({})", cpu.fault_msg());
}

/// Run `code`, then `fnstsw ax`, and return the status word.
#[track_caller]
fn status_after(cpu: &mut BochsOracle, what: &str, code: &[u8]) -> u16 {
    step_one(cpu, what, code);
    step_one(cpu, "fnstsw ax", &FNSTSW_AX);
    cpu.get_gpr(RAX) as u16
}

/// Every guest instruction that reaches `DEV_extfpuirq_set_fpu_error`.
///
/// Table-driven and printed as it goes: if one of these ever dereferences a
/// null plugin pointer again, the process dies with no assertion to report, and
/// the last line printed names the culprit.
#[test]
fn every_x87_instruction_that_touches_the_ferr_line_survives() {
    // (name, encoding) — all memory forms use [rdi] = DATA.
    const CASES: &[(&str, &[u8])] = &[
        // clear_unmasked_fpu_exception(), unconditionally
        ("fnclex", &[0xDB, 0xE2]),
        ("fnstenv [rdi]", &[0xD9, 0x37]),
        // set_ or clear_, depending on the status/control words
        ("fldcw [rdi]", &[0xD9, 0x2F]),
        ("fldenv [rdi]", &[0xD9, 0x27]),
        ("frstor [rdi]", &[0xDD, 0x27]),
        ("fxrstor [rdi]", &[0x0F, 0xAE, 0x0F]),
        ("xrstor [rdi]", &[0x0F, 0xAE, 0x2F]),
    ];

    for (name, code) in CASES {
        // Printed BEFORE the step: a segfault leaves this as the last output.
        println!("  -> {name}");
        let mut cpu = fresh();
        if *name == "xrstor [rdi]" {
            // XRSTOR takes the state-component bitmap in EDX:EAX. x87 only.
            cpu.set_gpr(RAX, 1);
        }
        cpu.set_rip(CODE);
        cpu.write_mem(CODE, code);
        let out = cpu.step();
        assert!(out.is_retired(), "{name}: {out:?} ({})", cpu.fault_msg());
    }
}

/// The *set* branch: restoring a state whose pending exception is unmasked must
/// raise FERR#, which architecturally means B and ES appear in the status word.
///
/// Asserting the status word rather than "it did not crash" is the point — a
/// stub that silently swallowed the call would still pass a liveness check, but
/// the B/ES update happens in the same function as the `DEV_*` call, so this
/// pins that the whole path ran.
#[test]
fn restoring_an_unmasked_pending_exception_raises_ferr() {
    let mut cpu = fresh();
    let mut image = [0u8; 512];
    image[0..2].copy_from_slice(&0x0000u16.to_le_bytes()); // FCW: nothing masked
    image[2..4].copy_from_slice(&SW_ZERO_DIV.to_le_bytes()); // FSW: #Z pending
    cpu.write_mem(DATA, &image);

    let sw = status_after(&mut cpu, "fxrstor [rdi]", &[0x0F, 0xAE, 0x0F]);
    assert_eq!(
        sw,
        SW_BACKWARD | SW_SUMMARY | SW_ZERO_DIV,
        "expected B|ES|ZE, got {sw:#06x}"
    );
}

/// The *clear* branch, from the state the previous test builds: FNCLEX drops
/// the exception bits, and with them FERR#.
#[test]
fn fnclex_lowers_ferr_again() {
    let mut cpu = fresh();
    let mut image = [0u8; 512];
    image[0..2].copy_from_slice(&0x0000u16.to_le_bytes());
    image[2..4].copy_from_slice(&SW_ZERO_DIV.to_le_bytes());
    cpu.write_mem(DATA, &image);

    let raised = status_after(&mut cpu, "fxrstor [rdi]", &[0x0F, 0xAE, 0x0F]);
    assert_ne!(raised & SW_SUMMARY, 0, "fixture did not raise ES first");

    let sw = status_after(&mut cpu, "fnclex", &[0xDB, 0xE2]);
    assert_eq!(sw, 0, "fnclex should clear the whole exception state, got {sw:#06x}");
}

/// The masked case takes the *other* branch of the same test, so it needs its
/// own case: a pending exception that the control word masks must NOT set ES.
#[test]
fn restoring_a_masked_pending_exception_leaves_ferr_low() {
    let mut cpu = fresh();
    let mut image = [0u8; 512];
    image[0..2].copy_from_slice(&0x003Fu16.to_le_bytes()); // FCW: all masked
    image[2..4].copy_from_slice(&SW_ZERO_DIV.to_le_bytes()); // FSW: #Z pending
    cpu.write_mem(DATA, &image);

    let sw = status_after(&mut cpu, "fxrstor [rdi]", &[0x0F, 0xAE, 0x0F]);
    assert_eq!(sw & (SW_BACKWARD | SW_SUMMARY), 0, "ES/B set despite the mask, sw={sw:#06x}");
    assert_eq!(sw & SW_ZERO_DIV, SW_ZERO_DIV, "the #Z flag itself should survive");
}

/// Sanity for the instrument the tests above measure with: on a fresh oracle
/// the x87 unit is initialised and the status word reads back clean.
#[test]
fn fnstsw_reads_a_clean_status_word_at_reset() {
    let mut cpu = fresh();
    cpu.set_gpr(RAX, 0xDEAD_BEEF_DEAD_BEEF);
    step_one(&mut cpu, "fnstsw ax", &FNSTSW_AX);
    assert_eq!(cpu.get_gpr(RAX) as u16, 0, "reset status word");
    // FNSTSW writes AX only; the rest of RAX must be untouched.
    assert_eq!(cpu.get_gpr(RAX) >> 16, 0xDEAD_BEEF_DEAD, "FNSTSW clobbered more than AX");
}

/// The list above is a hand-copy of Bochs' `init_stubs()`, and a re-vendor that
/// adds a plugin pointer upstream would not break the build — it would leave
/// one member NULL and wait for a guest instruction to find it. So check the
/// invariant at its source: every `plugin*` member Bochs declares must be
/// assigned in the shim's constructor.
///
/// Source-level on purpose. The C++ side cannot see this — there is no way to
/// enumerate members — and by the time a runtime check could look, the damage
/// is a segfault in whatever instruction got there first.
#[test]
fn the_shim_wires_up_every_plugin_pointer_bochs_declares() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = std::fs::read_to_string(root.join("vendor/bochs/iodev/iodev.h"))
        .expect("read vendor/bochs/iodev/iodev.h");
    let shim = std::fs::read_to_string(root.join("csrc/bochs_shim.cpp"))
        .expect("read csrc/bochs_shim.cpp");

    // Member declarations look like:  bx_cmos_stub_c    *pluginCmosDevice;
    let declared: Vec<&str> = header
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let name = l.strip_suffix(';')?.rsplit('*').next()?.trim();
            (l.starts_with("bx_") && l.contains('*') && name.starts_with("plugin")).then_some(name)
        })
        .collect();
    assert!(
        declared.len() >= 14,
        "only found {} plugin pointers in iodev.h — has the declaration style \
         changed? This test is now blind: fix the parser.",
        declared.len()
    );

    // The constructor body, so an assignment elsewhere in the file does not count.
    let ctor = shim
        .split_once("bx_devices_c::bx_devices_c()")
        .expect("shim defines bx_devices_c::bx_devices_c")
        .1;
    let ctor = &ctor[..ctor.find("\n}").expect("constructor is brace-terminated")];

    let missing: Vec<&&str> = declared
        .iter()
        .filter(|name| !ctor.contains(&format!("{name} = &")))
        .collect();
    assert!(
        missing.is_empty(),
        "csrc/bochs_shim.cpp's bx_devices_c constructor leaves {missing:?} NULL. \
         Bochs declares {} plugin pointers; mirror iodev/devices.cc:init_stubs() \
         for the new one(s), or the first guest instruction to use it segfaults \
         the oracle.",
        declared.len()
    );
}
