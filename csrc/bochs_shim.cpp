/* =============================================================================
 * bochs_shim.cpp — extern "C" ABI between Rust and the Bochs CPU core.
 *
 * The Bochs CPU is a library of C++ objects that normally sit inside the full
 * Bochs emulator. Embedding just the CPU means supplying, ourselves, the parts
 * of that emulator the CPU refers to:
 *
 *   1. the CPU and memory singletons (`bx_cpu`, `bx_mem`) — Bochs is built
 *      with --disable-smp, so `BX_CPU(x)` is `&bx_cpu`: exactly ONE CPU per
 *      process. That is why BochsOracle is a process singleton;
 *   2. a memory implementation. Bochs' own (memory/libmemory.a) wants one
 *      eagerly-allocated flat RAM array, which cannot cover a sparse 64-bit
 *      address space, so we implement BX_MEM_C's access methods over a lazily
 *      allocated page map instead;
 *   3. the instrumentation callbacks (`bx_instr_*`). Bochs is configured with
 *      --enable-instrumentation=instrument/stubs, so the CPU *calls* these; we
 *      do not link the stub bodies but provide our own — this is how a step
 *      stops after one instruction and how faults are observed;
 *   4. stubs for the device/GUI/plugin layer that Bochs' own config.cc and
 *      siminterface.cc reference. Those two are linked from the Bochs tree
 *      (rather than hand-rolled) so the CPUID/feature defaults are Bochs'
 *      real ones.
 *
 * FLAGS CORRECTNESS: Bochs evaluates the arithmetic flags lazily — the raw
 * `eflags` field is stale until `force_flags()` runs. Every read here goes
 * through `read_eflags()`, which forces materialization. Reading the field
 * directly (as some embeddings do) silently returns wrong CF/PF/AF/ZF/SF/OF.
 * ===========================================================================*/

#include "bochs.h"
#include "param_names.h"
#include "cpu/cpu.h"
#include "memory/memory-bochs.h"
#include "pc_system.h"
#include "iodev/iodev.h"
#include "iodev/virt_timer.h"
#include "iodev/hdimage/hdimage.h"
#include "gui/gui.h"
#include "plugin.h"

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <new>
#include <unordered_map>

/* ---------------------------------------------------------------------------
 * 1) The singletons Bochs expects to exist
 * ------------------------------------------------------------------------- */

/* STATIC INIT ORDER MATTERS. Within one translation unit globals are
 * constructed in declaration order, and BX_CPU_C's constructor builds a local
 * APIC that registers a timer with `bx_pc_system` and logs while doing it. So
 * every object the CPU constructor may touch is defined here, and `bx_cpu`
 * itself is defined LAST, at the bottom of this file. */
BOCHSAPI BX_MEM_C bx_mem;
BOCHSAPI bx_pc_system_c bx_pc_system;

/* ---------------------------------------------------------------------------
 * 2) Sparse guest memory
 *
 * A flat page map: guest physical page -> 4 KiB host page, allocated on first
 * touch and zero-filled (matching the "never written reads as zero" contract).
 * Bochs' CPU only ever accesses within a single page per call, which is what
 * lets getHostMemAddr hand out a raw pointer.
 * ------------------------------------------------------------------------- */

namespace {

constexpr uint64_t PG_SIZE = 0x1000;
constexpr uint64_t PG_MASK = ~(PG_SIZE - 1);
/* Bochs' physical addresses are 52-bit. */
constexpr uint64_t PHY_MASK = 0x000F'FFFF'FFFF'FFFFull;

struct GuestMemory {
    std::unordered_map<uint64_t, uint8_t *> pages;

    ~GuestMemory() { clear(); }

    void clear() {
        for (auto &kv : pages) {
            ::operator delete(kv.second, std::align_val_t(PG_SIZE));
        }
        pages.clear();
    }

    /* Existing page, or nullptr when `alloc` is false and it is absent. */
    uint8_t *page(uint64_t gpa, bool alloc) {
        const uint64_t key = (gpa & PHY_MASK) & PG_MASK;
        auto it = pages.find(key);
        if (it != pages.end()) {
            return it->second;
        }
        if (!alloc) {
            return nullptr;
        }
        uint8_t *p = static_cast<uint8_t *>(
            ::operator new(PG_SIZE, std::align_val_t(PG_SIZE)));
        std::memset(p, 0, PG_SIZE);
        pages.emplace(key, p);
        return p;
    }

    bool mapped(uint64_t gpa) { return page(gpa, false) != nullptr; }

    void read(uint64_t gpa, unsigned len, void *dst) {
        uint8_t *p = page(gpa, false);
        if (p == nullptr) {
            std::memset(dst, 0, len); // unwritten memory reads as zero
            return;
        }
        std::memcpy(dst, p + ((gpa & PHY_MASK) & (PG_SIZE - 1)), len);
    }

    void write(uint64_t gpa, unsigned len, const void *src) {
        uint8_t *p = page(gpa, true);
        std::memcpy(p + ((gpa & PHY_MASK) & (PG_SIZE - 1)), src, len);
    }
};

/* The single live instance's memory (one CPU per process, see the header
 * comment). Held as a pointer so oracle_free can tear it down deterministically
 * and a later oracle_new starts from a clean address space. */
GuestMemory *g_mem = nullptr;

GuestMemory &mem() {
    if (g_mem == nullptr) {
        g_mem = new GuestMemory();
    }
    return *g_mem;
}

/* ---------------------------------------------------------------------------
 * Step / fault bookkeeping, driven by the instrumentation callbacks
 * ------------------------------------------------------------------------- */

struct StepCtl {
    bool armed = false;      // a step is in progress
    bool retired = false;    // an instruction (or REP iteration) completed
    bool faulted = false;
    unsigned vector = 0;
    unsigned error_code = 0;
    bool halted = false;
};

StepCtl g_step;

/* Break out of cpu_loop: the CPU checks async_event, the driver loop checks
 * kill_bochs_request. */
void request_stop() {
    bx_cpu.async_event = 1;
    bx_pc_system.kill_bochs_request = 1;
}

} // namespace

