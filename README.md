# hybrid-x86-oracle

Pluggable **golden-reference x86-64 CPUs** for differential testing, behind one
API. Write a diff harness once against the `X86Oracle` trait, then point it at
whichever reference you want — or at all of them, and diff the references
against each other.

```rust
use x86_oracle::*;

// One harness, every backend.
fn add_matches<O: X86Oracle>(mut cpu: O, a: u64, b: u64) -> u64 {
    cpu.set_rip(DEFAULT_CODE_ADDR);
    cpu.set_gpr(RAX, a);
    cpu.set_gpr(RBX, b);
    assert!(cpu.step_bytes(&[0x48, 0x01, 0xD8]).is_retired()); // add rax, rbx
    cpu.get_gpr(RAX)
}

assert_eq!(add_matches(SailOracle::new(), 5, 7), 12);
assert_eq!(add_matches(BochsOracle::new(), 5, 7), 12);   // --features bochs
```

## Backends

| Backend | Feature | What it is | Coverage | Instances |
|---|---|---|---|---|
| `SailOracle` | `sail` (default) | The Sail model translated from the ACL2 **x86isa** formal model ([rems-project/sail-x86-from-acl2](https://github.com/rems-project/sail-x86-from-acl2)) | 64-bit user-mode **integer** ISA + SSE data movement — semantics machine-checked in ACL2 | unlimited |
| `BochsOracle` | `bochs` | The **Bochs** CPU core, embedded directly (no emulator around it) | Essentially all user-mode x86-64: SSE → **AVX-512**, BMI, CET | one per process |
| `KvmOracle` | `kvm` | **The host CPU itself**, through KVM (Linux) — hardware ground truth | Whatever this machine implements | unlimited |
| `WhpOracle` | `whp` | The same, through the Windows Hypervisor Platform | Whatever this machine implements | unlimited (one runs at a time) |

None is a substitute for the others: Sail gives proof-checked integer semantics
(including flag corner cases), Bochs gives breadth, the hardware backends give
the final word — where a model and the hardware disagree, the model is wrong.
Running them all and diffing is the point; `docs/backend-differences.md` lists
the places they are *allowed* to differ.

`AnyOracle`/`Backend` provide runtime (rather than compile-time) backend choice,
including `Backend::available()` and `Backend::pairs()` for "run against
everything compiled in".

### Platforms

The two software backends build and run anywhere with C and C++ compilers. The
two hardware backends are the *same backend* — one guest design, one set of
control structures, one fault-observation scheme (`src/hw.rs`) — wearing the
hypervisor interface its OS provides, so nothing above them has to care which:

| Feature | Linux | Windows (native) | macOS |
|---|---|---|---|
| `sail` | ✅ | ✅ | ✅ |
| `bochs` | ✅ | ✅ | ✅ |
| `hardware` | KVM | WHP | — (no backend yet; HVF would fit the same seam) |

```sh
cargo test --features bochs,hardware   # everything this platform can do
```

`hardware` enables both `kvm` and `whp`; each is inert off its own OS, so the
same command line works on either. `Backend::hardware()` names whichever applies
here, and `Backend::available()`/`pairs()` already omit anything unusable — so a
differential campaign written once runs unchanged on both, testing three
references on Linux and Windows and two on macOS.

Nothing has to be available for a build to succeed: `--no-default-features
--features hardware` on a platform with no hypervisor backend still compiles, and
its tests skip. "Which references exist here" is a runtime answer, never a
compile error.

## Shared semantic contract

Backends are only comparable if they agree on the ground rules. All of them
guarantee:

- **Reset state**: 64-bit long mode, CPL 0, flat address space (linear == the
  address you pass to the memory API), all GPRs and vector registers zero,
  `RFLAGS == 0x2`, `CR4.OSFXSR` set, `RIP == 0`.
- **`step()` = exactly one instruction**, fetched from the instance's own memory
  at RIP. No interrupts, no timers, nothing asynchronous. x86 instructions are
  variable-length, so there is no opcode injection: write the bytes at RIP
  (`step_bytes` does both).
- **Per-instance memory**, byte-addressed, little-endian; never-written
  addresses read as zero.
- **Faults do not vector**: a faulting instruction yields `StepOutcome::Fault`
  carrying committed-so-far state, not an IDT dispatch. `FaultKind` is a
  normalized, comparable classification; the message is free text.
- **`FaultKind::Unimplemented` is not a fault**: it means the backend does not
  implement the encoding. `StepOutcome::is_comparable()` filters those out.

## State access

| State | API |
|---|---|
| RAX–R15 (ModRM order) | `set_gpr`/`get_gpr` + `RAX`..`R15` |
| RIP (canonical 48-bit) | `set_rip`/`get_rip` |
| RFLAGS | `set_rflags`/`get_rflags` + `FLAG_*`, `ARITH_FLAGS` |
| ZMM0–ZMM31 (512 b) | `set_zmm`/`get_zmm` (8 × `u64`, chunk 0 = XMM low half) |
| MSRs by architectural number | `set_msr`/`get_msr` + `IA32_EFER`, `IA32_FS_BASE`, … |
| CR0/2/3/4/8 | `set_cr`/`get_cr` |
| Segments (selector, base, limit) | `set_seg_*`/`get_seg_*` + `SEG_ES`..`SEG_GS` |
| Memory | `read_mem`/`write_mem`, `*_u64`, `*_byte`, `is_mapped` |
| Execution | `step`, `step_bytes`, `fault_msg` |
| Snapshots | `snapshot`, `restore`, `CpuState::diff`, `diff_masked`, `eq_integer` |

In 64-bit mode the FS/GS bases live in `IA32_FS_BASE`/`IA32_GS_BASE`; both
backends keep `set_seg_base` and those MSRs in sync, so either API works.

## Quick start

Both the Sail and Bochs backends are **self-contained**: the pre-generated Sail
model, a patched Sail runtime, and the Bochs CPU sources are all vendored, so a
build needs nothing but C and C++ compilers (C++20). No network, no Bochs
checkout, no autotools, no Sail toolchain.

```sh
cargo test                  # sail backend only
cargo test --features bochs # + the Bochs CPU, compiled from vendor/bochs/
```

The first Bochs build compiles ~670 files and takes about a minute; after that
cargo caches it. `BOCHS_DIR` overrides the source root if you want to track a
different Bochs revision, and `scripts/vendor-bochs.sh` refreshes
`vendor/bochs/` from a pinned upstream commit.

The hardware backends need hardware virtualization plus permission to use it.

On **Linux** (KVM), access to `/dev/kvm`:

```sh
sudo chmod 666 /dev/kvm             # or: sudo usermod -aG kvm $USER  (then re-login)
cargo test --features bochs,hardware
```

On **Windows** (WHP), the Windows Hypervisor Platform optional feature — which
implies Hyper-V, so it needs a reboot, and it is what WSL2 already turns on:

```powershell
dism /Online /Enable-Feature /FeatureName:HypervisorPlatform /All
cargo test --features bochs,hardware
```

Nothing extra is needed to *build*: `winhvplatform.dll` is resolved with
`GetProcAddress` at first use, not imported, so the test binary still starts on a
machine that has no WHP at all — it just reports the backend unavailable. Either
way, when the hypervisor is missing or inaccessible the hardware tests report a
skip and pass and `Backend::available()` omits the backend, so a campaign runs
unchanged everywhere.

`cargo nextest run` gives process-per-test parallelism, which is also how you
get more than one Bochs oracle at a time.

### Build directory

The Sail and Bochs builds produce a large `target/` (a few GB with both profiles
and both host targets). If the checkout lives on a network or interop filesystem
— a WSL2 `/mnt/c` path being the common case — put it on a local disk instead,
or every rebuild pays the crossing:

```sh
export CARGO_TARGET_DIR=~/.cache/cargo-target/hybrid-x86-oracle
```

or, to make it stick for this checkout only, a gitignored `.cargo/config.toml`
with `[build] target-dir = "..."`.

Mind the interop edge if you drive Windows `cargo.exe` from the same WSL tree:
it reads that same `.cargo/config.toml` but cannot resolve a WSL path, so a
`target-dir` of `/home/you/...` silently becomes `C:\home\you\...`. Exporting
`CARGO_TARGET_DIR` does **not** fix it — WSL only forwards variables named in
`WSLENV`, so the env var never reaches the Windows process and the config wins
anyway. Pass the override on the command line instead:

```sh
cargo.exe test --target x86_64-pc-windows-msvc --features bochs,whp \
    --target-dir 'C:\Users\you\.cargo-target\hybrid-x86-oracle'
```

## The differential suite

`tests/differential.rs` runs 60 single-instruction cases from a bit-identical
starting state on **every pair** of available backends and compares registers,
flags and memory. With all three enabled on an AVX2 Linux host:

```
arith  sail vs bochs: 40 agreed, 2 skipped     memory sail vs bochs: 11 agreed
arith  sail vs kvm:   40 agreed, 2 skipped     memory sail vs kvm:   11 agreed
arith  bochs vs kvm:  42 agreed, 0 skipped     memory bochs vs kvm:  11 agreed
branch: 7 agreed on all three pairs
  sail   vector: 32 regs x 512 bits
  bochs  vector: 32 regs x 512 bits
  kvm    vector: 16 regs x 256 bits
```

The `bochs vs kvm` row is the interesting one: it checks an emulator against
the silicon, including the instructions Sail does not implement. The same
command on Windows prints `whp` wherever this shows `kvm`.

Cases whose flags the ISA leaves undefined are annotated `.undef(FLAG_SF | ...)`
and masked; `docs/backend-differences.md` explains each one. There is also a
`harness_detects_divergence` test, so a suite that silently compares nothing
cannot pass.

`tests/portable.rs` runs the same 31-check contract suite against every backend
(and against `AnyOracle`), so a new backend inherits the whole thing.

`tests/throughput.rs` is an ignored benchmark for sizing a campaign — how many
steps per second each backend sustains, and what an instance costs to create:

```sh
cargo test --release --features bochs,hardware --test throughput -- --ignored --nocapture
```

The two kinds of backend fail in opposite directions, so read both with care: the
loop re-runs one instruction from a fixed address, which is the best case for an
emulator's decode cache, while a hardware backend pays a hypervisor round trip
per instruction and so measures the cost of *observing* a step rather than of
executing one.

## How it is built

```
                        src/lib.rs   trait X86Oracle, StepOutcome, CpuState
                      /      |      \
           src/sail.rs   src/bochs.rs   src/hw.rs   the guest: RAM, page tables,
                |             |          /      \   GDT/IDT/TSS, fault stubs, XSAVE
        csrc/shim.cpp  csrc/bochs_shim.cpp     \
                |             |        src/kvm.rs  src/whp.rs
        vendor/sail/   vendor/bochs/   (Linux)     (Windows)
   (sail --cpp -> C++ class) (upstream CPU sources)
```

The two software backends follow the same recipe: a C/C++ shim exposes a flat
`extern "C"` handle ABI, `build.rs` compiles it, and the Rust side is a thin safe
wrapper that serializes calls.

- **Sail**: `sail --cpp` turns the model into `class model::Model` whose
  architectural state is instance members, which is what allows many
  independent oracles; regeneration steps are below.
- **Bochs**: `vendor/bochs/` holds the upstream CPU sources (unpatched, pinned
  revision — see `vendor/bochs/PROVENANCE`), which `build.rs` compiles directly.
  `csrc/bochs_shim.cpp` supplies the pieces of the emulator the CPU
  core expects — the CPU/memory singletons, a sparse page-map memory, the
  instrumentation callbacks that make a step stop after one instruction, an
  identity-mapped page table for long mode, and a minimal parameter tree. No
  third-party embedding layer is involved. The one thing in that tree that is
  not portable is `config.h`, which is Bochs' own configure output and so
  records *a Linux host's* answers; `build.rs` rewrites the few lines that are
  wrong for MSVC into a copy it force-includes, leaving the vendored file
  pristine, which is why Windows needs no extra setup.

The two hardware backends are one design with two drivers. `src/hw.rs` is the
guest: a minimal VM — one vCPU, one flat RAM slot, identity-mapped 2 MiB pages, a
flat GDT. Faults are the subtle part: hardware *must* vector through an IDT, so
the guest gets one whose 256 gates each point at their own `HLT`, delivered on an
IST stack. The stub's address gives the vector, the exception frame gives the
faulting RIP/RSP/error code, and the backend then undoes the vectoring so the
observable state matches the other backends' "faults do not vector" contract —
from any RSP, including zero. `src/hw.rs` also locates the vector registers in an
XSAVE image, in either of the two layouts the architecture defines.

- **KVM** (`src/kvm.rs`): ioctls on `/dev/kvm`, `KVM_GUESTDBG_SINGLESTEP`, and
  the IDT-stub scheme above for faults.
- **WHP** (`src/whp.rs`): a WHP partition, RFLAGS.TF for stepping, and — where
  available — `ExceptionExitBitmap`, which hands us the exception *before* the CPU
  delivers it, so RIP/RSP/RFLAGS are already the faulting instruction's and there
  is nothing to unwind. If exception intercepts are unavailable it falls back to
  the IDT stubs, which need nothing from WHP but halt exits, and are enough on
  their own for both faults *and* stepping. `winhvplatform.dll` is loaded with
  `GetProcAddress`, not imported, so a build has no WHP SDK requirement and the
  binary starts on machines without WHP.

### Non-obvious things these backends get right

All of these were found the hard way and are covered by tests, because none of
them announces itself: each either produces a wrong "reference" in silence, or
fails in a place that points at the wrong culprit.

1. **Lazy flags.** Bochs computes the arithmetic flags on demand; the raw
   `eflags` field is stale until `force_flags()` runs. Reading it directly (as
   some embeddings do) returns wrong CF/PF/AF/ZF/SF/OF. Every read here goes
   through `read_eflags()`. `add_sets_parity` in the portable suite catches a
   regression.
2. **A20 masking and derived mode state.** Every physical address is ANDed with
   `bx_pc_system.a20_mask`, which is zero until `set_enable_a20(1)` — so every
   translation resolves to physical 0 and the CPU quietly fetches zeroes.
   Similarly, `cpu_mode` is derived state: writing CR0/CR4/EFER without
   `handleCpuModeChange()` leaves the core in real mode. `BochsOracle::new`
   asserts the core really reached long mode.
3. **Single-stepping is RFLAGS.TF, on both hypervisors.** `KVM_SET_GUEST_DEBUG`
   sets the trap flag (and WHP has nothing else to offer), so any later register
   write — i.e. every `set_rip`/`set_gpr` — clears it again and the guest runs
   away until it triple-faults. Single-stepping is therefore re-armed immediately
   before each run, not once at startup.
4. **Exception delivery needs its own stack.** An oracle legitimately starts with
   `RSP == 0`, where pushing an exception frame faults, then double-faults, then
   triples. Every IDT gate uses IST1 and a dedicated stack, so a fault is
   observable from *any* register state.
5. **XSAVE has two layouts.** Component offsets come from `CPUID.(EAX=0Dh):EBX`
   in the standard format but have to be accumulated in the compacted one, and
   which you got is bit 63 of the image's own XCOMP_BV. Guessing reads the wrong
   bytes rather than failing, so `src/hw.rs` reads XCOMP_BV and handles both.
6. **WHP reads register values with an *aligned* SSE move.** `winhvplatform.dll`
   copies the `WHV_REGISTER_VALUE` array with `movaps` on some of its register
   paths (`Tr` and `Ldtr` among them), so a buffer that is merely 8-byte aligned
   — all `#[repr(C)]` over 64-bit members guarantees — faults on those and
   nowhere else, and only when the compiler happened to place it on 8 mod 16.
   An alignment fault has no faulting address, so Windows
   reports it as an access violation *"reading address 0xFFFFFFFFFFFFFFFF"*
   inside the DLL, which reads like a null-pointer bug in WHP rather than a
   buffer that is off by eight. `src/whp.rs` puts the alignment in the type, so
   no call site can get it wrong.
7. **WHP maps guest RAM for one partition per process.** A second
   `WHvMapGpaRange` fails with `HV_STATUS_OPERATION_DENIED` while any other
   partition in the process holds a range — whatever size or guest address it
   asks for. Unmapping the incumbent frees the slot instantly, and the mapping
   is only needed while the guest is *executing*, so oracles hand it over on
   `step()`. That is what keeps `WhpOracle` instances independent, and why the
   handover is lazy: it costs a few milliseconds, and a lone oracle never pays it.

## Regenerating the Sail model

Only needed after editing `sail/harness.sail` or to track a newer
sail-x86-from-acl2. Needs Sail 0.20.1 + z3.

```sh
cd sail-x86-from-acl2/model
ORACLE=/path/to/hybrid-x86-oracle

sail --cpp --c-no-main -O --Oconstant-fold --memo-z3 \
    --c-preserve main --c-preserve init_harness \
    --c-preserve set_gpr --c-preserve get_gpr \
    --c-preserve set_rip --c-preserve get_rip \
    --c-preserve set_rflags --c-preserve get_rflags \
    --c-preserve step_x86 \
    --c-preserve set_zmm_chunk --c-preserve get_zmm_chunk \
    --c-preserve set_msr --c-preserve get_msr \
    --c-preserve set_ctr --c-preserve get_ctr \
    --c-preserve set_seg_visible --c-preserve get_seg_visible \
    --c-preserve set_seg_base --c-preserve get_seg_base \
    --c-preserve set_seg_limit --c-preserve get_seg_limit \
    --c-preserve set_seg_attr --c-preserve get_seg_attr \
    --c-preserve get_fault_msg \
    prelude.sail register_types.sail registers.sail register_accessors.sail \
    opcode_ext.sail memory_accessors.sail init.sail config.sail \
    "$ORACLE/sail/quiet_logging.sail" step.sail main.sail \
    "$ORACLE/sail/harness.sail" \
    -o cpp/model

python3 "$ORACLE/scripts/fix_cpp_model.py" cpp/model.h cpp/model.cpp
```

Two deliberate substitutions: `logging.sail` is replaced by
`sail/quiet_logging.sail` (the stock one prints every memory access to stdout),
and `sail/harness.sail` is appended last. `main.sail` is kept — it provides the
Sail `main` (required even with `--c-no-main`) and `initialise_64_bit_mode()`.
Sail's dead-code elimination drops anything unreferenced, hence one
`--c-preserve` per wrapper. Then refresh `vendor/` as described in
`vendor/README.md`.

`fix_cpp_model.py` is not optional. Four of its fixes make the generated C++
compile and survive multiple instances; the fifth corrects a real semantic error
in the ACL2→Sail translation (`ROR` computes CF from the wrong bit for rotate
counts > 1 — `docs/backend-differences.md` §4b). Skip it and the `sail` backend
silently disagrees with hardware.

## Limitations

- **Application level only**: flat addressing, no page-table experiments (the
  machinery exists in both backends but the oracle does not set it up).
- **No asynchronous events** — by design; it is what makes the oracles
  deterministic.
- **`RDRAND`/`RDSEED`** are not usable as references. Exclude them; on a
  hardware backend `RDTSC` and `CPUID` are non-reproducible for the same reason.
- **A hardware backend is one machine's answer**: coverage equals the host CPU's
  (the host these examples came from: 16 × 256-bit vectors, no AVX-512), and
  implementation-defined behaviour may differ on another CPU.
- **Licensing**: this crate's own code is BSD-3-Clause. The Sail backend
  statically links a **mini-gmp (LGPL)** vendored under `vendor/sail/`; the
  Bochs backend statically links **Bochs (LGPL-2.1)**, vendored under
  `vendor/bochs/`.
  Both matter if you redistribute binaries — see `LICENSE` and
  `vendor/README.md`.

## Adding a backend

1. Implement `X86Oracle`; honour the contract above.
2. Add a cargo feature and a `Backend` variant.
3. Add one line to `tests/portable.rs` — the 31-check contract suite runs
   against it immediately.
4. Add it to the pairs in `tests/differential.rs`.

Possible next backends: a remote/cloud CPU of a different vendor (AMD vs Intel
implementation-defined behaviour is a rich diff source), or a second formal
model for the SIMD subset that Sail leaves out.
