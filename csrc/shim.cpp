/* =============================================================================
 * shim.cpp — extern "C" ABI between Rust and the Sail C++ model class
 * (`model::Model`, from `sail --cpp`; architectural state lives in members, so
 * each oracle_new() is independent).
 *
 * MEMORY ISOLATION: the runtime keeps memory in process globals (sail_memory /
 * sail_tags block lists, rts.c). Each instance owns its own pair; the MemCtx
 * RAII guard swaps them into the globals around every call. Correct only
 * single-threaded — the Rust wrapper serializes all calls under one mutex.
 *
 * NAME MANGLING: Sail prefixes top-level names with `z` and escapes a literal
 * 'z' to "zz", so e.g. set_zmm_chunk -> zset_zzmm_chunk.
 * ===========================================================================*/

#include "sail.h"
#include "model.h"

#include <stdint.h>
#include <string.h>

/* Memory-model globals + accessors from rts.c (not exposed in rts.h).
 * read/write_mem are byte-addressed, little-endian (byte at `address` = LSB)
 * and are exactly what the model's loads/stores decompose into. */
extern "C" {
struct block;
struct tag_block;
extern struct block *sail_memory;
extern struct tag_block *sail_tags;
void kill_mem(void);
uint64_t read_mem(uint64_t address);
void write_mem(uint64_t address, uint64_t byte);
bool sail_addr_mapped(uint64_t address);
}

namespace {

struct OracleInstance {
    model::Model m;
    struct block *memory = nullptr;
    struct tag_block *tags = nullptr;
};

OracleInstance *cast(void *h) { return static_cast<OracleInstance *>(h); }

/* Swap the instance's memory lists into the runtime globals for the call, save
 * the (possibly grown) heads back on exit. Single-threaded by contract. */
class MemCtx {
    OracleInstance *inst;

public:
    explicit MemCtx(OracleInstance *i) : inst(i) {
        sail_memory = i->memory;
        sail_tags = i->tags;
    }
    ~MemCtx() {
        inst->memory = sail_memory;
        inst->tags = sail_tags;
    }
};

} // namespace

extern "C" {

void *oracle_new(void) {
    OracleInstance *o = new OracleInstance();
    MemCtx ctx(o);
    o->m.model_init();           // setup_rts() refcounted; zinit_harness resets
    o->m.zinit_harness(UNIT);
    return o;
}

void oracle_free(void *h) {
    OracleInstance *o = cast(h);
    {
        MemCtx ctx(o);
        kill_mem();        // frees THIS instance's memory
        o->m.model_fini(); // refcounted cleanup_rts inside
    }
    delete o;
}

/* GPR index = x86 ModRM/ACL2 encoding: 0=RAX 1=RCX 2=RDX 3=RBX 4=RSP 5=RBP
 * 6=RSI 7=RDI 8..15=R8..R15. */
void oracle_set_gpr(void *h, uint32_t n, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_gpr((fbits)(n & 0xff), (fbits)value);
}

uint64_t oracle_get_gpr(void *h, uint32_t n) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_gpr((fbits)(n & 0xff));
}

void oracle_set_rip(void *h, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_rip((fbits)value);
}

uint64_t oracle_get_rip(void *h) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_rip(UNIT);
}

void oracle_set_rflags(void *h, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_rflags((fbits)value);
}

uint64_t oracle_get_rflags(void *h) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_rflags(UNIT);
}

/* 0 = retired normally, 1 = Emsg fault/model error, 2 = SYSCALL stub. */
uint64_t oracle_step(void *h) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zstep_x86(UNIT);
}

/* set_zmm_chunk => zset_zzmm_chunk (zz mangling). */
void oracle_set_zmm(void *h, uint32_t n, uint32_t chunk, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_zzmm_chunk((fbits)(n & 0xff), (fbits)(chunk & 0xff), (fbits)value);
}

uint64_t oracle_get_zmm(void *h, uint32_t n, uint32_t chunk) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_zzmm_chunk((fbits)(n & 0xff), (fbits)(chunk & 0xff));
}

void oracle_set_msr(void *h, uint32_t idx, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_msr((fbits)(idx & 0xff), (fbits)value);
}

uint64_t oracle_get_msr(void *h, uint32_t idx) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_msr((fbits)(idx & 0xff));
}

void oracle_set_ctr(void *h, uint32_t idx, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_ctr((fbits)(idx & 0xff), (fbits)value);
}

uint64_t oracle_get_ctr(void *h, uint32_t idx) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_ctr((fbits)(idx & 0xff));
}

void oracle_set_seg_visible(void *h, uint32_t n, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_seg_visible((fbits)(n & 0xff), (fbits)value);
}

uint64_t oracle_get_seg_visible(void *h, uint32_t n) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_seg_visible((fbits)(n & 0xff));
}

void oracle_set_seg_base(void *h, uint32_t n, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_seg_base((fbits)(n & 0xff), (fbits)value);
}

uint64_t oracle_get_seg_base(void *h, uint32_t n) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_seg_base((fbits)(n & 0xff));
}

void oracle_set_seg_limit(void *h, uint32_t n, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_seg_limit((fbits)(n & 0xff), (fbits)value);
}

uint64_t oracle_get_seg_limit(void *h, uint32_t n) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_seg_limit((fbits)(n & 0xff));
}

void oracle_set_seg_attr(void *h, uint32_t n, uint64_t value) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    o->m.zset_seg_attr((fbits)(n & 0xff), (fbits)value);
}

uint64_t oracle_get_seg_attr(void *h, uint32_t n) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint64_t)o->m.zget_seg_attr((fbits)(n & 0xff));
}

/* Copies the last exception message (empty string if the last step retired
 * normally) into buf (NUL-terminated, truncated to cap-1) and returns the
 * full untruncated length. */
size_t oracle_get_fault_msg(void *h, char *buf, size_t cap) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    sail_string s;
    CREATE(sail_string)(&s);
    o->m.zget_fault_msg(&s, UNIT);
    size_t len = strlen(s);
    if (buf != nullptr && cap > 0) {
        size_t n = len < cap - 1 ? len : cap - 1;
        memcpy(buf, s, n);
        buf[n] = '\0';
    }
    KILL(sail_string)(&s);
    return len;
}

uint8_t oracle_read_mem(void *h, uint64_t addr) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return (uint8_t)read_mem(addr);
}

void oracle_write_mem(void *h, uint64_t addr, uint8_t byte) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    write_mem(addr, (uint64_t)byte);
}

bool oracle_is_mapped(void *h, uint64_t addr) {
    OracleInstance *o = cast(h);
    MemCtx ctx(o);
    return sail_addr_mapped(addr);
}

} // extern "C"