/* ---------------------------------------------------------------------------
 * BX_MEM_C: the access methods the CPU calls. Everything else in the class is
 * unused by the CPU core, so trivial ctors/dtors suffice.
 * ------------------------------------------------------------------------- */

BX_MEM_C::BX_MEM_C() = default;
BX_MEM_C::~BX_MEM_C() = default;
BX_MEMORY_STUB_C::BX_MEMORY_STUB_C() = default;
BX_MEMORY_STUB_C::~BX_MEMORY_STUB_C() = default;

void BX_MEM_C::readPhysicalPage(BX_CPU_C *, bx_phy_address addr, unsigned len, void *data) {
    mem().read((uint64_t)addr, len, data);
}

void BX_MEM_C::writePhysicalPage(BX_CPU_C *, bx_phy_address addr, unsigned len, void *data) {
    mem().write((uint64_t)addr, len, data);
}

Bit8u *BX_MEM_C::getHostMemAddr(BX_CPU_C *, bx_phy_address addr, unsigned) {
    /* Always called with a page-aligned address; Bochs indexes the returned
     * pointer with the intra-page offset itself. */
    /* Allocate on demand even for reads: untouched memory must read as zeros,
     * and Bochs caches this pointer in its TLB for the whole page. */
    return mem().page((uint64_t)addr, true);
}

bool BX_MEM_C::dbg_fetch_mem(BX_CPU_C *, bx_phy_address addr, unsigned len, Bit8u *buf) {
    mem().read((uint64_t)addr, len, buf);
    return true;
}

bool BX_MEM_C::registerMemoryHandlers(void *, memory_handler_t, memory_handler_t,
                                      memory_direct_access_handler_t, bx_phy_address,
                                      bx_phy_address) {
    return false; // no MMIO devices in an oracle
}

Bit64u BX_MEMORY_STUB_C::get_memory_len() {
    /* The full 52-bit physical space is "present" — pages materialize lazily. */
    return PHY_MASK + 1;
}

/* ---------------------------------------------------------------------------
 * 3) Instrumentation callbacks
 *
 * after_execution fires once per completed instruction; repeat_iteration fires
 * once per REP iteration (with RIP parked on the instruction). Stopping on
 * both gives per-iteration stepping for string operations, matching the ACL2
 * model's granularity.
 * ------------------------------------------------------------------------- */

/* Bochs declares these with C++ linkage (instrument/stubs/instrument.h), so no
 * extern "C" here. */

void bx_instr_after_execution(unsigned, bxInstruction_c *) {
    if (g_step.armed) {
        g_step.retired = true;
        request_stop();
    }
}

void bx_instr_repeat_iteration(unsigned, bxInstruction_c *) {
    if (g_step.armed) {
        g_step.retired = true;
        request_stop();
    }
}

void bx_instr_exception(unsigned, unsigned vector, unsigned error_code) {
    if (g_step.armed && !g_step.faulted) {
        g_step.faulted = true;
        g_step.vector = vector;
        g_step.error_code = error_code;
        /* Stop before the CPU vectors through an IDT we deliberately do not
         * provide: the contract is that faults do not vector, leaving the
         * committed-so-far state observable. */
        request_stop();
    }
}

void bx_instr_hlt(unsigned) {
    g_step.halted = true;
    request_stop();
}

/* Hooks we do not use. Bochs calls these unconditionally, so every one must
 * exist as a symbol. */
void bx_instr_init_env(void) {}
void bx_instr_exit_env(void) {}
void bx_instr_initialize(unsigned) {}
void bx_instr_exit(unsigned) {}
void bx_instr_reset(unsigned, unsigned) {}
void bx_instr_mwait(unsigned, bx_phy_address, unsigned, Bit32u) {}
void bx_instr_debug_promt(void) {}
void bx_instr_debug_cmd(const char *) {}
void bx_instr_cnear_branch_taken(unsigned, bx_address, bx_address) {}
void bx_instr_cnear_branch_not_taken(unsigned, bx_address) {}
void bx_instr_ucnear_branch(unsigned, unsigned, bx_address, bx_address) {}
void bx_instr_far_branch(unsigned, unsigned, Bit16u, bx_address, Bit16u, bx_address) {}
void bx_instr_opcode(unsigned, bxInstruction_c *, const Bit8u *, unsigned, bool, bool) {}
void bx_instr_interrupt(unsigned, unsigned) {}
void bx_instr_hwinterrupt(unsigned, unsigned, Bit16u, bx_address) {}
void bx_instr_tlb_cntrl(unsigned, unsigned, bx_phy_address) {}
void bx_instr_cache_cntrl(unsigned, unsigned) {}
void bx_instr_prefetch_hint(unsigned, unsigned, unsigned, bx_address) {}
void bx_instr_clflush(unsigned, bx_address, bx_phy_address) {}
void bx_instr_cpuid(unsigned) {}
void bx_instr_before_execution(unsigned, bxInstruction_c *) {}
void bx_instr_inp(Bit16u, unsigned) {}
void bx_instr_inp2(Bit16u, unsigned, unsigned) {}
void bx_instr_outp(Bit16u, unsigned, unsigned) {}
void bx_instr_lin_access(unsigned, bx_address, bx_address, unsigned, unsigned, unsigned) {}
void bx_instr_phy_access(unsigned, bx_address, unsigned, unsigned, unsigned) {}
void bx_instr_wrmsr(unsigned, unsigned, Bit64u) {}
void bx_instr_vmexit(unsigned, Bit32u, Bit64u) {}

