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
  including AVX-512. What it *claims* is the selected CPU model's CPUID: the
  shim picks `sapphire_rapids`, the widest **AVX-512-capable** model in cpudb
  (the full AVX-512 family, CET, MOVDIR*, WAITPKG). Since 2026-09-14 the shim
  also layers every extension the core implements but that model lacks on top
  of it through Bochs' own `add_features` parameter — AVX10.2 (+MOVRS), the
  Arrow-Lake VEX groups (`avx_ne_convert`, `sha512`/`sm3`/`sm4`, `avx_ifma`,
  `avx_vnni_int8/16`, `cmpccxadd`), `avx512vp2intersect`, AMX-FP16/COMPLEX/
  FP8/AVX512/MOVRS, and AMD's XOP/FMA4/TBM/SSE4a/3DNow! — see §4q for the list,
  the reasoning and the caveats. What it still cannot execute is what this
  Bochs revision has no handlers for: AVX512-ER/PF, 4FMAPS, Key Locker,
  AMX-TRANSPOSE/TF32, LWP.
  An earlier revision asked for the nonexistent model name `bx_generic`, which
  `set_by_name` ignores — leaving the build default `corei7_haswell_4770`
  silently selected, so every EVEX encoding (and `kmov*`) raised #UD despite
  the core being compiled with EVEX support. The shim now aborts on an unknown
  model name, and `tests/bochs_patches.rs` pins that AVX-512 and CET really are
  claimed.
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

## 4d. `MOVD`/`MOVQ` to an XMM register merged instead of zero-extending — patched

`x86-movd/movq-to-xmm` (66 0F 6E, all operand shapes) wrote only
`operand_size` (4 or 8) bytes of the destination XMM register, leaving the
rest as it was. The SDM is explicit that the destination is **zero-extended**
across the full register, and Bochs and real hardware both do so. On a
reset-zero register the two behaviours coincide, which is why the suite never
noticed before it started seeding dirty vector registers.

Fixed by `scripts/fix_cpp_model.py` (fix 8): the value passed to the register
write is already the zero-extended unsigned source, so the write size becomes
the full 16 bytes. Regression cases are the `movd`/`movq ... (zero-ext)`
entries in `tests/differential.rs`, each with a dirtied destination.

## 4e. `MOVDDUP`/`MOVSLDUP` executed as `MOVLPS` — patched

x86isa implements neither MOVDDUP (F2 0F 12) nor MOVSLDUP (F3 0F 12), but its
two-byte dispatch routed **all four** mandatory prefixes of 0F 12 to
`x86-movlps/movlpd`. The F2/F3 forms therefore executed silently with
MOVLPS/MOVLPD semantics — a 64-bit load into the low half — leaving the high
half stale where the real instructions duplicate lanes. The neighbouring
unimplemented prefix arms (F3 0F 16 MOVSHDUP, F3 0F 7E MOVQ) already raise the
model's "Opcode Unimplemented in x86isa!" error, which the shim classifies as
`FaultKind::Unimplemented` — a skippable coverage gap rather than a wrong
answer.

Fixed by `scripts/fix_cpp_model.py` (fix 9): the F3 and F2 dispatch arms of
0F 12 now raise that same error. Regression cases are the `movsldup`/`movddup`
entries in `tests/differential.rs` (sail skips them; backends that implement
the instructions are diffed on the real semantics) plus the `movlps`/`movlpd`
entries pinning that the legitimate arms kept their meaning.

## 4f. CET shadow stacks are opt-in

Only Bochs models CET, and even there the reset state leaves it **off**:
`X86Oracle::enable_shadow_stack(base, len)` answers
`Err(ShadowStackError::Unsupported)` on every other backend, which a diff
harness should treat exactly like `Unimplemented` — skip the backend, do not
fail the case. The other failure, `BadRange`, is deliberately a separate
variant: it says the backend *has* CET and rejected the arguments, which is a
caller bug and must not be swallowed as "no CET here".

`Unsupported` is a runtime answer, not just a compile-time one. Bochs gates the
CET instructions on the CPU model claiming the extension, so the backend asks
`is_cpu_extension_supported(BX_ISA_CET)` before touching CR4 — writing CR4.CET
regardless would report success while every CET instruction still raised `#UD`.
Enabling also sets CR0.WP, which the architecture requires before CR4.CET (the
reset state already has it; only a caller that wrote CR0 first would notice).

Enabling it is a real change of machine, not a flag: `CALL` starts pushing the
return address to the shadow stack and `RET` pops and *compares* it, so a caller
must also point `set_ssp` into the mapped region. That is why the plain reset
state cannot have it on — every existing call/ret case would need a shadow stack
to run.

What "mapped" means is a page attribute, so `enable_shadow_stack` rewrites the
identity map: the leaf entry gets Dirty=1 and R/W=0 (Bochs enforces exactly this
in `check_leaf_entry_faults`), which is what makes an ordinary store fault while
`WRSS` and the CPU's own pushes succeed. Granularity is the identity map's 2 MiB
frame — keep ordinary data out of the frames you convert. Seeding is unaffected:
the memory accessors reach physical memory without walking the page tables.

