/* =============================================================================
 * harness.sail — C-callable wrappers for the x86 oracle, appended LAST to the
 * sail-x86-from-acl2 model (see README Step 2 for the full `sail --cpp`
 * command).
 *
 * Build gotchas (same recipe as sail-aarch64-oracle):
 *   - logging.sail is REPLACED by quiet_logging.sail (the stock one prints
 *     every memory access to stdout).
 *   - main.sail is kept: it provides `main` (required even with --c-no-main)
 *     and initialise_64_bit_mode(), both of which we use.
 *   - Sail's C++ backend dead-code-eliminates unreferenced code, so every
 *     wrapper needs `--c-preserve <unmangled-name>` on the command line.
 *   - The ABI uses only fixed-width bitvectors (<= 64 b) so the generated C
 *     is plain uint64_t/unit with no GMP sail_int (exception: get_fault_msg
 *     returns a sail_string).
 *   - Name mangling: `z` prefix, and a literal `z` inside escapes to `zz`
 *     (set_zmm_chunk -> zset_zzmm_chunk) — shim.cpp calls the mangled names.
 * ===========================================================================*/

/* Message of the last Emsg/Syscall exception caught by step_x86 (empty if the
 * last step retired normally). ACL2-model faults look like "Fault: (:UD)",
 * model shortcomings like "Model error: ...". */
register last_exception : string

/* Reset to a deterministic application-level 64-bit-mode state, mirroring the
 * emulator invocation used by the upstream validation flow:
 *   x86_emulator -C rflags=2 -C cr4=512 -C set64bit=1 -C app_view=1
 * All architectural registers are zeroed explicitly (the generated
 * initialize_registers() zero-fills too, but we do not rely on it), rflags
 * bit 1 (the always-one bit) is set, CR4.OSFXSR enables SSE, and
 * EFER.LMA + CS.L put the model in 64-bit mode. app_view = flat linear
 * memory, no paging. */
function init_harness() -> unit = {
    foreach (i from 0 to 15) {
        write_rgfi(i, 0x0000000000000000)
    };
    rip = 0x0000000000000000;
    rflags = Mk_rflagsbits(0x00000002);
    foreach (i from 0 to 6) {
        msrs[i] = 0x0000000000000000
    };
    foreach (i from 0 to 5) {
        seg_visibles[i] = 0x0000;
        seg_hidden_attrs[i] = 0x0000;
        seg_hidden_bases[i] = 0x0000000000000000;
        seg_hidden_limits[i] = 0x00000000
    };
    foreach (i from 0 to 1) {
        ssr_visibles[i] = 0x0000;
        ssr_hidden_attrs[i] = 0x0000;
        ssr_hidden_bases[i] = 0x0000000000000000;
        ssr_hidden_limits[i] = 0x00000000;
        strs[i] = sail_zeros(80)
    };
    foreach (i from 0 to 31) {
        zmms[i] = sail_zeros(512)
    };
    foreach (i from 0 to 16) {
        ctrs[i] = 0x0000000000000000
    };
    app_view = true;
    marking_view = false;
    log_register_writes = false;
    ms_reg = false;
    fault_reg = false;
    haltAddrReg = sail_zeros(48);
    last_exception = "";
    /* CR0.PE|MP|ET|NE|PG roughly as a 64-bit OS would leave them is not
     * modelled here; the app-view model only consults CR4.OSFXSR (SSE) and
     * the EFER.LMA/CS.L pair set below. */
    ctrs[4] = 0x0000000000000200;   /* CR4.OSFXSR */
    initialise_64_bit_mode();
    ()
}

/* GPRs use the x86 ModRM/ACL2 encoding:
 * 0=RAX 1=RCX 2=RDX 3=RBX 4=RSP 5=RBP 6=RSI 7=RDI 8..15=R8..R15. */
function set_gpr(n: bits(8), value: bits(64)) -> unit = {
    write_rgfi(unsigned(n[3 .. 0]), value)
}

function get_gpr(n: bits(8)) -> bits(64) = {
    rgfi(unsigned(n[3 .. 0]))
}

/* RIP is architecturally 48 bits in this model (canonical form), stored
 * sign-extended to 64. The setter truncates, the getter sign-extends. */
function set_rip(value: bits(64)) -> unit = {
    write_rip(truncate(value, 48))
}

function get_rip() -> bits(64) = {
    sail_sign_extend(read_rip(), 64)
}

/* The ACL2 model keeps RFLAGS as 32 bits (the upper 32 are reserved-zero
 * architecturally). Bit 1 should always be written as 1. */