/* ---------------------------------------------------------------------------
 * 4) Emulator-layer stubs
 *
 * Bochs' config.cc / siminterface.cc are linked from the Bochs tree so the CPU
 * sees its real parameter defaults; they in turn reference the device, GUI and
 * plugin layers, which an oracle has none of.
 * ------------------------------------------------------------------------- */

BOCHSAPI bx_devices_c bx_devices;
BOCHSAPI bx_virt_timer_c bx_virt_timer;
BOCHSAPI bx_hdimage_ctl_c bx_hdimage_ctl;
BOCHSAPI bx_gui_c *bx_gui = nullptr;
/* Zero-initialized: every debug/trace flag off, which is what
 * main.cc's bx_init_bx_dbg() amounts to for a CPU-only embedding. */
BOCHSAPI bx_debug_t bx_dbg;
BOCHSAPI Bit32u apic_id_mask = 0;
BOCHSAPI bool bx_user_quit = false;
BOCHSAPI logfunctions *pluginlog = nullptr;
/* Bochs' main.cc sets this; the local APIC reads it to pick xAPIC vs legacy. */
bool simulate_xapic = true;

bx_devices_c::bx_devices_c() = default;
bx_devices_c::~bx_devices_c() = default;
void bx_devices_c::init(BX_MEM_C *) {}
void bx_devices_c::exit(void) {}
void bx_devices_c::reset(unsigned) {}
bool bx_devices_c::is_agp_present(void) { return false; }
Bit32u bx_devices_c::inp(Bit16u, unsigned) { return 0xFFFFFFFF; } // no devices: reads float high
void bx_devices_c::outp(Bit16u, unsigned, Bit32u) {}

bx_virt_timer_c::bx_virt_timer_c() = default;
void bx_virt_timer_c::set_realtime_delay(void) {}

/* Storage/GUI/plugin layer: config.cc walks these while building the parameter
 * tree, but an oracle has no devices, no window and no plugins. */
bx_hdimage_ctl_c::bx_hdimage_ctl_c() = default;
void bx_hdimage_ctl_c::init(void) {}
const char **bx_hdimage_ctl_c::get_mode_names(void) {
    static const char *none[] = {nullptr};
    return none;
}
int bx_hdimage_ctl_c::get_mode_id(const char *) { return -1; }

/* Defining this key virtual also emits bx_pci_device_c's vtable/typeinfo, which
 * the PCI-enabled APIC code references even though no device is ever present. */
Bit32u bx_pci_device_c::pci_read_handler(Bit8u, unsigned) { return 0; }

void bx_gui_c::cleanup(void) {}
void bx_gui_c::update_drive_status_buttons(void) {}
bool bx_gui_c::parse_user_shortcut(const char *) { return false; }

/* bochsrc handling: siminterface.cc's save/restore and option paths reference
 * these, but an oracle is configured programmatically and never reads or writes
 * a Bochs configuration file. */
char *bx_find_bochsrc(void) { return nullptr; }
int bx_read_configuration(const char *) { return 0; }
int bx_write_configuration(const char *, int) { return 0; }
void bx_reset_options(void) {}
int bx_parse_param_from_list(const char *, const char *, bx_list_c *) { return 0; }
int bx_parse_nic_params(const char *, const char *, bx_list_c *) { return 0; }
int bx_parse_usb_port_params(const char *, const char *, int, bx_list_c *) { return 0; }
int bx_split_option_list(const char *, const char *, char **, int) { return 0; }
int bx_write_param_list(FILE *, bx_list_c *, const char *, bool) { return 0; }

void bx_exit(int) {}
int bx_atexit(void) { return 0; }
int bx_begin_simulation(int, char **) { return 0; }
void bx_set_log_actions_by_device(bool) {}
void print_statistics_tree(bx_param_c *, int) {}

bool pluginDevicePresent(const char *) { return false; }
Bit8u bx_get_plugins_count_np(Bit16u) { return 0; }
const char *bx_get_plugin_name_np(Bit16u, Bit8u) { return nullptr; }
Bit8u bx_get_plugin_flags_np(Bit16u, Bit8u) { return 0; }
int bx_load_plugin_np(const char *, Bit16u) { return 0; }
int bx_unload_opt_plugin(const char *, bool) { return 0; }

extern "C" {
/* RDRAND/RDSEED entropy. Deliberately a fixed sequence: an oracle must be
 * reproducible. Callers should exclude these instructions from diffing. */
Bit64u bochscpu_rand(unsigned) {
    static Bit64u state = 0x9E37'79B9'7F4A'7C15ull;
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    return state;
}
}


/* ---------------------------------------------------------------------------
 * 5) The configuration parameter tree
 *
 * The CPU reads its feature configuration out of Bochs' parameter tree
 * (SIM->get_param_*). Bochs builds that tree in config.cc's bx_init_options(),
 * but that function also initialises the plugin/device/GUI subsystems and
 * panics without them ("no plugins found"). So we build just the sixteen
 * parameters the CPU core actually reads, with values chosen for an oracle:
 * one CPU, widest available feature set, no host-workaround hacks.
 * ------------------------------------------------------------------------- */

/* Bochs' bx_init_siminterface() creates this root list ("bochs") and
 * SIM->get_param() resolves paths against it; config.cc attaches its subtrees
 * the same way. Defined in gui/siminterface.cc. */
extern bx_list_c *root_param;
/* cpu/init.cc — index into the cpudb table for this build's default model. */
int bx_default_cpuid_model(void);