Because the granularity is a whole frame, a range sharing one with the identity
map's own page tables (`BochsOracle::RESERVED_START`, which lies *inside* the
mapped address space and so is not excluded by the address limit) is refused
outright rather than trusted to the caller: accepting it would hand the CPU a
shadow stack on top of PML4, and the first `CALL` would overwrite it — silently,
since the walk keeps hitting stale TLB entries long after. `enable_shadow_stack`
reports a refused range with the same `false` it uses for "this backend has no
CET", so validate the range before reading that as a capability answer.

A failed shadow-stack check raises **`#CP`**, which `FaultKind` names in its own
right (`ControlProtection`). Distinguishing it from `#PF` is the whole point of
a negative CET case: a `#PF` there means the case set the stacks up wrong and
never reached the comparison.

Three things surprise people writing cases here:

- **A `CALL` with displacement 0 does not push.** Bochs guards the shadow-stack
  push with `i->Id() != 0`, so the PIC idiom `call $+5` proves nothing. Use a
  non-zero displacement.
- **An ordinary store right after a `WRSS` to the same page may not fault**: it
  hits the TLB entry the `WRSS` just installed. Test the fault on a fresh
  instance, or before the first shadow-stack access to that page.
- **A fault is the end of a case.** There is no IDT, so the `#PF` an ordinary
  store takes leaves the CPU unable to retire anything after it.

`RDSSPD`/`RDSSPQ` are the exception to all of this: they are ungated in Bochs'
decoder (they must be NOPs on pre-CET CPUs), so they decode with CET off and
merely do nothing. Every other CET instruction is `#UD` until the extension is
enabled — which the `sapphire_rapids` CPU model claims natively (an earlier
revision added it via `add_features = "cet"` on a model without it), so with
shadow stacks off they report the ISA's answer for "disabled" rather than "not
implemented".

That last part is **not** opt-in and cannot be: the CPU model is fixed before
`initialize()`, long before anyone calls `enable_shadow_stack`. So every Bochs
instance decodes the CET encodings and reports CET in `CPUID.(EAX=7).ECX[7]`,
including instances that never enable shadow stacks. Nothing in the suite
compares CPUID today (`tests/sail_backend.rs` only expects Sail to report it
unimplemented), but a future CPUID diff would see Bochs claim CET where the KVM
or WHP host may not — that is a difference in the *model*, not a bug in either.
`SSP` is likewise not part of `CpuState`, so it never enters a state diff.

## 4g. `KSHIFTLW`/`KSHIFTRW` zeroed at count 15 — patched (Bochs)

The first Bochs semantic bug this suite found (4b–4e were all model-side bugs
on the ACL2/Sail branch): `cpu/avx/avx512_mask16.cc` guards the 16-bit mask
shifts with `count < 15` where the SDM says `COUNT ≤ 15` — so
`kshiftlw k, k, 15`, the idiomatic top-bit isolate, returns 0 instead of
0x8000. A pure typo: the 8/32/64-bit siblings all use `count < width`.

Found by the zens k-op differential suite the first day the Bochs backend
actually executed AVX-512 (see §2 — the CPU-model fallback had kept it #UD
until then); no second backend could confirm (the host CPU lacks AVX-512, Sail
has no vector ISA), but the SDM text and the sibling-width pattern are
unambiguous.

Still present on upstream master as of 2026-08-19 (`d5c0ad9`), so this crate
carries the fix itself, as
[`patches/bochs/0001-kshiftlw-kshiftrw-count-15.patch`](../patches/bochs/0001-kshiftlw-kshiftrw-count-15.patch).
`scripts/vendor-bochs.sh` applies it after copying, so re-vendoring replays it
instead of silently reverting it, and `tests/bochs_patches.rs` fails if it ever
stops taking effect. If a future bump makes it stop applying because upstream
took the fix, delete the patch and say so here — do not force it through.

## 4h. `VPCONFLICTD/Q` — two compounding bugs, fixed upstream

Found by the zens phase-5 differential suite the day the family landed, against
the then-vendored Bochs `b64f49e` (2025-05-13). Two bugs stacked:
`simd_pconflictd/q` (`cpu/simd_int.h`) looped `i < index-1` — an off-by-one
that never compares a lane with its immediate predecessor, though its own
comment says "all previous elements" — and the `VPCONFLICTD/Q` handlers
(`cpu/avx/avx512_bitalg.cc`) wrote each lane's result back over the copy of the
source they were still reading, so lane 2 onward compared against conflict
bitmaps instead of source values. `[7,3,7,7,…]` returned all-zeros where the
SDM (and the zens model) give lanes 2/3 the bits 0x1/0x5.

