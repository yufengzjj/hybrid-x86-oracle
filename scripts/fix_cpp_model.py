#!/usr/bin/env python3
"""Post-process the Sail-generated C++ model.

Usage: fix_cpp_model.py path/to/model.h path/to/model.cpp

Fixes 1-4 work around Sail 0.20.1 `--cpp` backend bugs on large models
(arm-v9.4-a); fixes 5-7 correct semantic errors in the ACL2->Sail translation
itself, so they apply to the C backend too.

1. Some method declarations are emitted TWICE in the class body — valid C, but
   a C++ error. -> drop exact duplicate declaration lines.

2. Some prelude methods are declared and called but never defined (the C
   backend inlines them; the C++ backend loses them): per-enum
   `zeq_anyzIE<E>z5zK` plus zUInt0/zediv_nat/zemod_nat/zappend_str/zputchar/
   zeq_anyzIrzK. -> emit model_missing.cpp with the obvious bodies.

3. Calls `sint`/`sub_vec_int` instead of `sail_signed`/`sub_bits_int` — handled
   by aliases in vendor/sail-runtime/sail.h, not here.

4. CRITICAL use-after-free: the ~250k GMP scratch temporaries (zghz3*) are
   file-scope globals, but every instance's model_init/model_fini runs the
   startup_z*/finish_z* calls that create/kill them — so freeing one instance
   pulls the scratch out from under the others. -> guard the startup block to
   the first live instance and finish to the last (refcounted), leaving the
   per-instance parts untouched.

5. WRONG SEMANTICS: `ror_spec_N` computes CF from the result's LSB when the
   rotate count is > 1 — that is ROL's rule. Upstream ACL2 x86isa uses
   `(logbit size-1 result)`, the MSB, in *both* the `1` and `otherwise`
   branches; the Sail translation kept MSB only for count 1. Confirmed against
   Bochs and real hardware, which agree with each other and with the SDM.
   Affects all four widths, count > 1 only. -> rewrite the shift amount from 0
   to size-1 in each ror_spec_N. See docs/backend-differences.md §4b.

6. WRONG SEMANTICS: `x86_cmps` and `x86_cmpxchg` pass the two compared values
   to `gpr_arith_logic_spec` the wrong way round, so the flags describe the
   negated difference. ACL2 uses `(src1 src2)` = `([rSI] [rDI])` for CMPS and
   `(rAX reg/mem)` for CMPXCHG. -> swap the two operands at the single call
   site in each. See docs/backend-differences.md §4c.

7. WRONG SEMANTICS: REP-prefixed string ops never terminate. Three separate
   translation errors, all in `x86_movs` / `x86_cmps` / `x86_stos`:
     a. `x86_stos` reads the *segment* prefix field where ACL2 reads the rep
        field, so `rep stos` never repeats at all.
     b. The rCX == 0 test ACL2 performs before doing anything is missing, so
        entering with rCX == 0 executes one iteration and wraps the counter to
        all-ones instead of falling through.
     c. The 0xF3 arm advances RIP exactly when it should not: it keeps
        repeating once the counter hits zero, and consults ZF even for MOVS and
        STOS, where 0xF3 is plain REP and ZF is not part of the termination
        condition.
   -> read the rep field; insert the entry test; recompute the advance
   condition from the counter alone (MOVS/STOS) or from the counter and ZF
   (CMPS). See docs/backend-differences.md §4.
"""
import os
import re
import sys

SPECIALS = {  # name -> (decl regex, body template)
    "zUInt0": (
        r"void zUInt0\(sail_int \*rop, lbits\);",
        "void Model::zUInt0(sail_int *rop, lbits op)\n"
        "{\n  sail_unsigned(rop, op);\n}\n",
    ),
    "zediv_nat": (
        r"void zediv_nat\(sail_int \*rop, sail_int, sail_int\);",
        "void Model::zediv_nat(sail_int *rop, sail_int op1, sail_int op2)\n"
        "{\n  ediv_int(rop, op1, op2);\n}\n",
    ),
    "zemod_nat": (
        r"void zemod_nat\(sail_int \*rop, sail_int, sail_int\);",
        "void Model::zemod_nat(sail_int *rop, sail_int op1, sail_int op2)\n"
        "{\n  emod_int(rop, op1, op2);\n}\n",
    ),
    "zappend_str": (
        r"void zappend_str\(sail_string \*rop, const_sail_string, const_sail_string\);",
        "void Model::zappend_str(sail_string *rop, const_sail_string op1, const_sail_string op2)\n"
        "{\n  concat_str(rop, op1, op2);\n}\n",
    ),
    "zputchar": (
        r"unit zputchar\(sail_int\);",
        "unit Model::zputchar(sail_int op)\n"
        "{\n  return sail_putchar(op);\n}\n",
    ),
    "zeq_anyzIrzK": (
        r"bool zeq_anyzIrzK\(real, real\);",
        "bool Model::zeq_anyzIrzK(real op1, real op2)\n"
        "{\n  return EQUAL(real)(op1, op2);\n}\n",
    ),
}

