//! KVM backend: **the host CPU itself** as the reference, on Linux.
//!
//! This is ground truth — not a model of x86, but x86. Where a model and this
//! backend disagree, the model is wrong (or the instruction's behaviour is
//! architecturally undefined and both are allowed).
//!
//! The trade-off is that "the host CPU" is a specific CPU: coverage is whatever
//! *this* machine implements, results can differ between machines for
//! implementation-defined behaviour, and a few instructions (`RDTSC`,
//! `RDRAND`, `CPUID`) are not reproducible references at all. See
//! [`Self::host_vector_width`] and `docs/backend-differences.md`.
//!
//! The guest itself — RAM, page tables, descriptor tables, the fault stubs — is
//! [`crate::hw`], shared with the Windows backend
//! ([`WhpOracle`](crate::WhpOracle)); this file is only the KVM half.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use kvm_bindings::{
    kvm_dtable, kvm_guest_debug, kvm_msr_entry, kvm_regs, kvm_segment, kvm_userspace_memory_region,
    kvm_xcr, Msrs, KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, KVM_MAX_CPUID_ENTRIES,
};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};

use crate::hw::{
    self, GuestRam, XsaveLayout, CR0_INIT, CR4_INIT, CTRL_BASE, EFER_INIT, GDT_ADDR, GDT_LIMIT,
    IDT_ADDR, IDT_LIMIT, PML4_ADDR, RAM_SIZE, SEL_CODE64, SEL_DATA, SEL_TSS, TSS_ADDR, TSS_LIMIT,
};
use crate::{
    FaultKind, StepOutcome, X86Oracle, ZMM_CHUNKS, ZMM_REGS, FLAG_RF, FLAG_TF, IA32_EFER,
    IA32_FMASK, IA32_FS_BASE, IA32_GS_BASE, IA32_KERNEL_GS_BASE, IA32_LSTAR, IA32_STAR, SEG_COUNT,
    SEG_CS, SEG_DS, SEG_ES, SEG_FS, SEG_GS, SEG_SS,
};

// ---------------------------------------------------------------------- oracle

/// KVM's own state is per-VM, so instances are independent; but the crate
/// serializes anyway to keep the API uniform and because `Kvm::new` is cheap
/// only when not contended.
static KVM_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    KVM_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct KvmOracle {
    // Drop order matters: the vCPU and VM must go before the RAM their memory
    // slot points at. Rust drops fields in declaration order.
    vcpu: VcpuFd,
    _vm: VmFd,
    _kvm: Kvm,
    ram: GuestRam,
    xsave: XsaveLayout,
    last_fault: Option<(u8, u64)>,
    /// Pages this instance has written through the direct memory API. Guest
    /// stores are invisible here, so `is_mapped` is a hint (as documented).
    touched: HashSet<u64>,
}

impl KvmOracle {
    /// Linear addresses below this are usable; above it there is no RAM.
    pub const ADDRESS_LIMIT: u64 = CTRL_BASE;
    /// Page tables, GDT/IDT and the fault stubs live here — do not use this
    /// region as data.
    pub const RESERVED_START: u64 = CTRL_BASE;

    /// Whether this host can be used as a KVM oracle at all: KVM present and
    /// `/dev/kvm` openable by this user. Check it before [`new`](Self::new) in
    /// code that must not panic.
    pub fn is_available() -> bool {
        Kvm::new().is_ok()
    }

    /// A fresh VM in the reset state described at the crate level.
    ///
    /// # Panics
    /// If KVM is unavailable — most often because `/dev/kvm` is not readable by
    /// this user (`sudo chmod 666 /dev/kvm`, or join the `kvm` group). Use
    /// [`try_new`](Self::try_new) to handle that.
    pub fn new() -> Self {
        Self::try_new().expect(
            "KVM is unavailable: check that /dev/kvm exists and is accessible \
             (sudo chmod 666 /dev/kvm, or join the kvm group), and that this is a \
             Linux host with hardware virtualization enabled",
        )
    }

