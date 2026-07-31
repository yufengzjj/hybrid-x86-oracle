# Where the backends legitimately differ

Two correct x86 implementations do not have to agree on everything. This is the
list of places where a divergence is *expected*, so a diff campaign can mask it
instead of filing it. Everything **not** on this list is a finding.

Measured with the suite in `tests/differential.rs` (60 single-instruction cases,
`cargo test --features bochs`), which iterates `Backend::pairs()` — so it covers
whatever the platform provides without naming backends.

The two hardware backends (KVM on Linux, WHP on Windows) are one implementation
with two hypervisor drivers, so where this document says something about "the
hardware backends" it applies to both by construction, not by coincidence.
Anywhere they genuinely differ is called out.

## 1. Architecturally undefined flags

The ISA leaves some flags undefined after certain instructions; each
implementation may compute whatever it likes. Confirmed divergences:

| Instruction | Undefined per Intel SDM | Observed |
|---|---|---|
| `IMUL r64, r/m64` (2-operand) | SF, ZF, AF, PF | Bochs sets SF from the result; Sail leaves it clear |
| `MUL r/m64` | SF, ZF, AF, PF | same (SF) |
| `SHLD`/`SHRD`, count > 1 | OF (and AF always) | Bochs sets OF; Sail leaves it clear |

Mask these with `CpuState::diff_masked(&other, FLAG_SF | FLAG_ZF | ...)`; the
differential suite annotates each case with `.undef(...)`.

Rule of thumb: only diff the flags an instruction actually defines. `ARITH_FLAGS`
is the full status set, which is right for ADD/SUB/AND/OR/XOR/CMP/TEST/NEG/INC/DEC
and the shifts with count 1.

## 2. Instruction coverage

`StepOutcome::Fault { kind: FaultKind::Unimplemented, .. }` means the backend
does not implement the encoding — it says nothing about semantics, so those
cases must be skipped (`StepOutcome::is_comparable()` returns false).

- **Sail** implements the 64-bit user-mode integer ISA plus SSE data movement.
  Not implemented: CPUID, `BSF`/`BSR`, integer SIMD (`PXOR` and friends), SSE
  arithmetic, most of the two-byte map, all of VEX/EVEX (decoded, then reported
  as unimplemented).
- **Bochs** implements essentially everything a user-mode program can execute,
  including AVX-512.
- **KVM / WHP** implement exactly what the host CPU does — no more, no less. On
  a host without AVX-512, EVEX encodings raise a genuine #UD. That is correct
  ground truth for that machine but *not* a semantic disagreement with a
  backend that executes them, so check `vector_chunks()`/`vector_regs()` (or
  `host_vector_width()`) before diffing wide-vector cases.

So the useful overlap is the integer ISA. That is also where Sail is most
valuable: its semantics come from the ACL2 x86isa model and are machine-checked.

## 2b. Vector register width

`X86Oracle::vector_chunks()` reports how many 64-bit chunks of each vector
register a backend can hold, and `vector_regs()` how many registers exist.
Sail and Bochs report 32 × 512 bits; the hardware backends report whatever the
host exposes to the guest (32 × 512 with AVX-512, else 16 × 256, else 16 × 128).
Writes above that width are dropped and reads return zero. Compare only
`min(a, b)` chunks.

On WHP this is what the *guest* CPUID says, not the host's: the backend asks
`WHvGetVirtualProcessorCpuidOutput` where available, and only counts a component
that XCR0 actually accepted. So a hypervisor that hides AVX-512 shows up as
`vector_chunks() == 4` rather than as mysterious #UDs.

## 3. Instances per process

- **Sail**: any number of independent oracles, each with its own memory.
- **KVM / WHP**: any number — each is its own VM (or WHP partition).
- **Bochs**: exactly one. The CPU core is a process-wide singleton (built with
  SMP disabled), so `BochsOracle::new()` blocks until the previous instance is
  dropped. A portable test must therefore hold at most one oracle at a time —
  note that `let c = ...; let c = ...;` *shadows* rather than drops. For
  parallelism use process-per-test (`cargo nextest run`).

