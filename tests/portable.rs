//! The portable suite, run against every backend compiled into this build.
//! Add a backend here and it inherits all of `tests/common/mod.rs`.

mod common;

#[cfg(feature = "sail")]
suite_for!(sail, x86_oracle::SailOracle::new());

#[cfg(feature = "bochs")]
suite_for!(bochs, x86_oracle::BochsOracle::new());

// Hardware ground truth, when this machine allows it (needs /dev/kvm access).
#[cfg(backend_kvm)]
suite_for!(
    kvm,
    x86_oracle::KvmOracle::new(),
    x86_oracle::KvmOracle::is_available()
);

// The same, on Windows (needs the Windows Hypervisor Platform feature enabled).
#[cfg(backend_whp)]
suite_for!(
    whp,
    x86_oracle::WhpOracle::new(),
    x86_oracle::WhpOracle::is_available()
);

// The runtime-dispatch wrapper must behave exactly like the concrete type.
#[cfg(any(feature = "sail", feature = "bochs", backend_kvm, backend_whp))]
suite_for!(
    any_default,
    x86_oracle::AnyOracle::new(*x86_oracle::Backend::available().first().unwrap()),
    !x86_oracle::Backend::available().is_empty()
);