ENUM_EQ = re.compile(r"^\s+bool (zeq_anyzIE[A-Za-z_0-9]+z5zK)\(enum (z[A-Za-z_0-9]+), enum \2\);\s*$")


def dedup_header(path: str) -> list[str]:
    seen = set()
    out = []
    dropped = 0
    in_class = False
    for line in open(path):
        if line.startswith("class "):
            in_class = True
        elif line.startswith("};"):
            in_class = False
            seen.clear()
        if in_class and line.rstrip().endswith(");"):
            if line in seen:
                dropped += 1
                continue
            seen.add(line)
        out.append(line)
    with open(path, "w") as f:
        f.writelines(out)
    print(f"{path}: dropped {dropped} duplicate declaration(s)")
    return out


def gen_missing(header_lines: list[str], header_path: str, cpp_path: str) -> None:
    # Skip any name model.cpp already defines.
    defined = set()
    defn = re.compile(r"^[a-z][a-z_0-9 ]*\**\s*Model::(z[A-Za-z_0-9]+)\(")
    for line in open(cpp_path):
        m = defn.match(line)
        if m:
            defined.add(m.group(1))

    bodies = []
    declared_specials = set()
    for line in header_lines:
        m = ENUM_EQ.match(line)
        if m and m.group(1) not in defined:
            name, ty = m.group(1), m.group(2)
            bodies.append(
                f"bool Model::{name}(enum {ty} op1, enum {ty} op2)\n"
                f"{{\n  return op1 == op2;\n}}\n"
            )
            defined.add(name)  # tolerate dup declarations
            continue
        for name, (decl_re, body) in SPECIALS.items():
            if name not in declared_specials and re.search(decl_re, line):
                declared_specials.add(name)
                if name not in defined:
                    bodies.append(body)

    out_path = os.path.join(os.path.dirname(header_path), "model_missing.cpp")
    with open(out_path, "w") as f:
        f.write(
            "// Generated by scripts/fix_cpp_model.py — definitions for methods the\n"
            "// Sail 0.20.1 --cpp backend declares (and calls) but never emits.\n"
            '#include "sail.h"\n#include "sail_config.h"\n#include "rts.h"\n'
            '#include "model.h"\n\nnamespace model {\n\n'
        )
        f.write("\n".join(bodies))
        f.write("\n} // namespace model\n")
    print(f"{out_path}: generated {len(bodies)} missing definition(s)")


STARTUP = re.compile(r"^\s+startup_z[A-Za-z_0-9]+\(\);\s*$")
FINISH = re.compile(r"^\s+finish_z[A-Za-z_0-9]+\(\);\s*$")
GUARD_DECL = "static int sail_scratch_refs = 0; /* fix_cpp_model.py */\n"


def refcount_scratch(cpp_path: str) -> None:
    """Guard the global-scratch startup/finish runs to first/last instance."""
    tmp_path = cpp_path + ".tmp"
    in_fn = None  # state machine: None | "init" | "fini"
    runs_wrapped = 0
    already = False
    with open(cpp_path) as src, open(tmp_path, "w") as dst:
        pending = []

        def flush(cond: str) -> None:
            nonlocal runs_wrapped
            if pending:
                dst.write(f"  if ({cond}) {{ /* fix_cpp_model.py */\n")
                dst.writelines(pending)
                dst.write("  }\n")
                pending.clear()
                runs_wrapped += 1

        for line in src:
            if "fix_cpp_model.py" in line:
                already = True
            if in_fn is None:
                if line.startswith("void Model::model_init(void)"):
                    in_fn = "init"
                    dst.write(GUARD_DECL + line + "{\n")
                    dst.write("  const bool sail_first = (sail_scratch_refs++ == 0);\n")
                    dst.write("  (void)sail_first;\n")
                    continue
                if line.startswith("void Model::model_fini(void)"):
                    in_fn = "fini"
                    dst.write(line + "{\n")
                    dst.write("  const bool sail_last = (--sail_scratch_refs == 0);\n")
                    dst.write("  (void)sail_last;\n")
                    continue
                dst.write(line)
                continue
            # inside model_init / model_fini
            if line.startswith("{"):
                continue  # opening brace already emitted
            pat = STARTUP if in_fn == "init" else FINISH
            cond = "sail_first" if in_fn == "init" else "sail_last"
            if pat.match(line):
                pending.append(line)
                continue
            flush(cond)
            dst.write(line)
            if line.startswith("}"):
                in_fn = None
    if already:
        os.remove(tmp_path)
        print(f"{cpp_path}: scratch refcount already applied, skipped")
        return
    os.replace(tmp_path, cpp_path)
    print(f"{cpp_path}: wrapped {runs_wrapped} startup/finish run(s) in refcount guards")


