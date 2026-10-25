//! The C API of the Sail and Bochs backends. On Windows MSVC build.rs links both into
//! `x86oracle.dll` (cfg `x86oracle_dll`), loaded by absolute path on first call, so a
//! program that uses the oracle does not carry its own copy of them; elsewhere each is a
//! static library.

#[cfg(x86oracle_dll)]
pub(crate) mod dll {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::sync::OnceLock;

    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    struct Module(*mut c_void);

    // The module handle is process-wide and never unloaded.
    unsafe impl Send for Module {}
    unsafe impl Sync for Module {}

    fn module() -> *mut c_void {
        static MODULE: OnceLock<Module> = OnceLock::new();
        MODULE
            .get_or_init(|| {
                // The DLL's own directory (where build.rs put gmp's DLL) is searched first.
                let path = Path::new(env!("X86ORACLE_DLL"));
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                let h = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH) };
                if h.is_null() {
                    panic!("LoadLibraryExW({}) failed: error {}", path.display(), unsafe { GetLastError() });
                }
                Module(h)
            })
            .0
    }

    /// `name` ends with a NUL.
    pub(crate) fn symbol(name: &str) -> *mut c_void {
        let p = unsafe { GetProcAddress(module(), name.as_ptr()) };
        assert!(!p.is_null(), "{} not exported by {}", name.trim_end_matches('\0'), env!("X86ORACLE_DLL"));
        p
    }
}

/// Declares the C functions as `unsafe fn`s of the same names, called through the
/// DLL's export table under cfg `x86oracle_dll`. build.rs exports every `fn x86oracle_*`
/// named in the enabled backends' sources.
macro_rules! native_api {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)*) => {
        #[cfg(not(x86oracle_dll))]
        extern "C" {
            $(fn $name($($arg: $ty),*) $(-> $ret)?;)*
        }

        #[cfg(x86oracle_dll)]
        struct NativeApi {
            $($name: unsafe extern "C" fn($($ty),*) $(-> $ret)?,)*
        }

        #[cfg(x86oracle_dll)]
        fn native_api() -> &'static NativeApi {
            static API: std::sync::OnceLock<NativeApi> = std::sync::OnceLock::new();
            API.get_or_init(|| NativeApi {
                // SAFETY: the export of that name is the C function declared with this signature.
                $($name: unsafe { std::mem::transmute(crate::native::dll::symbol(concat!(stringify!($name), "\0"))) },)*
            })
        }

        $(
            #[cfg(x86oracle_dll)]
            unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
                (native_api().$name)($($arg),*)
            }
        )*
    };
}