**No longer patched here.** Upstream fixed both on 2026-06-09 ("fixed bug in
VPCONFLICTD/VPCONFLICTQ instructions" plus "complete fix for VPCONFLICT*"),
taking `i < index` in the helper and iterating the handlers backwards so the
in-place write is safe. Bumping to `d5c0ad9` picked that up, and the vendored
tree is stock here again. Kept as a record: it is the reason the pin is no
longer allowed to drift far behind upstream. The same bump also brought
upstream fixes for masked `VPEXPANDB/W` with a memory operand, missing
zero-upper on `VPBROADCASTMB2Q/MW2D`, masked `VPMULTISHIFTQB`, MAXVL being read
from the encoding rather than XCR0, `VEXTRACT*` masked memory fault
suppression, zero-mask `VMOVAPS/APD` still accessing memory, `VPSHUFBITQMB`,
and spurious #PE from `VREDUCE*` — every one of which this suite would
otherwise have rediscovered one instruction at a time.

## 4i. `VCVTTPH2{,U}DQ` / `VCVTTPH2{,U}QQ` decoded to the non-truncating handlers — patched (Bochs)

Found by the zens FP16 differential suite the day the FP16 converts landed.
`decoder/ia_opcodes_evex.def` bound all eight truncating packed fp16-to-dword
and fp16-to-qword rows (`BX_IA_EVEX_VCVTTPH2DQ_VdqWph`, `..UDQ..`, `..QQ..`,
`..UQQ..`, each plus its `_Kmask` twin) to the NON-truncating execute methods
(`VCVTPH2DQ_VdqWphR` and friends) — and to the non-truncating disassembly
strings, so even the internal disasm hid the difference. The truncating
handlers (`VCVTTPH2DQ_VdqWphR`, ..., keyed on `f16_to_i32_round_to_zero` etc.)
exist in `avx/avx512_cvt16.cc` but were never referenced: a copy-paste bug in
the def rows. `vcvttph2dq` of -367.5 returned RN's -368 instead of truncation's
-367. The word-sized rows (`VCVTTPH2{,U}W`) and the scalar `VCVTTSH2{,U}SI`
rows were bound correctly, which is what narrowed it to the def table rather
than the softfloat layer.

Nothing else about those rows was wrong: the load functions, operand kinds, ISA
gate and EVEX attributes already matched their non-truncating siblings, so the
fix is eight two-token substitutions and nothing more.

Still present on upstream master as of 2026-08-21 (`7cf9830`), so this crate
carries the fix itself, as
[`patches/bochs/0002-vcvttph2-truncating-handlers.patch`](../patches/bochs/0002-vcvttph2-truncating-handlers.patch).
`scripts/vendor-bochs.sh` applies it after copying, so re-vendoring replays it
instead of silently reverting it, and `tests/bochs_patches.rs` fails if it ever
stops taking effect — on both the masked and unmasked encodings, with the
non-truncating twins asserted alongside so that binding *everything* to the
truncating handler cannot pass either. zens' `test_fp16_cvt_directed` and the
FP16 template sweep cover it downstream, but that is the consumer's suite; it
does not protect this vendored tree.

One sibling shape looked the same at first and is not. The AVX10.2 fp8
converts (`VCVTPH2BF8S`, `VCVT2PH2BF8S`, `VCVTBIASPH2BF8S` and their `HF8`
counterparts) are also bound to the same handlers as their non-saturating
twins — but there the handler itself tells the two apart, deriving a
`saturate` flag from the IA opcode id and passing it down to the
`convert_ne_fp16_to_{bf8,hf8}` / `convert_truncate_fp16_to_*_bias` helpers in
`avx/bf8.h` / `avx/hf8.h`. Four of the six compare against their `S` id; the
two-source `VCVT2PH2BF8` / `VCVT2PH2HF8` compared against the PLAIN id, so
their saturation was inverted. Unreachable while AVX10.2 was off (§2); now that
it is on (§4q) it is fixed as §4r.

## 4j. `VPGATHERQD` / `VGATHERQPS` with a zmm index left the ymm destination's upper 256 bits stale — patched (Bochs)

Found by the zens gather/scatter differential suite the day the group landed.
The q-index d-element gathers produce a result half the index width, so the
EVEX handler `VGATHERQPS_MASK_VpsVSib` (shared by `vpgatherqd`) must zero the
destination above 256 bits for a zmm index and above 128 for a ymm index (SDM:
`DEST[MAXVL-1:VL/2] := 0`). The code derived the clear length as `len--`, which
is right for VL256 (2 → 1 = BX_VL128) but yields 3 at VL512 — a value
`BX_CLEAR_AVX_REGZ` matches against neither BX_VL256 nor BX_VL128, so it cleared
NOTHING and bits 256..511 of the ymm destination kept whatever the register
held. `len >>= 1` is the intended arithmetic (4 → 2 = BX_VL256).

The VEX form (`VGATHERQPS_VpsHps`) was already correct — its result never
exceeds xmm and it clears above 128 unconditionally — as were the d-index and
q-element EVEX gathers, whose results are full width. Only the one VL512
q-index d-element shape was affected, on both `vpgatherqd` and `vgatherqps`.

Still present on upstream master as of 2026-09-12, so this crate carries the
fix itself, as
[`patches/bochs/0003-vpgatherqd-zmm-index-upper-clear.patch`](../patches/bochs/0003-vpgatherqd-zmm-index-upper-clear.patch),
applied by `scripts/vendor-bochs.sh` like its siblings, and
`tests/bochs_patches.rs` fails if it ever stops taking effect — on both
`vpgatherqd` and `vgatherqps` with a zmm index, seeding the destination with
stale bits so "cleared nothing" is visible, and with a partial mask so a fix
that zeroed the *whole* register cannot pass either; the ymm-index form is
asserted alongside as the boundary the fix must not move. zens'
`test_gather_directed` / `test_gather_templates` cover it downstream.

## 4k–4o. AMX — enabled 2026-09-12, and five Bochs bugs that surfaced at once

Until 2026-09-12 the vendored core was configured WITHOUT AMX (`config.h`
had `BX_SUPPORT_AMX 0`, so `cpu/avx/amx.cc` compiled to nothing and the
sapphire_rapids model did not advertise the feature) and the shim's XCR0 left
bits 17/18 (XTILECFG/XTILEDATA) clear, so every AMX instruction was #UD on the
Bochs backend the same way it is on a host without AMX. `configure` now runs
with `--enable-amx` (`scripts/vendor-bochs.sh`, PROVENANCE), `config.h`
carries `BX_SUPPORT_AMX 1`, and `csrc/bochs_shim.cpp` starts the vCPU with
XCR0 = 0x600E7. That lights up AMX-TILE, AMX-INT8 and AMX-BF16 — the subset
Sapphire Rapids has (no AMX-FP16/FP8/COMPLEX: those stay #UD, as on the
hardware). The XSAVE layout is unchanged for the components that existed
before; TILECFG lands at offset 2752 and TILEDATA at 2816 (`cpu/crregs.h`,
the Intel layout this core already used for opmask@1088 … PKRU@2688).

Running the zens AMX suite against the freshly enabled core found five bugs in
the vendored AMX code — all in code that had never executed in this crate
before, and all still present on upstream master as of 2026-09-12. Each is
carried as a patch under `patches/bochs/`, applied by `scripts/vendor-bochs.sh`
like its siblings.

### 4k. `STTILECFG` decoded at VEX.W1 — patched (Bochs)

`BxOpcodeGroup_VEX_0F3849` bound `BX_IA_STTILECFG` to `ATTR_VEX_W1`. The
instruction is VEX.128.66.0F38.**W0** 49 /0 (SDM), which is what LLVM and GNU
as emit, so the real encoding matched nothing and raised #UD unconditionally
while `ldtilecfg`/`tilerelease`/`tilezero` in the same group correctly asked
for W0.
[`patches/bochs/0004-sttilecfg-vex-w0.patch`](../patches/bochs/0004-sttilecfg-vex-w0.patch).

### 4l. `xsave_tilecfg_state` swapped rows and bytes_per_row — patched (Bochs)

The routine behind `STTILECFG` and XSAVE component 17 wrote `tilecfg[n].rows`
into the 16-bit colsb slots (bytes 16..31) and `bytes_per_row` into the row
bytes (48..55) — the reverse of the architectural image and of
`configure_tiles`' own read side. A configuration therefore did not round-trip:
`ldtilecfg` of `{rows 2, colsb 12}` followed by `sttilecfg` stored `{colsb 2,
rows 12}`, and an XSAVE/XRSTOR pair reconfigured every tile transposed.

### 4m. TILEDATA save/restore covered 8 of 16 rows — patched (Bochs)

`xsave_tiledata_state` / `xrstor_tiledata_state` looped `row <
BX_TILE_REGISTERS` (8) instead of `BX_TILE_MAX_ROWS` (16), so only the first
4 KiB of the 8 KiB component moved; rows 8..15 of every tile stayed stale in
the save area on XSAVE and in the registers on XRSTOR.

### 4n. AMX-INT8 dot products ignored byte signedness — patched (Bochs)

`DPBDSS`/`DPBDSU`/`DPBDUS` unpacked each dword into a `const Bit8u` array and
formed the products as `Bit32s(xbyte[i]) * Bit32s(ybyte[i])`; converting an
unsigned byte to `Bit32s` gives 0..255, so no sign extension ever happened and
all four `TDPB??D` instructions computed the unsigned×unsigned product:
`tdpbssd` on the byte pair `0xFF · 0x01` accumulated 255 where −1 was due, so
every INT8 GEMM with a byte ≥ 0x80 on a signed side was wrong (only `tdpbuud`
was correct).
[`patches/bochs/0006-amx-int8-byte-signedness.patch`](../patches/bochs/0006-amx-int8-byte-signedness.patch).

### 4o. TILEDATA XINUSE inverted — patched (Bochs)

`xsave_tiledata_state_xinuse` returned `tile_use_tracker == 0`, i.e. "in use"
exactly when no tile had been touched. XSTATE_BV[18] was therefore 0 after a
`tileloadd` and 1 on a fresh machine, and XSAVEOPT/XSAVEC would skip live tile
data while saving empty tiles.

4l, 4m and 4o are one patch,
[`patches/bochs/0005-amx-xsave-tilecfg-tiledata.patch`](../patches/bochs/0005-amx-xsave-tilecfg-tiledata.patch).
Downstream, zens' `test_tilecfg` (the LDTILECFG/STTILECFG round trip — 4k and
4l), `test_amx_int8` / `test_amx_int8_random` (all four INT8 variants against a
reference GEMM — 4n) and `test_amx_fp16_bf16` diff against this backend and
agree bit for bit with the patches in place. 4m and 4o were observed through
XSAVE with RFBM = bits 17+18 (XSTATE_BV[18] came out 0 after a `tileloadd`,
and tile rows 8..15 never reached the area); zens keeps that case engine-only
because its model uses the MPX-less component offsets, so the regression for
those two lives in `tests/bochs_patches.rs` here. That file pins all five
against the SDM and fails if any of 0004–0006 stops taking effect: the
LDTILECFG→STTILECFG round trip with rows ≠ colsb on every tile (4k, 4l), XSAVE
of a freshly `tileloadd`ed 16-row tile into a stale-filled area checking
XSTATE_BV[18] and all 16 rows (4m, 4o), XSAVE on the AMX init state checking
XSTATE_BV[18] = 0 (4o), XRSTOR of a hand-built image followed by `tilestored`
(4m), and the four `tdpb??d` variants on a byte quadruple that gives each
signedness combination its own answer (4n; `tdpbuud` is the control).

