//! Throughput of each backend, for planning diff campaigns.
//!
//! Ignored by default (it takes a few seconds and the numbers are
//! machine-specific). Run it with:
//!
//! ```sh
//! cargo test --release --features bochs,hardware --test throughput -- --ignored --nocapture
//! ```
//!
//! The step loop re-executes one instruction from a fixed address, which is the
//! best case for an emulator's decode cache — treat the Bochs figure as an
//! upper bound, not a campaign estimate. A hardware backend has the opposite
//! bias: it pays a hypervisor round trip per instruction, so it measures the
//! cost of *observing* one step rather than of executing it.
#![cfg(feature = "bochs")]

use std::time::Instant;
use x86_oracle::*;

fn bench<O: X86Oracle>(mut c: O, n: u32) -> f64 {
    c.set_rip(DEFAULT_CODE_ADDR);
    c.set_gpr(RBX, 1);
    c.write_mem(DEFAULT_CODE_ADDR, &[0x48, 0x01, 0xD8]); // add rax, rbx
    let t = Instant::now();
    for _ in 0..n {
        assert!(c.step().is_retired());
        c.set_rip(DEFAULT_CODE_ADDR);
    }
    let rate = n as f64 / t.elapsed().as_secs_f64();
    // Correctness gate: RAX must have been incremented once per step, or the
    // "throughput" is measuring nothing.
    assert_eq!(c.get_gpr(RAX), n as u64, "[{}] steps did not execute", c.backend_name());
    rate
}

#[test]
#[ignore = "benchmark; run explicitly with --ignored"]
fn steps_per_second() {
    #[cfg(feature = "sail")]
    eprintln!("sail : {:>9.0} steps/s", bench(SailOracle::new(), 20_000));
    eprintln!("bochs: {:>9.0} steps/s", bench(BochsOracle::new(), 20_000));
    #[cfg(backend_kvm)]
    if KvmOracle::is_available() {
        eprintln!("kvm  : {:>9.0} steps/s", bench(KvmOracle::new(), 20_000));
    }
    #[cfg(backend_whp)]
    if WhpOracle::is_available() {
        eprintln!("whp  : {:>9.0} steps/s", bench(WhpOracle::new(), 20_000));
    }
    eprintln!("--- instance creation ---");
    #[cfg(feature = "sail")]
    {
        let t = Instant::now();
        for _ in 0..20 {
            let _ = SailOracle::new();
        }
        eprintln!("sail  new: {:>8.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
    let t = Instant::now();
    for _ in 0..20 {
        let _ = BochsOracle::new();
    }
    eprintln!("bochs new: {:>8.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    #[cfg(backend_kvm)]
    if KvmOracle::is_available() {
        let t = Instant::now();
        for _ in 0..20 {
            let _ = KvmOracle::new();
        }
        eprintln!("kvm   new: {:>8.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
    #[cfg(backend_whp)]
    if WhpOracle::is_available() {
        let t = Instant::now();
        for _ in 0..20 {
            let _ = WhpOracle::new();
        }
        eprintln!("whp   new: {:>8.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
}