namespace {

/* The CPU model list, taken from the same cpudb.h table Bochs' own config uses,
 * so every model compiled into libcpudb.a is selectable. */
const char *cpu_model_names[] = {
#define bx_define_cpudb(model) #model,
#include "cpudb.h"
    nullptr
};

void build_param_tree() {
    bx_list_c *root = root_param; // created by bx_init_siminterface()
    new bx_list_c(root, "statistics", "Statistics");

    bx_list_c *cpu = new bx_list_c(root, "cpu", "CPU Options");

    new bx_param_enum_c(cpu, "model", "CPU configuration",
                        "Pre-defined CPU configuration", cpu_model_names,
                        bx_default_cpuid_model(), 0);
    new bx_param_string_c(cpu, "add_features", "Add features", "", "",
                          BX_PATHNAME_LEN);
    new bx_param_string_c(cpu, "exclude_features", "Exclude features", "", "",
                          BX_PATHNAME_LEN);
    new bx_param_num_c(cpu, "n_processors", "Processors", "", 1, 1, 1);
    new bx_param_num_c(cpu, "n_cores", "Cores", "", 1, 1, 1);
    new bx_param_num_c(cpu, "n_threads", "Threads", "", 1, 1, 1);
    new bx_param_num_c(cpu, "ips", "Emulated IPS", "", BX_MIN_IPS, BX_MAX_BIT32U,
                       4000000);
    new bx_param_num_c(cpu, "quantum", "SMP quantum", "", 1, 32, 16);
    /* An oracle must not restart itself: a triple fault is a result to report,
     * not a reason to reset the machine. */
    new bx_param_bool_c(cpu, "reset_on_triple_fault", "Reset on triple fault", "", 0);
    /* Unknown MSRs read as zero rather than #GP, matching the trait contract
     * that unmodelled MSRs are simply inert. */
    new bx_param_bool_c(cpu, "ignore_bad_msrs", "Ignore unknown MSRs", "", 1);
    new bx_param_bool_c(cpu, "cpuid_limit_winnt", "Limit CPUID to 3", "", 0);
    new bx_param_bool_c(cpu, "mwait_is_nop", "MWAIT is a NOP", "", 1);
    new bx_param_filename_c(cpu, "msrs", "Configurable MSR file", "", "",
                            BX_PATHNAME_LEN);
    new bx_param_string_c(cpu, "brand_string", "CPUID brand string", "",
                          "x86-oracle (Bochs CPU)", BX_CPUID_BRAND_LEN + 1);

    /* misc.* — debug conveniences of the full emulator; all off. */
    bx_list_c *misc = new bx_list_c(root, "misc", "Miscellaneous");
    new bx_param_bool_c(misc, "iodebug_all_rings", "iodebug all rings", "", 0);
    bx_list_c *e9 = new bx_list_c(misc, "port_e9_hack", "Port E9 hack");
    new bx_param_bool_c(e9, "enabled", "enabled", "", 0);
    new bx_param_bool_c(e9, "all_rings", "all rings", "", 0);
}

} // namespace

/* ---------------------------------------------------------------------------
 * The oracle ABI
 * ------------------------------------------------------------------------- */