    pub fn try_new() -> std::io::Result<Self> {
        let _g = lock();

        let kvm = Kvm::new()?;
        let vm = kvm.create_vm()?;
        let ram = GuestRam::new(RAM_SIZE as usize)?;

        let region = kvm_userspace_memory_region {
            slot: 0,
            flags: 0,
            guest_phys_addr: 0,
            memory_size: RAM_SIZE,
            userspace_addr: ram.host_ptr() as u64,
        };
        // SAFETY: the region describes our own live mapping, and `ram` outlives
        // the VM (drop order in the struct).
        unsafe { vm.set_user_memory_region(region)? };

        let vcpu = vm.create_vcpu(0)?;
        // Without an explicit CPUID the guest sees none of the host's features.
        let cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)?;
        vcpu.set_cpuid2(&cpuid)?;

        let mut oracle = KvmOracle {
            vcpu,
            _vm: vm,
            _kvm: kvm,
            ram,
            // Placeholder: the real layout can only be probed once XCR0 is set,
            // which enter_long_mode does.
            xsave: XsaveLayout::probe(&hw::host_cpuid, 0, 0),
            last_fault: None,
            touched: HashSet::new(),
        };
        hw::build_control_structures(&mut oracle.ram);
        oracle.enter_long_mode()?;
        Ok(oracle)
    }

    /// Widest vector register the host CPU implements, in bits. On a host
    /// without AVX-512 this is 256 (or 128 without AVX): `get_zmm` chunks above
    /// that read as zero, and AVX-512 encodings raise #UD — which is the
    /// *correct* ground truth for this machine, not a model bug. Skip such cases
    /// when diffing against a backend that implements them.
    pub fn host_vector_width(&self) -> usize {
        self.xsave.vector_width()
    }

    /// Diagnostic dump of the control state KVM actually holds (it may reject or
    /// normalize parts of what we set).
    pub fn debug_sregs(&self) -> String {
        let s = self.vcpu.get_sregs().expect("sregs");
        format!(
            "cr0={:#x} cr3={:#x} cr4={:#x} efer={:#x} cs{{sel:{:#x} base:{:#x} l:{} db:{} type:{:#x} p:{}}} \
             ss{{sel:{:#x} type:{:#x} p:{}}} tr{{sel:{:#x} type:{:#x} p:{} unusable:{}}} \
             ldt{{unusable:{}}} gdt{{base:{:#x} limit:{:#x}}} idt{{base:{:#x} limit:{:#x}}}",
            s.cr0, s.cr3, s.cr4, s.efer,
            s.cs.selector, s.cs.base, s.cs.l, s.cs.db, s.cs.type_, s.cs.present,
            s.ss.selector, s.ss.type_, s.ss.present,
            s.tr.selector, s.tr.type_, s.tr.present, s.tr.unusable,
            s.ldt.unusable,
            s.gdt.base, s.gdt.limit, s.idt.base, s.idt.limit
        )
    }

    /// The exception vector of the last fault (6 = #UD, 0 = #DE, ...).
    pub fn last_vector(&self) -> Option<u8> {
        self.last_fault.map(|(v, _)| v)
    }

    /// The error code the CPU pushed with the last fault, if that vector has
    /// one.
    pub fn last_error_code(&self) -> Option<u64> {
        self.last_fault.and_then(|(v, e)| hw::has_error_code(v).then_some(e))
    }

    /// Put the vCPU in 64-bit long mode, CPL 0, with the flat segments and the
    /// tables `hw::build_control_structures` laid down.
    fn enter_long_mode(&mut self) -> std::io::Result<()> {
        let mut sregs = self.vcpu.get_sregs()?;

        let code = kvm_segment {
            base: 0,
            limit: 0xFFFF_FFFF,
            selector: SEL_CODE64,
            type_: 0b1011, // execute/read, accessed
            present: 1,
            dpl: 0,
            db: 0,
            s: 1,
            l: 1, // 64-bit
            g: 1,
            avl: 0,
            unusable: 0,
            padding: 0,
        };
        let data = kvm_segment {
            base: 0,
            limit: 0xFFFF_FFFF,
            selector: SEL_DATA,
            type_: 0b0011, // read/write, accessed
            present: 1,
            dpl: 0,
            db: 1,
            s: 1,
            l: 0,
            g: 1,
            avl: 0,
            unusable: 0,
            padding: 0,
        };

        sregs.cs = code;
        sregs.ds = data;
        sregs.es = data;
        sregs.ss = data;
        sregs.fs = data;
        sregs.gs = data;
        sregs.gdt = kvm_dtable { base: GDT_ADDR, limit: GDT_LIMIT, padding: [0; 3] };
        // TR must point at the TSS holding IST1, and be marked busy.
        sregs.tr = kvm_segment {
            base: TSS_ADDR,
            limit: TSS_LIMIT,
            selector: SEL_TSS,
            type_: 11, // busy 64-bit TSS
            present: 1,
            dpl: 0,
            db: 0,
            s: 0,
            l: 0,
            g: 0,
            avl: 0,
            unusable: 0,
            padding: 0,
        };
        sregs.idt = kvm_dtable { base: IDT_ADDR, limit: IDT_LIMIT, padding: [0; 3] };
        sregs.cr0 = CR0_INIT;
        sregs.cr3 = PML4_ADDR;
        sregs.cr4 = CR4_INIT;
        sregs.efer = EFER_INIT;
        self.vcpu.set_sregs(&sregs)?;

        // Enable every XSAVE feature the host has, so the guest may use its
        // widest vector registers, then read back what KVM accepted.
        let host_xcr0 = {
            let leaf = hw::host_cpuid(0x0D, 0);
            ((leaf.edx as u64) << 32) | leaf.eax as u64
        };
        let mut xcrs = self.vcpu.get_xcrs()?;
        xcrs.nr_xcrs = 1;
        xcrs.xcrs[0] = kvm_xcr { xcr: 0, reserved: 0, value: host_xcr0 };
        self.vcpu.set_xcrs(&xcrs)?;
        let effective_xcr0 = self
            .vcpu
            .get_xcrs()
            .ok()
            .and_then(|x| x.xcrs.iter().take(x.nr_xcrs as usize).find(|c| c.xcr == 0).map(|c| c.value))
            .unwrap_or(host_xcr0);

        let regs = kvm_regs { rflags: 0x2, ..Default::default() };
        self.vcpu.set_regs(&regs)?;

        // Vector state starts zeroed like the other backends. KVM_GET_XSAVE
        // always hands back the standard (non-compacted) format, but the layout
        // is derived from the image itself rather than assumed.
        let mut xsave = self.vcpu.get_xsave()?;
        let bytes = xsave_bytes_mut(&mut xsave);
        self.xsave =
            XsaveLayout::probe(&hw::host_cpuid, effective_xcr0, hw::xsave_xcomp_bv(bytes));
        hw::xsave_reset_vectors(bytes, &self.xsave);
        // SAFETY: `xsave` came from KVM_GET_XSAVE and only the vector-state
        // bytes were modified, so the region stays structurally valid.
        unsafe { self.vcpu.set_xsave(&xsave)? };
        Ok(())
    }

    /// Arm single-stepping for the *current* RIP.
    ///
    /// Must be done immediately before each `KVM_RUN`, not once at startup:
    /// KVM implements single-step with RFLAGS.TF, so any later `KVM_SET_REGS`
    /// (every `set_rip`/`set_gpr` here) would clear it and the guest would run
    /// away — which shows up as a triple fault once it wanders off the code.
    /// KVM also records `singlestep_rip` at this point to decide whether a #DB
    /// belongs to the harness.
    fn arm_singlestep(&self) {
        let debug = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: Default::default(),
        };
        self.vcpu.set_guest_debug(&debug).expect("KVM_SET_GUEST_DEBUG");
    }

    fn regs(&self) -> kvm_regs {
        self.vcpu.get_regs().expect("KVM_GET_REGS")
    }

    fn set_regs(&self, regs: &kvm_regs) {
        self.vcpu.set_regs(regs).expect("KVM_SET_REGS");
    }

    /// Read one architectural MSR. EFER and the FS/GS bases live in sregs, the
    /// rest go through `KVM_GET_MSRS`.
    fn read_msr(&self, index: u32) -> u64 {
        let entries = vec![kvm_msr_entry { index, reserved: 0, data: 0 }];
        let mut msrs = Msrs::from_entries(&entries).expect("Msrs::from_entries");
        match self.vcpu.get_msrs(&mut msrs) {
            Ok(n) if n == 1 => msrs.as_slice()[0].data,
            _ => 0,
        }
    }

    fn write_msr(&self, index: u32, data: u64) {
        let entries = vec![kvm_msr_entry { index, reserved: 0, data }];
        let msrs = Msrs::from_entries(&entries).expect("Msrs::from_entries");
        let _ = self.vcpu.set_msrs(&msrs);
    }

    fn segment(&self, n: u32) -> kvm_segment {
        let sregs = self.vcpu.get_sregs().expect("KVM_GET_SREGS");
        match n {
            SEG_ES => sregs.es,
            SEG_CS => sregs.cs,
            SEG_SS => sregs.ss,
            SEG_DS => sregs.ds,
            SEG_FS => sregs.fs,
            _ => sregs.gs,
        }
    }

    fn modify_segment(&self, n: u32, f: impl FnOnce(&mut kvm_segment)) {
        let mut sregs = self.vcpu.get_sregs().expect("KVM_GET_SREGS");
        let slot = match n {
            SEG_ES => &mut sregs.es,
            SEG_CS => &mut sregs.cs,
            SEG_SS => &mut sregs.ss,
            SEG_DS => &mut sregs.ds,
            SEG_FS => &mut sregs.fs,
            _ => &mut sregs.gs,
        };
        f(slot);
        self.vcpu.set_sregs(&sregs).expect("KVM_SET_SREGS");
    }

    /// `modify_segment` for the non-segment sregs fields.
    fn modify_sregs(&self, f: impl FnOnce(&mut kvm_bindings::kvm_sregs)) {
        let mut sregs = self.vcpu.get_sregs().expect("KVM_GET_SREGS");
        f(&mut sregs);
        self.vcpu.set_sregs(&sregs).expect("KVM_SET_SREGS");
    }

    /// Undo the hardware's exception vectoring so the observable state matches
    /// the other backends: RIP back on the faulting instruction, RSP back to its
    /// pre-fault value. Returns (vector, error code).
    fn unwind_fault(&mut self, vector: u8) -> (u8, u64) {
        let mut regs = self.regs();
        let frame = hw::unwind_frame(&self.ram, regs.rsp, vector);
        regs.rip = frame.rip;
        regs.rsp = frame.rsp;
        regs.rflags = frame.rflags;
        self.set_regs(&regs);
        (vector, frame.error_code)
    }
}