ROR_WIDTHS = (8, 16, 32, 64)
# The `otherwise` branch of ror_spec_N, which selects bit 0 of the result. The
# count==1 branch uses zlogbit_bits instead, so this pattern occurs exactly once
# per function — asserted below rather than assumed.
ROR_CF_LSB = "(safe_rshift(UINT64_MAX, 64 - 1) & (zresult >> INT64_C(0)))"


def fix_ror_cf(cpp_path: str) -> None:
    """ror_spec_N with count > 1 must take CF from the MSB, not the LSB."""
    src = open(cpp_path).read()
    patched = 0
    for size in ROR_WIDTHS:
        fn = f"Model::zror_spec_{size}(uint64_t"
        start = src.find(fn)
        if start < 0:
            raise SystemExit(f"{cpp_path}: no {fn} — model layout changed, fix 5 needs review")
        end = src.index("\n}\n", start)
        body = src[start:end]
        want = ROR_CF_LSB.replace("INT64_C(0)", f"INT64_C({size - 1})")
        if body.count(want) == 1:
            continue  # already applied
        n = body.count(ROR_CF_LSB)
        if n != 1:
            raise SystemExit(
                f"{cpp_path}: expected 1 CF-from-LSB site in ror_spec_{size}, found {n} "
                "— model layout changed, fix 5 needs review"
            )
        src = src[:start] + body.replace(ROR_CF_LSB, want) + src[end:]
        patched += 1
    if not patched:
        print(f"{cpp_path}: ror_spec CF fix already applied, skipped")
        return
    with open(cpp_path, "w") as f:
        f.write(src)
    print(f"{cpp_path}: fixed CF for count > 1 in {patched} ror_spec_N function(s)")


def function_body(src: str, name: str, fix: str) -> tuple[int, int]:
    """Byte range of `name`'s generated definition, `[start, end)`."""
    start = src.find(f"unit Model::{name}(")
    if start < 0:
        raise SystemExit(f"no {name} in the model — layout changed, {fix} needs review")
    return start, src.index("\n}\n", start)


# ------------------------------------------------------------------- fix 6

MARK_OPERANDS = "/* oracle-fix-6: operands swapped */ "
# gpr_arith_logic_spec(operand_size, operation, dst, src, input_rflags). The two
# instructions below each call it exactly once, which is asserted rather than
# assumed.
ARITH_CALL = re.compile(
    r"zgpr_arith_logic_spec\((?P<size>z\w+), INT64_C\((?P<op>\d+)\), "
    r"(?P<dst>z\w+), (?P<src>z\w+), (?P<fl>z\w+)\)"
)


def fix_compare_operands(cpp_path: str) -> None:
    """CMPS compares [rSI] with [rDI], CMPXCHG compares rAX with reg/mem."""
    src = open(cpp_path).read()
    if MARK_OPERANDS in src:
        print(f"{cpp_path}: compare-operand fix already applied, skipped")
        return
    patched = []
    for name in ("zx86_cmps", "zx86_cmpxchg"):
        start, end = function_body(src, name, "fix 6")
        body = src[start:end]
        hits = list(ARITH_CALL.finditer(body))
        if len(hits) != 1:
            raise SystemExit(
                f"{cpp_path}: expected 1 gpr_arith_logic_spec call in {name}, found "
                f"{len(hits)} — model layout changed, fix 6 needs review"
            )
        m = hits[0]
        swapped = (
            f"{MARK_OPERANDS}zgpr_arith_logic_spec({m['size']}, INT64_C({m['op']}), "
            f"{m['src']}, {m['dst']}, {m['fl']})"
        )
        body = body[: m.start()] + swapped + body[m.end() :]
        src = src[:start] + body + src[end:]
        patched.append(name)
    with open(cpp_path, "w") as f:
        f.write(src)
    print(f"{cpp_path}: swapped compare operands in {', '.join(patched)}")