Enabling AMX also exposed a reset gap: Bochs' `reset(BX_RESET_HARDWARE)`
clears the x87, MXCSR, zmm and opmask state but never touches the AMX unit
(`cpu/init.cc` has no `amx->clear()`), so on this crate's singleton core a
tile configuration, tile data and the in-use tracker — hence XSTATE_BV[17:18]
— survived from one `BochsOracle` into the next. Real hardware leaves RESET
with AMX in its init state. `csrc/bochs_shim.cpp`'s `enter_long_mode` now
calls `amx->clear()` alongside its own register zeroing, and
`tests/bochs_patches.rs` loads a tile in one instance and checks the next one
reads an all-zero `sttilecfg` image and XSTATE_BV[17:18] = 0. This is a shim
fix, not a patch: it is about the oracle's fresh-instance contract, which
upstream has no equivalent of.

### 4p. Stale traces after a store into executed code once XCR0 has AMX bits — patched (Bochs)

The second thing XCR0 = 0x600E7 broke was not AMX at all: three x87 cases in
`tests/bochs_embedding.rs` began reading back the state of the *previous*
instruction. `step_bytes` writes the instruction at RIP, steps, and writes a
different instruction at the same RIP — and with the tile bits enabled Bochs
kept executing the first one. The trace cache indexes an entry by
`(pAddr & (entries-1)) ^ fetchModeMask` (`cpu/icache.h`, `hash`), and the
self-modifying-code path `handleSMC` finds the traces a store may have touched
by walking, per 128-byte page line the store hit, the 128 entries that line's
instructions hash to. That is only complete while `fetchModeMask < 0x80`;
`BX_FETCH_MODE_AMX_OK` is bit 7, so the moment `handleAvxModeChange` sets it
every trace lands in the neighbouring line's entries and the walk misses it.
Nothing else invalidates — the shim stamps host writes exactly as guest stores
are stamped (`invalidate_decoded`), and it was that path that stopped working,
not the shim. Any guest OS that enables AMX and then patches code it has
already run would hit the same thing on upstream Bochs.