/// Reinterpret the `[u32; 1024]` XSAVE region as bytes.
fn xsave_bytes_mut(xsave: &mut kvm_bindings::kvm_xsave) -> &mut [u8] {
    // SAFETY: the region is a plain POD array; u8 has weaker alignment than u32.
    unsafe {
        std::slice::from_raw_parts_mut(
            xsave.region.as_mut_ptr().cast::<u8>(),
            std::mem::size_of_val(&xsave.region),
        )
    }
}

fn xsave_bytes(xsave: &kvm_bindings::kvm_xsave) -> &[u8] {
    // SAFETY: as above.
    unsafe {
        std::slice::from_raw_parts(
            xsave.region.as_ptr().cast::<u8>(),
            std::mem::size_of_val(&xsave.region),
        )
    }
}

impl Default for KvmOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl X86Oracle for KvmOracle {
    fn backend_name(&self) -> &'static str {
        "kvm"
    }

    /// The host's widest vector register, in 64-bit chunks. On a CPU without
    /// AVX-512 this is 4 (YMM); the remaining chunks read as zero.
    fn vector_chunks(&self) -> usize {
        self.xsave.vector_width() / 64
    }

    fn vector_regs(&self) -> usize {
        self.xsave.vector_regs()
    }

    fn set_gpr(&mut self, n: u32, value: u64) {
        let _g = lock();
        let mut regs = self.regs();
        // KVM's kvm_regs is in its own order; map from the ModRM index.
        match n {
            0 => regs.rax = value,
            1 => regs.rcx = value,
            2 => regs.rdx = value,
            3 => regs.rbx = value,
            4 => regs.rsp = value,
            5 => regs.rbp = value,
            6 => regs.rsi = value,
            7 => regs.rdi = value,
            8 => regs.r8 = value,
            9 => regs.r9 = value,
            10 => regs.r10 = value,
            11 => regs.r11 = value,
            12 => regs.r12 = value,
            13 => regs.r13 = value,
            14 => regs.r14 = value,
            _ => regs.r15 = value,
        }
        self.set_regs(&regs);
    }

    fn get_gpr(&self, n: u32) -> u64 {
        let _g = lock();
        let regs = self.regs();
        match n {
            0 => regs.rax,
            1 => regs.rcx,
            2 => regs.rdx,
            3 => regs.rbx,
            4 => regs.rsp,
            5 => regs.rbp,
            6 => regs.rsi,
            7 => regs.rdi,
            8 => regs.r8,
            9 => regs.r9,
            10 => regs.r10,
            11 => regs.r11,
            12 => regs.r12,
            13 => regs.r13,
            14 => regs.r14,
            _ => regs.r15,
        }
    }

    fn set_rip(&mut self, value: u64) {
        let _g = lock();
        let mut regs = self.regs();
        regs.rip = value;
        self.set_regs(&regs);
    }

    fn get_rip(&self) -> u64 {
        let _g = lock();
        self.regs().rip
    }

    /// TF/RF are masked out: single-stepping is implemented with the trap flag,
    /// so the guest's raw RFLAGS carries state that belongs to the harness, not
    /// to the program under test.
    fn set_rflags(&mut self, value: u64) {
        let _g = lock();
        let mut regs = self.regs();
        regs.rflags = (value & !(FLAG_TF | FLAG_RF)) | 0x2;
        self.set_regs(&regs);
    }

    fn get_rflags(&self) -> u64 {
        let _g = lock();
        self.regs().rflags & !(FLAG_TF | FLAG_RF)
    }

    /// Chunks the host cannot hold are dropped (see
    /// [`host_vector_width`](Self::host_vector_width)).
    fn set_zmm(&mut self, n: u32, data: &[u64]) {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = lock();
        let mut xsave = self.vcpu.get_xsave().expect("KVM_GET_XSAVE");
        let layout = self.xsave;
        hw::xsave_set_zmm(xsave_bytes_mut(&mut xsave), &layout, n, data);
        // SAFETY: as in enter_long_mode — a KVM-provided region with only the
        // vector-state bytes and XSTATE_BV changed.
        unsafe { self.vcpu.set_xsave(&xsave) }.expect("KVM_SET_XSAVE");
    }

    fn get_zmm(&self, n: u32) -> [u64; ZMM_CHUNKS] {
        debug_assert!((n as usize) < ZMM_REGS);
        let _g = lock();
        let xsave = self.vcpu.get_xsave().expect("KVM_GET_XSAVE");
        hw::xsave_get_zmm(xsave_bytes(&xsave), &self.xsave, n)
    }

    fn set_msr(&mut self, msr: u32, value: u64) {
        let _g = lock();
        match msr {
            // These are sregs fields; writing them there keeps sregs and the
            // MSR view consistent.
            IA32_EFER => self.modify_sregs(|sregs| sregs.efer = value),
            IA32_FS_BASE => self.modify_segment(SEG_FS, |s| s.base = value),
            IA32_GS_BASE => self.modify_segment(SEG_GS, |s| s.base = value),
            IA32_STAR | IA32_LSTAR | IA32_FMASK | IA32_KERNEL_GS_BASE => {
                self.write_msr(msr, value)
            }
            _ => {} // not modelled: ignored, per the trait contract
        }
    }

    fn get_msr(&self, msr: u32) -> u64 {
        let _g = lock();
        match msr {
            IA32_EFER => self.vcpu.get_sregs().map(|s| s.efer).unwrap_or(0),
            IA32_FS_BASE => self.segment(SEG_FS).base,
            IA32_GS_BASE => self.segment(SEG_GS).base,
            IA32_STAR | IA32_LSTAR | IA32_FMASK | IA32_KERNEL_GS_BASE => self.read_msr(msr),
            _ => 0,
        }
    }

    fn set_cr(&mut self, n: u32, value: u64) {
        let _g = lock();
        self.modify_sregs(|sregs| match n {
            0 => sregs.cr0 = value,
            2 => sregs.cr2 = value,
            3 => sregs.cr3 = value,
            4 => sregs.cr4 = value,
            8 => sregs.cr8 = value,
            _ => {}
        });
    }

    fn get_cr(&self, n: u32) -> u64 {
        let _g = lock();
        let Ok(sregs) = self.vcpu.get_sregs() else { return 0 };
        match n {
            0 => sregs.cr0,
            2 => sregs.cr2,
            3 => sregs.cr3,
            4 => sregs.cr4,
            8 => sregs.cr8,
            _ => 0,
        }
    }

    fn set_seg_selector(&mut self, n: u32, value: u16) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        self.modify_segment(n, |s| s.selector = value);
    }

    fn get_seg_selector(&self, n: u32) -> u16 {
        let _g = lock();
        self.segment(n).selector
    }

    fn set_seg_base(&mut self, n: u32, value: u64) {
        debug_assert!((n as usize) < SEG_COUNT);
        let _g = lock();
        self.modify_segment(n, |s| s.base = value);
    }

    fn get_seg_base(&self, n: u32) -> u64 {
        let _g = lock();
        self.segment(n).base
    }

    fn set_seg_limit(&mut self, n: u32, value: u32) {
        let _g = lock();
        self.modify_segment(n, |s| s.limit = value);
    }

    fn get_seg_limit(&self, n: u32) -> u32 {
        let _g = lock();
        self.segment(n).limit
    }

    fn read_mem_byte(&self, addr: u64) -> u8 {
        let mut b = [0u8; 1];
        self.ram.read(addr, &mut b);
        b[0]
    }

    fn write_mem_byte(&mut self, addr: u64, byte: u8) {
        self.touched.insert(addr & !0xFFF);
        self.ram.write(addr, &[byte]);
    }

    fn read_mem(&self, addr: u64, buf: &mut [u8]) {
        self.ram.read(addr, buf);
    }

    fn write_mem(&mut self, addr: u64, data: &[u8]) {
        for page in (addr & !0xFFF)..=((addr + data.len() as u64) & !0xFFF) {
            self.touched.insert(page & !0xFFF);
        }
        self.ram.write(addr, data);
    }

    /// Only tracks writes made through this API — the guest's own stores are
    /// invisible to it, so this is the "hint" the trait describes.
    fn is_mapped(&self, addr: u64) -> bool {
        self.touched.contains(&(addr & !0xFFF))
    }

    fn step(&mut self) -> StepOutcome {
        let _g = lock();
        self.last_fault = None;
        self.arm_singlestep();

        // KVM can exit for reasons that are not the guest's doing (signals,
        // for instance); retry a bounded number of times.
        for _ in 0..64 {
            let exit = match self.vcpu.run() {
                Ok(exit) => exit,
                Err(e) if e.errno() == libc::EINTR => continue,
                Err(e) => {
                    self.last_fault = None;
                    return StepOutcome::Fault {
                        kind: FaultKind::OtherException,
                        msg: format!("KVM_RUN failed: {e}"),
                    };
                }
            };

            match exit {
                // The single-step trap: the instruction retired.
                VcpuExit::Debug(_) => return StepOutcome::Retired,

                // Either a fault stub (identified by RIP) or the guest's own HLT.
                VcpuExit::Hlt => {
                    let rip = self.regs().rip;
                    if let Some(vector) = hw::stub_vector(rip) {
                        let (vector, error) = self.unwind_fault(vector);
                        self.last_fault = Some((vector, error));
                        return StepOutcome::Fault {
                            kind: hw::classify(vector),
                            msg: self.fault_msg(),
                        };
                    }
                    // A real HLT: nothing can wake this oracle, so treat it as
                    // a retired instruction, like the Bochs backend does.
                    return StepOutcome::Retired;
                }

                // A triple fault means the IDT itself is broken — a bug here,
                // not a property of the instruction.
                VcpuExit::Shutdown => {
                    return StepOutcome::Fault {
                        kind: FaultKind::OtherException,
                        msg: "guest triple fault (KVM_EXIT_SHUTDOWN)".to_string(),
                    }
                }

                // No devices exist, so any MMIO/IO is an access outside RAM.
                VcpuExit::MmioRead(addr, _) | VcpuExit::MmioWrite(addr, _) => {
                    return StepOutcome::Fault {
                        kind: FaultKind::PageFault,
                        msg: format!("access outside guest RAM at {addr:#x}"),
                    }
                }

                other => {
                    return StepOutcome::Fault {
                        kind: FaultKind::OtherException,
                        msg: format!("unexpected KVM exit: {other:?}"),
                    }
                }
            }
        }

        StepOutcome::Fault {
            kind: FaultKind::OtherException,
            msg: "KVM_RUN did not make progress".to_string(),
        }
    }

    fn fault_msg(&self) -> String {
        match self.last_fault {
            None => String::new(),
            Some((v, e)) => hw::fault_message(v, e),
        }
    }
}