# ------------------------------------------------------------------- fix 7

MARK_REP = "/* oracle-fix-7"

# `let group_1_prefix : bits(8) = prefixes[seg]` in x86_stos, where ACL2 has
# `(prefixes->rep prefixes)` — the field the 0xF3/0xF2 dispatch below reads.
STOS_PREFIX_FIELD = "  zgroup_1_prefix = z_get_prefixes_seg(zprefixes);\n"
STOS_PREFIX_FIXED = (
    "  zgroup_1_prefix = z_get_prefixes_rep(zprefixes); " + MARK_REP + "a: rep, not seg */\n"
)

# `let counter_addr_size = select_address_size(proc_mode, Some(prefixes))`, the
# first point at which the counter's width is known. Only `ctx` and `badlength?`
# are live here, so an early return has just those two to release.
ADDR_SIZE = re.compile(
    r"  int64_t zcounter_addr_sizze;\n"
    r"  \{\n"
    r"    struct zoptionzIRprefixeszK (?P<t>z\w+);\n"
    r"    CREATE\(zoptionzIRprefixeszK\)\(&(?P=t)\);\n"
    r"    zSomezIRprefixeszK\(&(?P=t), zprefixes\);\n"
    r"    zcounter_addr_sizze = zselect_address_sizze\(zproc_mode, (?P=t)\);\n"
    r"    KILL\(zoptionzIRprefixeszK\)\(&(?P=t)\);\n"
    r"  \}\n"
)

# bits_of_int(counter_addr_size, 4) is just the low nibble: the size is 2, 4 or 8.
# Kept ASCII: this text ends up in the generated C++, which MSVC also compiles.
REP_ENTRY_TEST = f"""  {MARK_REP}b: a REP-prefixed string op whose counter is already zero performs
     no iteration at all - memory is not touched and rIP moves on. ACL2 tests
     this before reading either operand; without it the counter wraps to
     all-ones below and the loop never ends. */
  if ((zgroup_1_prefix == UINT64_C(0xF3) || zgroup_1_prefix == UINT64_C(0xF2))
      && zrgfi_sizze((uint64_t)zcounter_addr_sizze & UINT64_C(0xF), UINT64_C(0x1), zrex_byte)
             == UINT64_C(0)) {{
    zwrite_iptr(zproc_mode, ztemp_rip);
    KILL(zoptionzIizK)(&zbadlengthzL);
    KILL(sail_string)(&zctx);
    return UNIT;
  }}
"""

# The `counter == 0 | rflags[zf] == 0bK` guard the 0xF3 and 0xF2 arms of the
# rep dispatch share, down to the assignment that feeds the `if`. How deeply
# the dispatch is nested differs per instruction, so the base indent is
# captured and the rest written relative to it.
REP_COND = re.compile(
    r"(?P<ind>[ ]+)bool (?P<res>z\w+);\n"
    r"(?P=ind)\{\n"
    r"(?P=ind)  bool (?P<zero>z\w+);\n"
    r"(?P=ind)  (?P=zero) = \((?P<ctr>z\w+) == UINT64_C\(0x0000000000000000\)\);\n"
    r"(?P=ind)  bool (?P<stop>z\w+);\n"
    r"(?P=ind)  if \((?P=zero)\) \{  (?P=stop) = true;  \} else \{\n"
    r"(?P=ind)    uint64_t (?P<zf>z\w+);\n"
    r"(?P=ind)    \{\n"
    r"(?P=ind)      struct zrflagsbits (?P<sc>z\w+);\n"
    r"(?P=ind)      (?P=sc) = zrflags;\n"
    r"(?P=ind)      (?P=zf) = z_get_rflagsbits_zzf\((?P=sc)\);\n"
    r"(?P=ind)    \}\n"
    r"(?P=ind)    (?P=stop) = \((?P=zf) == UINT64_C\(0b[01]\)\);\n"
    r"(?P=ind)  \}\n"
    r"(?P=ind)  (?P=res) = (?P=stop);\n"
    r"(?P=ind)\}\n"
    r"(?P=ind)if \((?P=res)\) \{\n"
)