The hash is deliberately left as it is: nothing flushes the icache when XCR0 or
CR4.OSXSAVE changes, so the mode bits in the index are what keeps a trace
decoded with AMX_OK clear (its AMX instructions bound to `BxNoAMX`) from being
executed after it is set. Instead `handleSMC` walks up to the line an entry can
be displaced into,
[`patches/bochs/0007-icache-smc-walk-fetchmode-bit7.patch`](../patches/bochs/0007-icache-smc-walk-fetchmode-bit7.patch);
the per-entry `traceMask` test is unchanged, so this looks at more entries and
flushes nothing it did not flush before. `tests/bochs_patches.rs` pins it
directly — step one `mov`, overwrite it in place with another, step again — and
the `bochs_embedding.rs` x87 cases are back to green. Still present on upstream
master as of 2026-09-12.

## 4q. Extensions beyond Sapphire Rapids, enabled with `add_features` — 2026-09-14

The vendored core carries execute methods for a long tail of extensions that
`sapphire_rapids` does not advertise, and the CPUID bit is what gates decoding:
an encoding whose `BX_ISA_*` bit is off is #UD, exactly like on a host that
lacks the feature. Until 2026-09-14 that left the zens SIMD groups outside
Sapphire Rapids' feature set with no differential oracle at all. Rather than
switch models — `arrow_lake` has the newer VEX groups but no AVX-512, the AMD
models have XOP/3DNow! but nothing past AVX — the shim now sets Bochs' own
`cpu.add_features` parameter (`cpu/init.cc:add_remove_cpuid_features`, the
same list a bochsrc `cpu: add_features=` line takes) before `initialize()`,
with `cpu_added_features` in `csrc/bochs_shim.cpp`:

| group | feature names | what becomes executable |
|---|---|---|
| AVX10.2 | `avx10_1,avx10_2,movrs,avx10_2_movrs` | the bf16 arithmetic/compare/manipulation family, `v{u}comxs*`, `vminmax*`, the `ibs`/`iubs` and saturating-truncation converts, the fp8 converts, `vcvt2ps2phx`, `vdpphps`, `vmovrs*` (and the GPR `movrs`, which the sanity check ties to the vector half) |
| Arrow-Lake VEX | `avx_ne_convert,sha512,sm3,sm4,avx_ifma,avx_vnni_int8,avx_vnni_int16,cmpccxadd` | `vbcstne*`/`vcvtnee*`/`vcvtneo*`, `vsha512*`, `vsm3*`, `vsm4*` (EVEX forms need `avx10_2` too), the `{vex}` IFMA/VNNI-INT twins, `cmp<cc>xadd` |
| Tiger Lake | `avx512vp2intersect` | `vp2intersectd/q` |
| AMX | `amx_fp16,amx_complex,amx_fp8,amx_avx512,amx_movrs` | `tdpfp16ps`, `tcmm*`/`tconj*`, `tdp{b,h}{b,h}f8ps`, `tcvtrow*`/`tilemovrow`, `tileloaddrs*` (XCR0 already has bits 17/18, §4k) |
| AMD | `xop,fma4,tbm,sse4a,3dnow,3dnow_ext` | the XOP map (`vpcom*`, `vpperm`, `vprot*`, `vpsha*/vpshl*`, `vphadd*`, `vpmacs*`, `vfrcz*`, `vpermil2*`), FMA4, TBM, `extrq`/`insertq`/`movnts{s,d}`, the whole `0F 0F` 3DNow! block plus `femms` |