## 4. String instructions (`REP`)

Every backend runs **one iteration per step**, with RIP parked on the
instruction until the loop ends — so per-iteration stepping is portable, and a
`rep movsb` with `RCX = n` takes exactly `n` steps (`n = 0` takes one, which
does nothing). That is real hardware's behaviour under a single-step trap, not a
modelling convention.

One thing is *not* portable: the flags a `REPE`/`REPNE CMPS` computes **on an
iteration that does not end the loop**. Bochs and Sail expose them; hardware
does not — the `#DB` lands at an instruction boundary where the architectural
flag update has not been committed:

```text
repe cmpsb, rcx=3, all operands equal
             sail   bochs   kvm
  step 1     0x46   0x46    0x02   <- rcx=2, RIP parked
  step 2     0x46   0x46    0x02   <- rcx=1, RIP parked
  step 3     0x46   0x46    0x46   <- rcx=0, RIP advances
```

The loops converge: compare only when the instruction has finished. `MOVS` and
`STOS` set no flags, so mid-loop states are comparable there.

Bochs also sets RF while parked on a repeat, where hardware does not; the Bochs
backend masks TF and RF out of `get_rflags`/`set_rflags`, as the KVM and WHP
backends already did.

**This section used to describe a translation bug instead; it is now fixed in
the vendored model.** `x86_movs` / `x86_cmps` / `x86_stos` had three distinct
errors, all against
[`model/string_ops.sail`](https://github.com/rems-project/sail-x86-from-acl2/blob/master/model/string_ops.sail):

1. `x86_stos` reads `prefixes[seg]` where ACL2's `x86-stos` reads
   `(prefixes->rep prefixes)`, so `rep stos` never repeated at all — one store,
   RCX untouched, RIP straight past.
2. The `rCX == 0` test ACL2 performs *before* reading either operand is absent,
   so entering with `RCX = 0` executed an iteration anyway and wrapped the
   counter to `0xFFFF_FFFF_FFFF_FFFF` — a `rep movsb` that should be a no-op
   instead ran 2^64 times, writing memory as it went.
3. The `0xF3` arm has its two branches the wrong way round, so RIP advanced
   exactly when the loop should have continued and vice versa:

```sail
243 => {                                      // 0xF3
    let counter = ...rCX - 1;
    if counter == 0 | rflags[zf] == 0b0 then {
        write_rgfi_size(..., counter, ...)    // wrong: this is the *stop* case
    } else {
        write_rgfi_size(..., counter, ...);
        write_iptr(proc_mode, temp_rip)       // ...so RIP advances to continue
    }
}
```

   The `0xF2` arm next to it has the same two bodies in the opposite order,
   which is what makes the mistake visible. On top of that, consulting ZF at all
   is wrong for `MOVS` and `STOS`: there `0xF3` is plain `REP` and `0xF2`
   behaves identically (confirmed against both Bochs and hardware), so only the
   counter decides.

`scripts/fix_cpp_model.py` (fix 7) reads the rep field, inserts the entry test
and recomputes the advance condition. Regression cases: the `rep`/`repne`
`movsb`/`stosb`/`cmpsb` block in `tests/differential.rs`, at counts 0, 1 and 2.

## 4b. Rotates — a translation bug this crate patches

Not a legitimate difference: a second ACL2→Sail translation error, found by the
differential suite and **corrected in the vendored model**. Recorded here so
that anyone regenerating from upstream knows to expect it.

`ROR` sets CF from the **MSB** of the result, `ROL` from the LSB. Upstream ACL2
`rotates-spec.lisp` has this right, using `(logbit size-1 result)` in both the
`1` and the `otherwise` branch. The Sail translation
([`model/rotates_spec.sail`](https://github.com/rems-project/sail-x86-from-acl2/blob/master/model/rotates_spec.sail))
kept the MSB only for count 1 and fell back to `result[0 .. 0]` — ROL's rule —
for every larger count:

```sail
1 => { let cf : bits(1) = logbit(7, result); ... }   // correct
_ => { let cf : bits(1) = result[0 .. 0];   ... }    // wrong: that is ROL's CF
```

So `ror ax, 11` reported CF from the wrong bit. All four widths were affected,
count > 1 only; `ROL`, `RCL` and `RCR` are correct. Bochs and the hardware
backends agreed with each other and with the SDM, which is what identified Sail
as the outlier.

`scripts/fix_cpp_model.py` (fix 5) rewrites the shift amount from 0 to size-1 in
each `ror_spec_N`, and `vendor/sail/model.cpp.gz` ships patched — so the `sail`
backend is correct as distributed. A model you generate yourself is *not*, unless
you run that script, which the regeneration recipe already ends with.

The regression cases are `ror {rax,eax,ax,al}` plus `rol`/`rcl`/`rcr` at
count > 1 in `tests/differential.rs`. The suite missed this for as long as it
did because every rotate case it had used count 1 — the one path Sail got right.
Rotate-by-1 and rotate-by-n are separate branches in every implementation; cover
both.

## 4c. `CMPS` and `CMPXCHG` compare backwards — also patched

A third ACL2→Sail translation error, same treatment. Both instructions hand
`gpr_arith_logic_spec` its `dst` and `src` the wrong way round, so the flags
describe the negated difference:

| | ACL2 | Sail translation |
|---|---|---|
| `x86-cmps` | `(gpr-arith/logic-spec size *OP-SUB* src1 src2 ...)` where `src1 = [rSI]`, `src2 = [rDI]` | `gpr_arith_logic_spec(size, 4, dst, src, ...)` — `[rDI] - [rSI]` |
| `x86-cmpxchg` | `(gpr-arith/logic-spec size *OP-CMP* rAX reg/mem ...)` | `gpr_arith_logic_spec(size, 8, reg_mem, rax_var, ...)` — `reg/mem - rAX` |

`ZF` is right either way — which is why `CMPXCHG` still swapped correctly, and
why the one `cmpxchg` case the suite had (equal operands, where both directions
give zero) never noticed. `CF`, `SF`, `AF` and `PF` are all wrong whenever the
operands differ: `cmpsb` with `[rSI] = 5`, `[rDI] = 3` reported `0x93` (the
flags of `3 - 5`) instead of `0x02`.

Fixed by `scripts/fix_cpp_model.py` (fix 6), which swaps the two operands at the
single call site in each function. Regression cases are the `cmpsb`/`cmps
qword`/`cmpxchg` entries in `tests/differential.rs`, all with **unequal**
operands — an equal-operand comparison cannot distinguish `a - b` from `b - a`.

## 5. Faults

Neither backend vectors through an IDT: a fault leaves the state that was
committed before it and reports `StepOutcome::Fault`. But *how much* was
committed before the fault is not architectural, so **only compare register
state when both backends retired**. Compare the `FaultKind` classification
instead — the free-text `msg` is backend-specific by design.

Bochs and the hardware backends additionally expose the exact vector and error
code (`last_vector`, `last_error_code`).

The hardware backends have to work harder for this, because real hardware always
vectors. They install an IDT whose gates land on per-vector `HLT` stubs using an
IST stack, then restore RIP/RSP from the exception frame. Because delivery uses
the IST stack, the guest's own stack is never touched — so a fault is observable
even with `RSP == 0`, matching the model backends. The only residue is up to
48 bytes written to the dedicated IST stack, which lives in the reserved region.

WHP usually avoids even that: with `ExceptionExitBitmap` the exception exits to
userspace *before* delivery, so RIP, RSP and RFLAGS are untouched and nothing is
written to the IST stack at all. `WhpOracle::uses_exception_intercepts()` says
which path is live. The observable outcome is identical either way — that is the
point of having both.

## 6. Address space

- **Sail** (`app_view`): any canonical address.
- **KVM / WHP**: the low 252 MiB, identity-mapped (`ADDRESS_LIMIT`); the page
  tables, GDT, IDT, TSS and IST stack occupy the 4 MiB above it
  (`RESERVED_START`). Both use the same layout, from the same code, so an address
  valid on one is valid on the other. Accesses beyond guest RAM surface as
  `PageFault`.
- **Bochs**: the low 512 GiB, identity-mapped
  (`BochsOracle::ADDRESS_LIMIT`). Its page tables occupy ~2 MiB at
  `BochsOracle::RESERVED_START` (`0x7F_C000_0000`) — do not use that region as
  data. Above the limit there is no mapping, so an access faults.

Keep cross-backend test addresses well below 512 GiB.

## 7. Nondeterminism

`RDRAND`/`RDSEED` are not reproducible references: Bochs returns a fixed PRNG
sequence and Sail does not implement them. On the hardware backends, `RDTSC`,
`RDTSCP` and `CPUID` are non-reproducible too (real time, real CPU identity) —
and CPUID differs between the two hypervisors even on the same machine, since
each synthesizes its own leaves. Exclude them all.

## 8. `is_mapped` granularity

Sail tracks 16 MiB blocks, Bochs 4 KiB pages, and the hardware backends only
record writes made through the oracle's own API (guest stores are invisible to
them). Treat `is_mapped` as a hint about "was anything here ever written", never
as a portable predicate.

## 9. Throughput (why it matters for campaign design)

Measured with `tests/throughput.rs` on one machine (Core i9-13900K), under both
OSes — Linux via WSL2, Windows natively:

```sh
# Linux
cargo test --release --features bochs,hardware --test throughput -- --ignored --nocapture
# Windows
cargo test --release --target x86_64-pc-windows-msvc --features bochs,whp \
    --test throughput -- --ignored --nocapture
```

| Backend | steps/s | new() |
|---|---|---|
| sail (Linux) | ~16 k | 0.2 ms |
| sail (Windows) | ~19 k | 0.2 ms |
| bochs (Linux) | ~33 M | 4 ms |
| bochs (Windows) | ~33 M | 2 ms |
| kvm (Linux, **nested**) | ~155 k | 2 ms |
| whp (Windows) | ~265 k | 7 ms |

**Do not read those last two rows as "WHP is faster than KVM".** They were taken
at different levels of the virtualization stack: the Linux figure comes from
WSL2, which is itself a Hyper-V guest, so that KVM is nested (L2) and every step
pays two layers of exit, while WHP talks to the host hypervisor directly (L1).
Each number is accurate for *a campaign run that way on this machine*; neither is
a measurement of the hypervisor driver in isolation. A bare-metal Linux host
would need its own run.

What the two do have in common is the shape of the cost: both are dominated by
one VM entry/exit per step, which puts them in the 10⁵ range rather than the
emulators' 10⁷, and that is a floor rather than something to optimize away. WHP
pays more per step in principle — one `WHvRunVirtualProcessor` plus two or three
register calls (arm TF, read state), each its own transition, where KVM gets
registers back through the shared `kvm_run` page — so on equal footing expect it
at or below KVM, not above.

Caveat on the emulator figures: the loop re-executes one instruction from a fixed
address, which is the best case for Bochs' decode cache — a real campaign with
varied encodings will be far below 33 M.

The Sail model is the slow one at ~55-65 µs/step. That is inherent: the ACL2 model
is untyped, so the translation falls back to arbitrary-precision integers
(`sail_int`, ~34 k uses in the generated code) wherever it cannot infer a
bitvector width, and every one of those is a GMP operation. Unlike the ARM Sail
model it uses no rationals (`mpq` count is zero), so this is much cheaper than
that model's exact-rational FP paths, but it still sets the pace.

Practical consequences:

- Put the Sail backend on the *inside* of a campaign loop only for the integer
  ISA it is actually authoritative for; use Bochs or a hardware backend for volume.
- `SAIL_SYSTEM_GMP=1` (or the `system-gmp` feature) links the real libgmp
  instead of the bundled mini-gmp, which is faster for the wide paths. On a
  statically linked target (Rust's musl toolchain) this needs `libgmp.a`, not
  just the shared object — on Alpine that means the `gmp-static` package.
- Instance creation is cheap on every backend, so one oracle per test case is
  fine. A WHP partition is the priciest at ~7 ms, which only starts to matter if
  a case is a handful of steps long — there, reuse one oracle and reset it.