def brace_end(src: str, open_idx: int) -> int:
    """Index just past the `}` matching the `{` at `open_idx`."""
    depth = 0
    for i in range(open_idx, len(src)):
        if src[i] == "{":
            depth += 1
        elif src[i] == "}":
            depth -= 1
            if depth == 0:
                return i + 1
    raise SystemExit("unbalanced braces — model layout changed, fix 7 needs review")


def fix_rep_termination(cpp_path: str) -> None:
    """REP string ops must stop on rCX == 0, and only CMPS may look at ZF."""
    src = open(cpp_path).read()
    if MARK_REP in src:
        print(f"{cpp_path}: rep termination fix already applied, skipped")
        return
    # True where 0xF3 means REPE and the loop also stops on ZF; False where it
    # is plain REP and the counter alone decides.
    for name, repe in (("zx86_movs", False), ("zx86_cmps", True), ("zx86_stos", False)):
        start, end = function_body(src, name, "fix 7")
        body = src[start:end]

        if name == "zx86_stos":
            if body.count(STOS_PREFIX_FIELD) != 1:
                raise SystemExit(
                    f"{cpp_path}: no seg-prefix read in {name} — either upstream fixed it "
                    "or the layout changed; fix 7a needs review"
                )
            body = body.replace(STOS_PREFIX_FIELD, STOS_PREFIX_FIXED)

        anchor = ADDR_SIZE.search(body)
        if anchor is None:
            raise SystemExit(
                f"{cpp_path}: no counter_addr_size block in {name} "
                "— model layout changed, fix 7b needs review"
            )
        # The early return releases what the normal exit releases, minus the
        # allocations made after this point. Confirm that set is what it was.
        held = {"KILL(zoptionzIizK)(&zbadlengthzL);", "KILL(sail_string)(&zctx);"}
        if not held <= set(re.findall(r"KILL\(\w+\)\(&\w+\);", body[: anchor.start()])):
            raise SystemExit(
                f"{cpp_path}: {name} does not hold ctx and badlength? at the entry test "
                "— model layout changed, fix 7b needs review"
            )
        body = body[: anchor.end()] + REP_ENTRY_TEST + body[anchor.end() :]

        # Both arms of the dispatch write the decremented counter; exactly one
        # of them also advances rIP, and which one is not the same in the two
        # arms, so read it off rather than assume.
        arms = list(REP_COND.finditer(body))
        if len(arms) != 2:
            raise SystemExit(
                f"{cpp_path}: expected 2 rep-termination guards in {name}, found {len(arms)} "
                "— model layout changed, fix 7c needs review"
            )
        for m in reversed(arms):  # back to front: earlier offsets stay valid
            then_open = m.end() - 2
            then_end = brace_end(body, then_open)
            if not body.startswith(" else {", then_end):
                raise SystemExit(
                    f"{cpp_path}: rep guard in {name} is not an if/else "
                    "— model layout changed, fix 7c needs review"
                )
            else_end = brace_end(body, then_end + len(" else "))
            advances = [
                "zwrite_iptr" in body[then_open:then_end],
                "zwrite_iptr" in body[then_end:else_end],
            ]
            if advances.count(True) != 1:
                raise SystemExit(
                    f"{cpp_path}: rep guard in {name} does not advance rIP on exactly one arm "
                    "— model layout changed, fix 7c needs review"
                )
            # `stop` already reads `counter == 0 | zf == <the value that ends
            # this arm's loop>`, which is exactly CMPS's REPE/REPNE condition.
            # MOVS and STOS take 0xF3 and 0xF2 alike as plain REP, so for them
            # only the counter matters.
            advance = m["stop"] if repe else m["zero"]
            keep = "" if advances[0] else "!"
            at = m["ind"] + "  "
            # Dropping the ZF term leaves `stop` set but unread, which some
            # compilers warn about.
            unused = "" if repe else f"{at}(void){m['stop']};\n"
            guard = m.group(0).replace(
                f"{at}{m['res']} = {m['stop']};\n",
                f"{at}{MARK_REP}c */ {m['res']} = {keep}({advance});\n" + unused,
            )
            body = body[: m.start()] + guard + body[m.end() :]
        src = src[:start] + body + src[end:]
    with open(cpp_path, "w") as f:
        f.write(src)
    print(f"{cpp_path}: corrected rep termination in x86_movs, x86_cmps, x86_stos")


if __name__ == "__main__":
    header, cpp = sys.argv[1], sys.argv[2]
    kept = dedup_header(header)
    gen_missing(kept, header, cpp)
    refcount_scratch(cpp)
    fix_ror_cf(cpp)
    fix_compare_operands(cpp)
    fix_rep_termination(cpp)