Every name is a `x86_feature(...)` string in `cpu/decoder/features.h`; an
unknown one is `BX_PANIC` at init, and `cpuid.cc:sanity_checks()` enforces
the dependency chains the list satisfies (`avx10_2 → avx10_1 → AVX2`,
`amx_avx512 → avx10_2`, `movrs + avx10_2 ⇔ avx10_2_movrs`, `tbm → xop → AVX`,
`3dnow_ext → 3dnow → MMX`).

What this means for a consumer:

- The CPU Bochs now describes exists nowhere — Intel AVX10.2 next to AMD
  3DNow!. That is harmless for an oracle: each encoding executes the
  semantics Bochs gives *that* extension, and CPUID was already outside the
  diff (§7). The hardware backends keep raising a genuine #UD for whatever the
  host lacks, which the harness treats as a skip, so a case gains Bochs
  coverage without losing anything.
- Decoding changes at the edges, all of them from "#UD" to "executes": the
  `0F 0F` block and `femms`, the XOP `8F` prefix (ModRM.reg ≠ 0 — never a legal
  `pop` in 64-bit mode) and the FMA4 `0F3A 5C–7F` rows. No case in this crate
  relied on any of those being #UD. One thing that does NOT change is EVEX.b
  on a register form: Bochs' decoder treats it as "512-bit vector length,
  L'L is the rounding control" unconditionally (`fetchdecode64.cc`, the
  `if (i->getEvexb()) i->setVL(BX_VL512)` under `modC0()`), so a 256-bit
  register form with EVEX.b set was never #UD here — it silently ran as a
  512-bit operation with embedded rounding before `avx10_2` was on, and still
  does. The only EVEX.b #UD is the per-opcode `BX_PREPARE_EVEX_NO_SAE` check
  in `assignHandler`, which no ISA bit influences. (The AVX10 spec's early
  revisions gave AVX10.2 a 256-bit embedded-rounding form; rev 3.0 removed it,
  and Bochs never implemented it.)
- None of these handlers had ever executed in this crate. The AMX precedent
  (§4k–§4o: five bugs the first day) says to expect the same here; reading the
  handlers before flipping the switch already found §4r. `tests/bochs_patches.rs`
  pins the list: one representative encoding per feature name retires
  (`vminmaxps`, `vmovrsb`, `movrs`, `vbcstnebf162ps`, `vsha512msg1`,
  `vsm3msg1`, `vsm4key4`, `{vex} vpmadd52luq`, `vpdpbssd`, `vpdpwsud`,
  `cmpoxadd`, `vp2intersectd`, `vpcomltb`, `vfmaddps`, TBM `bextr`, `extrq`,
  `pfadd`, `pswapd`, and after an `ldtilecfg` `tdpfp16ps`, `tcmmimfp16ps`,
  `tdpbf8ps`, `tcvtrowd2ps`, `tileloaddrs`), while `vexp2ps`, `encodekey128`
  and `vp4dpwssd` are asserted to stay #UD as the boundary.
- CPUID only half-agrees. The `sapphire_rapids` model derives leaf 7.1 EAX,
  leaf 7.0 EDX, leaf 0x80000001 ECX and leaf 0x1E.1 EAX from the ISA set, so
  SHA512/SM3/SM4, CMPCCXADD, AMX-FP16, AVX-IFMA, MOVRS, VP2INTERSECT, SSE4a,
  XOP, FMA4, TBM and the AMX extensions do show up. But it hard-codes leaf 7.1
  EDX to zero, has no leaf 0x24 case at all, and reports 0x80000001 EDX
  through an Intel-only helper — so AVX10 (both the 7.1 EDX bit and the
  version leaf), AVX-VNNI-INT8/16, AVX-NE-CONVERT, AMX-COMPLEX and 3DNow! are
  invisible to a CPUID probe even though their encodings execute. Decoding is
  gated by the ISA bitmask, not by what CPUID says, so this changes nothing
  for the oracle; it matters only to a consumer that builds skip lists from
  the backend's CPUID instead of from the #UD it reports.
  `tests/bochs_patches.rs` checks the bits that are reported.
- Nothing was deliberately left off. What is missing cannot be enabled:
  `avx512er`/`avx512pf` are commented out of `features.h` (no `vexp2*`,
  `vrcp28*`, `vrsqrt28*`, prefetch gathers), and Key Locker, AVX512-4FMAPS,
  AMX-TRANSPOSE (`t2rpntlvwz*`, `ttransposed`, `ttdp*`, `ttcmm*`),
  AMX-TF32 (`tmmultf32ps`) and LWP have no handlers in this Bochs revision.

