#!/usr/bin/env python3
"""Post-process the Sail-generated C++ model.

Usage: fix_cpp_model.py path/to/model.h path/to/model.cpp

Fixes 1-4 work around Sail 0.20.1 `--cpp` backend bugs on large models
(arm-v9.4-a); fix 5 corrects a semantic error in the ACL2->Sail translation
itself, so it applies to the C backend too.

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


if __name__ == "__main__":
    header, cpp = sys.argv[1], sys.argv[2]
    kept = dedup_header(header)
    gen_missing(kept, header, cpp)
    refcount_scratch(cpp)
    fix_ror_cf(cpp)