function set_rflags(value: bits(64)) -> unit = {
    rflags = Mk_rflagsbits(value[31 .. 0])
}

function get_rflags() -> bits(64) = {
    sail_zero_extend(rflags.bits, 64)
}

/* ZMM0-ZMM31, 512 bits each, as 8 little-endian 64-bit chunks
 * (chunk 0 = bits 63:0 = the XMM low half). */
function set_zmm_chunk(n: bits(8), idx: bits(8), value: bits(64)) -> unit = {
    let r = unsigned(n[4 .. 0]);
    let i = unsigned(idx[2 .. 0]);
    var v : bits(512) = zmms[r];
    v[64 * i + 63 .. 64 * i] = value;
    zmms[r] = v
}

function get_zmm_chunk(n: bits(8), idx: bits(8)) -> bits(64) = {
    let r = unsigned(n[4 .. 0]);
    let i = unsigned(idx[2 .. 0]);
    let v : bits(512) = zmms[r];
    v[64 * i + 63 .. 64 * i]
}

/* MSRs by model index (x86isa order, NOT the IA32 MSR address). Verified
 * against the model's own uses (modes.sail, segmentation.sail, syscall.sail):
 *   0=IA32_EFER 1=IA32_FS_BASE 2=IA32_GS_BASE 3=IA32_KERNEL_GS_BASE
 *   4=IA32_STAR 5=IA32_LSTAR 6=IA32_FMASK
 * src/sail.rs maps architectural MSR numbers onto these. */
function set_msr(idx: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(idx[2 .. 0]);
    if i < 7 then msrs[i] = value else ()
}

function get_msr(idx: bits(8)) -> bits(64) = {
    let i = unsigned(idx[2 .. 0]);
    if i < 7 then msrs[i] else 0x0000000000000000
}

/* Control registers CR0-CR8... by model index 0..16. */
function set_ctr(idx: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(idx[4 .. 0]);
    if i < 17 then ctrs[i] = value else ()
}

function get_ctr(idx: bits(8)) -> bits(64) = {
    let i = unsigned(idx[4 .. 0]);
    if i < 17 then ctrs[i] else 0x0000000000000000
}

/* Segment registers by x86isa index: 0=ES 1=CS 2=SS 3=DS 4=FS 5=GS.
 * visible = the 16-bit selector; base/limit/attrs = the hidden
 * (descriptor-cache) parts consulted by the model in 32-bit modes and for
 * FS/GS bases in 64-bit mode. */
function set_seg_visible(n: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then seg_visibles[i] = value[15 .. 0] else ()
}

function get_seg_visible(n: bits(8)) -> bits(64) = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then sail_zero_extend(seg_visibles[i], 64) else 0x0000000000000000
}

function set_seg_base(n: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then seg_hidden_bases[i] = value else ()
}

function get_seg_base(n: bits(8)) -> bits(64) = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then seg_hidden_bases[i] else 0x0000000000000000
}

function set_seg_limit(n: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then seg_hidden_limits[i] = value[31 .. 0] else ()
}

function get_seg_limit(n: bits(8)) -> bits(64) = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then sail_zero_extend(seg_hidden_limits[i], 64) else 0x0000000000000000
}

function set_seg_attr(n: bits(8), value: bits(64)) -> unit = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then seg_hidden_attrs[i] = value[15 .. 0] else ()
}

function get_seg_attr(n: bits(8)) -> bits(64) = {
    let i = unsigned(n[2 .. 0]);
    if i < 6 then sail_zero_extend(seg_hidden_attrs[i], 64) else 0x0000000000000000
}

function get_fault_msg() -> string = last_exception

/* One x86 instruction: fetch (from model memory at RIP) + decode + execute.
 * Unlike the A64 oracle there is no opcode injection — x86 instructions are
 * variable-length, so the bytes must be written to memory at RIP first.
 *
 * Returns: 0 = instruction retired normally;
 *          1 = model threw Emsg (fault or model error — RIP and the rest of
 *              the state are whatever the model had committed by then; the
 *              message is readable via get_fault_msg);
 *          2 = SYSCALL reached its unimplemented-in-app-view stub. */
function step_x86() -> bits(64) = {
    last_exception = "";
    try {
        x86_fetch_decode_execute();
        0x0000000000000000
    } catch {
        Emsg(msg) => {
            last_exception = msg;
            0x0000000000000001
        },
        Syscall() => {
            last_exception = "Syscall";
            0x0000000000000002
        }
    }
}