## 4r. `VCVT2PH2BF8` / `VCVT2PH2HF8` saturation inverted — patched (Bochs)

The six AVX10.2 fp16→fp8 converts each have a plain and a saturating (`S`)
spelling bound to ONE execute method, which derives a `saturate` flag from the
IA opcode id. `VCVTPH2BF8/HF8` and `VCVTBIASPH2BF8/HF8` compare against their
`S` id; the two-source `VCVT2PH2BF8_Vf8HdqWphR` and `VCVT2PH2HF8_Vf8HdqWphR`
compared against the PLAIN `_Kmask` id (`avx/avx10_2_cvt_fp8.cc`), so the plain
form clamped overflow to the largest finite fp8 value and the `S` form let it
go to Inf (BF8) / NaN (HF8, which has no Inf) — exactly backwards, for any
input beyond the fp8 range (|x| > 57344 for BF8, > 448 for HF8). In-range
inputs, the NaN paths and the mask handling were unaffected.

Comparing against the `S` id, as the other four handlers do, is the whole fix:
[`patches/bochs/0008-vcvt2ph2-fp8-saturate-opcode-key.patch`](../patches/bochs/0008-vcvt2ph2-fp8-saturate-opcode-key.patch),
applied by `scripts/vendor-bochs.sh` like its siblings. Found by reading the
handlers §4q was about to make reachable, not by a diff — which is also why
§4i's earlier note ("no saturating handler at all") was wrong: the saturating
path exists in `avx/bf8.h` / `avx/hf8.h`, it was only keyed wrongly for two of
the six. Still present on upstream master as of 2026-09-14.

## 4s. AVX512-BF16: `VCVTNEPS2BF16` merge mask zeroed, `VCVTNE2PS2BF16` read src1 above VL — patched (Bochs)

Two defects in `avx/avx512_bf16.cc`, found 2026-09-14 by diffing an
independent model of the three AVX512-BF16 instructions (`vdpbf16ps` was
clean):

- `VCVTNEPS2BF16_MASK_VphWpsR` under a MERGE mask (`{k}` without `{z}`)
  blended the converted words into the live destination register and then
  unconditionally wrote its local `dst` — holes still zero from `dst.clear()`
  — over it with `BX_WRITE_AVX_REGZ`. Merge masking therefore behaved exactly
  like zeroing masking: `vcvtneps2bf16 xmm1 {k7}, ymm2` with k7 = 0x96 left
  words 0, 3, 5, 6 as 0 where hardware keeps the old xmm1 words. The fix
  blends the OLD destination's words into the local result for the unselected
  lanes (inverted mask) and keeps the existing `BX_WRITE_AVX_REGZ`, which is
  what zeroes the words above the converted count — the destination is half
  the source's width, so the sibling handlers' blend-into-the-register +
  `BX_CLEAR_AVX_REGZ(len)` idiom (`avx512_cvt16.cc`) would leave the
  destination's upper half stale here (the differential caught exactly that
  on a first attempt).
- `VCVTNE2PS2BF16_MASK_VphHpsWpsR` fills the low half of the destination from
  src2 and the high half from src1, but indexed src1 with the running word
  index `n` instead of `n - DWORD_ELEMENTS(len)`. At VL128/VL256 the high half
  came from src1's dwords ABOVE the vector length (register bits 128..255 /
  256..511 — wrong values, and different from the SDM's `SRC1.fp32[j - KL/2]`);
  at VL512 it read past the 64-byte local (undefined behaviour — the
  differential happened to pass there, which is what UB looks like).

Both are fixed by
[`patches/bochs/0009-avx512-bf16-merge-mask-and-src1-index.patch`](../patches/bochs/0009-avx512-bf16-merge-mask-and-src1-index.patch),
applied by `scripts/vendor-bochs.sh` like its siblings. `tests/bochs_patches.rs`
pins both — the merge case seeds every chunk of the destination and checks
words above the converted count too, so the blend-into-the-register variant
fails there, and the two-source case gives every dword of both sources a
distinct value at all three vector lengths. Still present on
upstream master as of 2026-09-14. Sapphire Rapids has AVX512-BF16 natively, so
no `add_features` entry is involved; the VEX `vcvtneps2bf16` row
(`avx_ne_convert`, `avx/avx_ne_convert.cc`) is a separate, unmasked handler and
was correct.

## 4t. AVX10.2: IBS converts sign-extended and mis-handled NaN, VMINMAX copied a NaN src1's sign, BF16 FMA rounded twice and multiplied the wrong operands, VCOMXSS/VCOMXSD swapped, VCVT2PH2{B,H}F8 lost its upper half at VL512, VMOVRS ignored its mask — patched (Bochs)

Eight defects found 2026-09-14 by diffing an independent model of the AVX10.2
group (the BF16 arithmetic family, VMINMAX, VCOMX, the saturating converts,
VCVT2PS2PHX, VDPPHPS, the FP8 converts and VMOVRS); each pinned by the Intel
AVX10.2 Architecture Specification (361050-007) text:

- `VCVT[T]{PS,PH,BF16}2IBS` wrote the signed byte SIGN-extended into its
  32/16-bit element (the handler macros assign an `int8_t` to a `Bit32u`/
  `Bit16u` lane) where the spec zeroes the upper bytes: `vcvtps2ibs` of -1.0
  gave 0xFFFFFFFF instead of 0x000000FF. Zero-extending wrappers are now bound
  to the twelve signed handlers (`avx_cvt.cc`, `avx512_cvt.cc`,
  `avx512_cvt16.cc`, `avx10_2_bf16.cc`); the unsigned forms were correct.
- `f32_to_i8` / `f32_to_ui8` (the rounding converts behind `VCVTPS2I[U]BS` and
  `VCVTBF162I[U]BS`) had their NaN check compiled out — the `#if` guard tests
  the i32/ui32 specialize constants, which are all equal — so a NaN fell into
  the overflow path and returned ±127/-128 (0xFF/0) by sign. The spec says
  "For NaN, (0) is returned", as the truncating and f16 siblings already did.
- `f{16,32,64}_minmax` with sign control 0b00 copied src1's sign onto a
  NUMBER result when src1 was the NaN of a `*Number` operation; the spec's
  table 11.4 says the sign control is ignored there ("does not copy the sign
  of SRC1 ... if SRC1 is a NAN").
- `bf16_mulAdd` (all twelve `VF[N]M{ADD,SUB}{132,213,231}BF16`) rounded the
  exact `a*b+c` to f32 and then to bf16 — a double rounding that loses a tiny
  addend against a product sitting on a bf16 midpoint. The spec's "infinite
  precision intermediate product ... RNE" is one rounding; the fix computes
  the f32 FMA under round-to-zero, jams the inexact flag into the LSB
  (round-to-odd) and lets the existing RNE narrowing round once. The other
  bf16 helpers are provably free of the problem (an 8×8-bit product is exact
  in f32; sums/quotients/roots of 8-bit operands cannot land within f32
  rounding distance of a bf16 midpoint), so they are unchanged.
- The twenty-four `VF[N]M{ADD,SUB}{132,213,231}BF16` rows were bound to the
  ACCUMULATE-shaped `HANDLE_AVX_3OP` / `HANDLE_AVX512_3OP_WORD_EL_MASK`
  templates (first lane operand = the destination register, src3 never read)
  although they list their operands in the fp16 rows' FMA3 role order, so
  every form multiplied the wrong pair: with dst=2, vvvv=3, rm=5 the 132/213/
  231 forms gave 9/8/11 instead of 13/11/17. New `HANDLE_AVX_3SRC` /
  `HANDLE_AVX512_3SRC_WORD_EL_MASK` templates read src1/src2/src3 (the
  `HANDLE_AVX_PFP_3OP` shape) and the rows are rebound.
- The EVEX `0F 2E`/`0F 2F` opmap rows swapped the two scalar sizes: the
  spec's `VCOMXSS`/`VUCOMXSS` encoding (F3.W0) dispatched the SD handler and
  the `VCOMXSD`/`VUCOMXSD` encoding (F2.W1) the SS one, so `vcomxss` compared
  the low QWORDS as doubles — NaN vs 1.0 came out "greater" (all flags clear;
  both bit patterns are tiny positive doubles). The IA names are swapped
  back (`fetchdecode_opmap_evex.cc`); handlers and `.def` rows were right.
- `VCVT2PH2BF8[S]` / `VCVT2PH2HF8[S]` walk their 2·KL byte lanes with a
  `Bit32u` lane bit and a 32-bit opmask read; at VL512 (64 lanes) the bit
  shifts out after 32 iterations, so the upper half — every byte converted
  from src1 — was never written and `zmm1` kept its old bytes. `Bit64u` and
  `BX_READ_OPMASK` fix it (`avx10_2_cvt_fp8.cc`).
- The MAP5 `6F` opmap group (`VMOVRS{B,W,D,Q}`) put `ATTR_MASK_K0` on its
  `_Kmask` rows instead of the plain ones — the reverse of every other group
  (the attribute marks the row that matches an ABSENT mask) — so a masked
  `vmovrs` ran the unmasked loader: merge holes were overwritten, `{z}`
  zeroed nothing. The attributes are moved to the plain rows.

All eight are fixed by
[`patches/bochs/0010-avx10-2-ibs-zero-extend-nan-minmax-sign-bf16-fma.patch`](../patches/bochs/0010-avx10-2-ibs-zero-extend-nan-minmax-sign-bf16-fma.patch),
applied by `scripts/vendor-bochs.sh` like its siblings. `tests/bochs_patches.rs`
pins each of the eight against the spec text: all twelve I[U]BS rows (masked
and unmasked) with [-1, 2, NaN, -300]; the four packed VMINMAX formats under
both NaN modes; all twelve BF16 FMA forms with 2/3/5 (plain and merge-masked)
plus the midpoint case that separates one rounding from two; the four
VCOMX encodings with a NaN whose other-width reading is ordered; the four
two-source FP8 converts at VL512 with mask bits above 31; and the four VMOVRS
widths under merge, zero and no mask. Still present on
upstream master as of 2026-09-14. No silicon with AVX10.2 was available to
adjudicate: the spec text is the authority for each item.

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