namespace {

/* Identity-mapping page tables.
 *
 * 2 MiB pages (PDE.PS), NOT 1 GiB: a 1 GiB page needs the BX_ISA_1G_PAGES
 * extension, and on a model without it Bochs treats PDPTE.PS as a reserved bit
 * — the walk then resolves every linear address to physical 0, which shows up
 * as the CPU silently fetching zeroes. 2 MiB pages are baseline for long mode.
 *
 * Layout: PML4[0] -> PDPT -> 512 page directories, each mapping 1 GiB in 2 MiB
 * pages, so linear == physical across the low 512 GiB. The tables live in the
 * last 1 GiB of that window (see PT_BASE): a diff harness must not use those
 * ~2 MiB of addresses as data.
 */
constexpr uint64_t PT_BASE = 0x7F'C000'0000ull; // reserved: page tables
constexpr uint64_t PT_PML4 = PT_BASE;
constexpr uint64_t PT_PDPT = PT_BASE + 0x1000;
constexpr uint64_t PT_PD0 = PT_BASE + 0x2000; // 512 consecutive page directories
constexpr uint64_t PT_BYTES = 0x2000 + 512 * 0x1000; // PML4 + PDPT + the 512 PDs
constexpr uint64_t IDENTITY_LIMIT = 512ull << 30;    // 512 GiB

constexpr uint64_t PTE_P = 0x001;  // present
constexpr uint64_t PTE_W = 0x002;  // writable
constexpr uint64_t PTE_US = 0x004; // user-accessible
constexpr uint64_t PTE_D = 0x040;  // dirty
constexpr uint64_t PTE_PS = 0x080; // page size (leaf at this level)
constexpr uint64_t FRAME_2M = 2ull << 20;

void build_identity_map() {
    const uint64_t pml4e = PT_PDPT | PTE_P | PTE_W | PTE_US;
    mem().write(PT_PML4, 8, &pml4e);

    for (uint64_t i = 0; i < 512; i++) {
        const uint64_t pd = PT_PD0 + i * 0x1000;
        const uint64_t pdpte = pd | PTE_P | PTE_W | PTE_US;
        mem().write(PT_PDPT + i * 8, 8, &pdpte);

        for (uint64_t j = 0; j < 512; j++) {
            const uint64_t base = (i << 30) | (j << 21); // 2 MiB frame
            const uint64_t pde = base | PTE_P | PTE_W | PTE_US | PTE_PS;
            mem().write(pd + j * 8, 8, &pde);
        }
    }
}

/* Re-map the 2 MiB frames covering [base, base+len) as CET shadow-stack pages.
 *
 * Bochs enforces the architectural encoding in paging.cc's
 * `check_leaf_entry_faults`: a shadow-stack access requires R/W=1 in every level
 * ABOVE the leaf (the identity map already has that) and, in the leaf itself,
 * Dirty=1 with R/W=0 — which is what makes ordinary stores fault while the CPU's
 * own call/ret pushes succeed. U/S must match the CPL, so a CPL-0 shadow stack
 * clears it here; combined_access is the AND down the walk, so the US=1 above
 * does not interfere.
 *
 * The identity map uses 2 MiB frames, so this rounds outward to whole frames —
 * a caller must keep ordinary data out of the frames it converts. Seeding is
 * unaffected either way: `oracle_bochs_write_mem` reaches physical memory
 * directly and never walks the page tables.
 */
bool map_shadow_stack(uint64_t base, uint64_t len) {
    if (len == 0 || base >= IDENTITY_LIMIT || len > IDENTITY_LIMIT - base) {
        return false;
    }
    const uint64_t first = base & ~(FRAME_2M - 1);
    const uint64_t last = (base + len - 1) | (FRAME_2M - 1);
    /* The page tables live INSIDE the identity map (see PT_BASE), so the limit
     * check above does not cover them. Handing the CPU a shadow stack that
     * shares a frame with them is not a caller's problem to notice later: the
     * first CALL pushes its return address over PML4[0], and because the walk
     * keeps hitting stale TLB entries the machine runs on for a while before
     * dying somewhere unrelated. Reject the whole rounded-up window — the same
     * region `BochsOracle::RESERVED_START` documents. */
    const uint64_t pt_first = PT_BASE & ~(FRAME_2M - 1);
    const uint64_t pt_last = (PT_BASE + PT_BYTES - 1) | (FRAME_2M - 1);
    if (first <= pt_last && pt_first <= last) {
        return false;
    }
    for (uint64_t a = first; a <= last; a += FRAME_2M) {
        const uint64_t pd = PT_PD0 + (a >> 30) * 0x1000;
        const uint64_t pde = a | PTE_P | PTE_D | PTE_PS;
        mem().write(pd + ((a >> 21) & 0x1FF) * 8, 8, &pde);
    }
    return true;
}

/* Descriptor attribute words as Bochs stores them (descriptor high dword >> 8):
 * bits 0-3 type, 4 S, 5-6 DPL, 7 P, 12 AVL, 13 L, 14 D/B, 15 G. */
constexpr Bit16u ATTR_CODE64 = 0xA09B; // type=B (exec/read/accessed), S, P, L, G
constexpr Bit16u ATTR_DATA = 0xC093;   // type=3 (read/write/accessed), S, P, D/B, G

void load_segment(bx_segment_reg_t *seg, Bit16u selector, Bit16u attr) {
    bx_cpu.set_segment_ar_data(seg, /*valid*/ true, selector,
                               /*base*/ 0, /*limit_scaled*/ 0xFFFFFFFF, attr);
}

/* Put the CPU in 64-bit long mode, CPL 0, flat identity-mapped. */
void enter_long_mode() {
    bx_cpu.cr0.set32(0x8005'0033); // PE MP ET NE WP AM PG
    bx_cpu.cr4.set32(0x0004'0220); // PAE OSFXSR OSXSAVE
    bx_cpu.cr3 = PT_PML4;
    bx_cpu.efer.set32(0x0000'0D00); // LME LMA NXE
    bx_cpu.xcr0.set32(0xE7);        // x87 SSE AVX opmask zmm_hi256 hi16_zmm

    load_segment(&bx_cpu.sregs[BX_SEG_REG_CS], 0x10, ATTR_CODE64);
    for (unsigned s : {BX_SEG_REG_SS, BX_SEG_REG_DS, BX_SEG_REG_ES, BX_SEG_REG_FS,
                       BX_SEG_REG_GS}) {
        load_segment(&bx_cpu.sregs[s], 0x18, ATTR_DATA);
    }

    bx_cpu.setEFlags(0x2);

    /* Bochs' hardware reset leaves RIP at the 0xFFF0 reset vector and RDX
     * holding the CPU signature (as real hardware does). The oracle contract is
     * "everything zero", so clear the architectural registers explicitly. */
    for (unsigned r = 0; r < BX_GENERAL_REGISTERS; r++) {
        bx_cpu.gen_reg[r].rrx = 0;
    }
    bx_cpu.gen_reg[BX_64BIT_REG_RIP].rrx = 0;
    bx_cpu.prev_rip = 0;
    for (unsigned r = 0; r < BX_XMM_REGISTERS; r++) {
        for (unsigned q = 0; q < 8; q++) {
            bx_cpu.vmm[r].vmm64u(q) = 0;
        }
    }
    bx_cpu.invalidate_prefetch_q();

    /* CRITICAL: cpu_mode is derived state. Writing cr0/cr4/efer only changes the
     * register fields; without these the core stays in the mode it was reset
     * into (real mode) and fetches from IP's low 16 bits instead of RIP. */
    bx_cpu.handleCpuModeChange();
    bx_cpu.handleCpuContextChange();
    bx_cpu.handleSseModeChange();
    bx_cpu.updateFetchModeMask();
    bx_cpu.TLB_flush();
}

bool g_alive = false;

/* Bochs logs to stderr through `logfunctions`, and its defaults are tuned for a
 * full emulator run: INFO/WARN/ERROR all print, and PANIC asks the user. For an
 * oracle that is pure noise — a guest fault is an *expected result* here,
 * reported through StepOutcome, not an emulator problem. A single `ud2` case
 * emits a full register dump plus "exception(): 3rd (13) exception with no
 * resolution", and the ACT_ASK path then adds "notify called, but no
 * bxevent_callback function is registered" because this shim has no GUI to ask.
 *
 * So drop everything below PANIC. PANIC is left alone: that one means Bochs
 * itself reached an impossible state, which is a real bug and must stay visible.
 *
 * Two calls are needed because `logfunctions`' constructor copies the defaults
 * into a per-instance array — set_default_log_action only affects objects built
 * later, and bx_cpu is a file-scope global that was constructed before main().
 * The -1 form walks the modules already registered with iofunctions.
 *
 * Set BOCHS_LOG=1 to keep Bochs' own defaults when debugging the backend. */
void quiet_bochs_log() {
    const char *verbose = std::getenv("BOCHS_LOG");
    if (verbose != nullptr && verbose[0] != '\0' && std::strcmp(verbose, "0") != 0) {
        return;
    }
    const int levels[] = {LOGLEV_DEBUG, LOGLEV_INFO, LOGLEV_WARN, LOGLEV_ERROR};
    for (int level : levels) {
        SIM->set_default_log_action(level, ACT_IGNORE);
        SIM->set_log_action(-1, level, ACT_IGNORE);
    }
}

} // namespace

extern "C" {

/* Non-zero if a BochsOracle already exists: the Bochs CPU is a process
 * singleton, so the Rust side turns this into a clear error instead of silent
 * state sharing. */
int oracle_bochs_alive(void) { return g_alive ? 1 : 0; }

/* Which BxCpuMode the core is in; 4 == BX_MODE_LONG_64. Exposed so a test can
 * assert the oracle really is in long mode rather than silently in real mode. */
uint32_t oracle_bochs_cpu_mode(void) { return (uint32_t)bx_cpu.cpu_mode; }

void *oracle_bochs_new(void) {
    if (g_alive) {
        return nullptr;
    }

    static bool one_time_init = false;
    if (!one_time_init) {
        one_time_init = true;
        /* Bochs' own start-up order (main.cc:bx_init_main): the logging
         * singletons must exist before anything logs, and every logfunctions
         * subclass we own needs a live sink — a NULL one trips an assert deep
         * inside the first ldebug() call. */
        SAFE_GET_IOFUNC();
        SAFE_GET_GENLOG();
        pluginlog = new logfunctions();
        pluginlog->put("PLGN");
        bx_init_siminterface();
        /* As early as SIM allows — CPU model selection and initialize() are
         * themselves chatty. bx_cpu is already registered with iofunctions
         * (logfunctions' constructor does that), so the -1 sweep reaches it
         * even though the object predates main(). */
        quiet_bochs_log();
        build_param_tree();
        /* "bx_generic" enables everything the build supports (AVX-512, BMI,
         * CET...), which is the point of this backend. */
        SIM->get_param_enum(BXPN_CPU_MODEL)->set_by_name("bx_generic");
        /* ...except CET, which bx_generic does not claim (only tigerlake,
         * sapphire_rapids and arrow_lake do in cpudb). Without the extension
         * every CET instruction except RDSSP — which is deliberately ungated,
         * being a NOP on older CPUs — decodes to BxError and reports #UD, so
         * the whole group would look unimplemented instead of merely disabled.
         * Enabling the extension alone changes nothing until CR4.CET is set
         * (see oracle_bochs_enable_shadow_stack): the instructions then behave
         * as the ISA says for "shadow stacks off". Must precede initialize(),
         * which is what applies this parameter. */
        SIM->get_param_string(BXPN_CPU_ADD_FEATURES)->set("cet");
        /* A20 masking is applied to EVERY physical address (A20ADDR in
         * paging.cc). bx_pc_system's mask is zero until this is called, which
         * silently translates every linear address to physical 0 — the CPU then
         * fetches zeroes no matter what the page tables say. */
        bx_pc_system.set_enable_a20(1);
        bx_cpu.initialize();
        bx_cpu.sanity_checks();
    }

    /* Fresh address space + architectural reset for every instance. */
    if (g_mem != nullptr) {
        g_mem->clear();
    }
    bx_cpu.reset(BX_RESET_HARDWARE);
    build_identity_map();
    enter_long_mode();

    g_step = StepCtl();
    g_alive = true;
    /* The handle is a token: all state lives in the process singletons. */
    return reinterpret_cast<void *>(1);
}

void oracle_bochs_free(void *) {
    if (g_mem != nullptr) {
        g_mem->clear();
    }
    g_alive = false;
}

/* -- registers ------------------------------------------------------------ */

/* Rust passes the ModRM index; Bochs' gen_reg array uses the same order. */
void oracle_bochs_set_gpr(uint32_t n, uint64_t v) { bx_cpu.gen_reg[n & 0xF].rrx = v; }
uint64_t oracle_bochs_get_gpr(uint32_t n) { return bx_cpu.gen_reg[n & 0xF].rrx; }

void oracle_bochs_set_rip(uint64_t v) {
    bx_cpu.gen_reg[BX_64BIT_REG_RIP].rrx = v;
    bx_cpu.prev_rip = v;
    bx_cpu.invalidate_prefetch_q();
}
uint64_t oracle_bochs_get_rip(void) { return bx_cpu.gen_reg[BX_64BIT_REG_RIP].rrx; }

/* read_eflags() forces the lazy OSZAPC flags — see the header comment.
 *
 * TF and RF are masked out on both directions, matching the KVM and WHP
 * backends: they belong to whatever the backend uses to stop after one
 * instruction, not to the program under test. Bochs sets RF itself whenever it
 * parks RIP on a repeating string operation, so without this a `rep movs` mid
 * -loop reads back 0x10002 here and 0x2 on hardware. */
static const Bit32u kHarnessFlags = 0x100 /* TF */ | 0x10000 /* RF */;

uint64_t oracle_bochs_get_rflags(void) { return bx_cpu.read_eflags() & ~kHarnessFlags; }
void oracle_bochs_set_rflags(uint64_t v) {
    bx_cpu.setEFlags(((Bit32u)v & ~kHarnessFlags) | 0x2);
}

void oracle_bochs_set_zmm(uint32_t n, uint32_t chunk, uint64_t v) {
    bx_cpu.vmm[n & 0x1F].vmm64u(chunk & 0x7) = v;
}
uint64_t oracle_bochs_get_zmm(uint32_t n, uint32_t chunk) {
    return bx_cpu.vmm[n & 0x1F].vmm64u(chunk & 0x7);
}

/* MSRs by architectural number, through the CPU's own RDMSR/WRMSR paths where
 * possible so side effects (e.g. EFER.LMA) behave architecturally. */
void oracle_bochs_set_msr(uint32_t msr, uint64_t v) {
    switch (msr) {
    case 0xC000'0080: bx_cpu.efer.set32((Bit32u)v); break;
    case 0xC000'0081: bx_cpu.msr.star = v; break;
    case 0xC000'0082: bx_cpu.msr.lstar = v; break;
    case 0xC000'0084: bx_cpu.msr.fmask = v; break;
    case 0xC000'0100:
        bx_cpu.sregs[BX_SEG_REG_FS].cache.u.segment.base = v;
        break;
    case 0xC000'0101:
        bx_cpu.sregs[BX_SEG_REG_GS].cache.u.segment.base = v;
        break;
    case 0xC000'0102: bx_cpu.msr.kernelgsbase = v; break;
#if BX_SUPPORT_CET
    case 0x6A2: bx_cpu.msr.ia32_cet_control[0] = v; break; // IA32_S_CET
    case 0x6A0: bx_cpu.msr.ia32_cet_control[1] = v; break; // IA32_U_CET
#endif
    default: break; // not modelled: ignored, per the trait contract
    }
}

uint64_t oracle_bochs_get_msr(uint32_t msr) {
    switch (msr) {
    case 0xC000'0080: return bx_cpu.efer.get32();
    case 0xC000'0081: return bx_cpu.msr.star;
    case 0xC000'0082: return bx_cpu.msr.lstar;
    case 0xC000'0084: return bx_cpu.msr.fmask;
    case 0xC000'0100: return bx_cpu.sregs[BX_SEG_REG_FS].cache.u.segment.base;
    case 0xC000'0101: return bx_cpu.sregs[BX_SEG_REG_GS].cache.u.segment.base;
    case 0xC000'0102: return bx_cpu.msr.kernelgsbase;
#if BX_SUPPORT_CET
    case 0x6A2: return bx_cpu.msr.ia32_cet_control[0];
    case 0x6A0: return bx_cpu.msr.ia32_cet_control[1];
#endif
    default: return 0;
    }
}

/* SSP is a register, not an MSR, and it sits past the 16 ModRM slots that
 * `oracle_bochs_set_gpr` masks into — so it needs its own door. */
void oracle_bochs_set_ssp(uint64_t v) {
#if BX_SUPPORT_CET
    bx_cpu.gen_reg[BX_64BIT_REG_SSP].rrx = v;
#else
    (void)v;
#endif
}

uint64_t oracle_bochs_get_ssp(void) {
#if BX_SUPPORT_CET
    return bx_cpu.gen_reg[BX_64BIT_REG_SSP].rrx;
#else
    return 0;
#endif
}

/* Turn CET shadow stacks on and give them somewhere to live. Opt-in on purpose:
 * with shadow stacks enabled every CALL pushes and every RET pops and COMPARES,
 * so a caller must also point SSP at the mapped region — which is not something
 * the plain reset state should impose on every test.
 *
 * Returns 0 on success, 1 if this build/model has no CET, 2 if the range was
 * refused. The caller turns those into ShadowStackError, and the distinction is
 * the point: "no CET here" is a reason to skip a case, a bad range is a bug. */
int oracle_bochs_enable_shadow_stack(uint64_t base, uint64_t len) {
#if BX_SUPPORT_CET
    /* The compile-time guard only says the Bochs *build* has the code. Whether
     * this CPU model claims the extension is a runtime question, and the answer
     * decides whether the instructions decode at all — oracle_bochs_new asks for
     * it by name (`add_features = "cet"`), a string that a future Bochs could
     * rename out from under us. Without this check CR4.CET would still be set,
     * because set32 does no validation, and every CET instruction would then be
     * #UD while this function reported success. */
    if (!bx_cpu.is_cpu_extension_supported(BX_ISA_CET)) {
        return 1;
    }
    if (!map_shadow_stack(base, len)) {
        return 2;
    }
    /* CR0.WP is an architectural precondition of CR4.CET (Bochs enforces the
     * converse in crregs.cc: clearing WP while CET is on is #GP). The reset
     * state already sets it, so this only matters if a caller wrote CR0 first —
     * and writing CR4.CET without it would build a machine no CPU can be. */
    bx_cpu.cr0.set32(bx_cpu.cr0.get32() | (1u << 16)); // CR0.WP
    bx_cpu.cr4.set32(bx_cpu.cr4.get32() | (1u << 23)); // CR4.CET
    /* SH_STK_EN | WR_SHSTK_EN, for both CPL 0 (where the oracle runs) and CPL 3.
     * WRSS needs the second bit; without it the instruction is #UD. */
    bx_cpu.msr.ia32_cet_control[0] = 0x3;
    bx_cpu.msr.ia32_cet_control[1] = 0x3;
    bx_cpu.updateFetchModeMask(); // CR0.WP feeds the paging privilege checks
    bx_cpu.TLB_flush();
    return 0;
#else
    (void)base;
    (void)len;
    return 1;
#endif
}

void oracle_bochs_set_cr(uint32_t n, uint64_t v) {
    switch (n) {
    case 0: bx_cpu.cr0.set32((Bit32u)v); bx_cpu.updateFetchModeMask(); break;
    case 2: bx_cpu.cr2 = v; break;
    case 3: bx_cpu.cr3 = v; bx_cpu.TLB_flush(); break;
    case 4: bx_cpu.cr4.set32((Bit32u)v); bx_cpu.TLB_flush(); break;
    default: break;
    }
}

uint64_t oracle_bochs_get_cr(uint32_t n) {
    switch (n) {
    case 0: return bx_cpu.cr0.get32();
    case 2: return bx_cpu.cr2;
    case 3: return bx_cpu.cr3;
    case 4: return bx_cpu.cr4.get32();
    default: return 0;
    }
}

/* Rust's SEG_* order is the x86isa one (ES CS SS DS FS GS); Bochs' enum is
 * ES=0 CS=1 SS=2 DS=3 FS=4 GS=5 as well, so the index passes through. */
void oracle_bochs_set_seg_selector(uint32_t n, uint64_t v) {
    bx_cpu.sregs[n % 6].selector.value = (Bit16u)v;
}
uint64_t oracle_bochs_get_seg_selector(uint32_t n) {
    return bx_cpu.sregs[n % 6].selector.value;
}
void oracle_bochs_set_seg_base(uint32_t n, uint64_t v) {
    bx_cpu.sregs[n % 6].cache.u.segment.base = v;
}
uint64_t oracle_bochs_get_seg_base(uint32_t n) {
    return bx_cpu.sregs[n % 6].cache.u.segment.base;
}
void oracle_bochs_set_seg_limit(uint32_t n, uint64_t v) {
    bx_cpu.sregs[n % 6].cache.u.segment.limit_scaled = (Bit32u)v;
}
uint64_t oracle_bochs_get_seg_limit(uint32_t n) {
    return bx_cpu.sregs[n % 6].cache.u.segment.limit_scaled;
}

/* GDTR (which == 0) / IDTR (which == 1). Register values only — the Rust side
 * uses this to mirror the hardware backends' table addresses; no memory backs
 * the tables here. That is safe NOT because delivery never reads the IDT — it
 * does: bx_instr_exception only sets stop flags, and exception() then walks
 * the IDT regardless — but because guest memory is sparse and reads as zeros,
 * so every gate is non-present and delivery aborts (cascading into Bochs'
 * "3rd exception with no resolution" bail-out) before committing any state;
 * bx_instr_exception's guard keeps only the first vector. Do NOT mirror real
 * tables into guest memory: a present gate would let delivery SUCCEED and
 * vector through the IDT, breaking the faults-do-not-vector contract. */
void oracle_bochs_set_dtr(uint32_t which, uint64_t base, uint64_t limit) {
    bx_global_segment_reg_t *r = which ? &bx_cpu.idtr : &bx_cpu.gdtr;
    r->base = (bx_address)base;
    r->limit = (Bit16u)limit;
}
/* The selector alone — the hidden TSS base/limit stay at reset, since nothing
 * here delivers through a TSS. STR reads only the selector. */
void oracle_bochs_set_tr_selector(uint64_t v) {
    bx_cpu.tr.selector.value = (Bit16u)v;
}

/* -- memory (linear addresses; identity-mapped, so linear == physical) ---- */

uint8_t oracle_bochs_read_mem(uint64_t addr) {
    uint8_t b = 0;
    mem().read(addr, 1, &b);
    return b;
}

/* Bochs caches decoded instruction traces keyed by PHYSICAL address, and its own
 * stores keep that cache honest by stamping the written page (memory.cc's
 * `decWriteStamp`, the self-modifying-code path). A host-side write from this
 * API bypasses all of that: it goes straight into GuestMemory, so re-running an
 * address that already executed would replay the OLD decoded instruction while
 * memory plainly holds the new bytes.
 *
 * That is precisely what `step_bytes` does — write code at RIP, step, rewind,
 * write different code at the same RIP — so stamp every host write the same way
 * Bochs stamps its own. It is O(1): the table is only consulted, and the icache
 * only walked, for pages a trace was actually built from. Writes are one byte at
 * a time here, so `decWriteStamp`'s "must not split a page" contract holds.
 * Above 4 GiB the stamp hash aliases, which can only over-invalidate.
 *
 * The prefetch queue holds raw bytes for the current page and is not covered by
 * the stamp table, so it goes too. */
void invalidate_decoded(uint64_t addr) {
    pageWriteStampTable.decWriteStamp((bx_phy_address)addr, 1);
    bx_cpu.invalidate_prefetch_q();
}

void oracle_bochs_write_mem(uint64_t addr, uint8_t byte) {
    mem().write(addr, 1, &byte);
    invalidate_decoded(addr);
}

void oracle_bochs_read_mem_slice(uint64_t addr, uint8_t *dst, uint64_t len) {
    for (uint64_t i = 0; i < len; i++) {
        mem().read(addr + i, 1, dst + i);
    }
}

void oracle_bochs_write_mem_slice(uint64_t addr, const uint8_t *src, uint64_t len) {
    for (uint64_t i = 0; i < len; i++) {
        mem().write(addr + i, 1, src + i);
        invalidate_decoded(addr + i);
    }
}

int oracle_bochs_is_mapped(uint64_t addr) { return mem().mapped(addr) ? 1 : 0; }

/* -- execution ----------------------------------------------------------- */

/* Returns 0 = retired, 1 = fault (vector/error via the out-params), 2 = HLT.
 * One call runs exactly one instruction (or one REP iteration). */
uint32_t oracle_bochs_step(uint32_t *out_vector, uint32_t *out_error) {
    g_step = StepCtl();
    g_step.armed = true;

    bx_cpu.async_event = 0;
    bx_pc_system.kill_bochs_request = 0;

    /* cpu_loop returns when an instrumentation callback asks it to. The guard
     * bounds pathological cases (an instruction that neither retires nor
     * faults would otherwise spin forever). */
    for (int guard = 0; guard < 1024; guard++) {
        if (bx_pc_system.kill_bochs_request) {
            break;
        }
        bx_cpu.cpu_loop();
    }

    g_step.armed = false;
    bx_pc_system.kill_bochs_request = 0;
    bx_cpu.async_event = 0;

    if (g_step.faulted) {
        if (out_vector != nullptr) {
            *out_vector = g_step.vector;
        }
        if (out_error != nullptr) {
            *out_error = g_step.error_code;
        }
        return 1;
    }
    if (g_step.halted) {
        return 2;
    }
    return 0;
}

} // extern "C"

/* ---------------------------------------------------------------------------
 * The CPU itself — defined last on purpose: its constructor reaches into
 * bx_pc_system and the logging singletons above, which must already be alive
 * (see the note at the top of the globals section).
 * ------------------------------------------------------------------------- */

BOCHSAPI BX_CPU_C bx_cpu;
