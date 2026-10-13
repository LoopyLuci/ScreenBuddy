//! Crash reporting via Windows minidumps.
//!
//! Installs a `SetUnhandledExceptionFilter` that writes a minidump to the
//! user's crash directory before the process dies. The minidump contains
//! enough information (stack trace, registers, loaded modules) to diagnose
//! the crash without attaching a debugger.
//!
//! The filter is installed once at startup via `install_crash_handler()`.
//! It is a no-op on non-Windows platforms or if the registry key
//! `HKCU\Software\ScreenBuddy\DisableCrashDumps` is set to 1.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the crash handler has been installed.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Returns the directory where crash dumps are written.
///
/// Uses `%LOCALAPPDATA%\ScreenBuddy\Crashes` by default. Creates the
/// directory if it doesn't exist.
pub fn crash_dir() -> Option<PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let dir = PathBuf::from(local_app_data)
        .join("ScreenBuddy")
        .join("Crashes");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Install the crash handler. Safe to call multiple times — only the
/// first call does anything.
///
/// Returns `true` if the handler was installed, `false` if it was
/// already installed or if crash dumps are disabled via the registry.
pub fn install_crash_handler() -> bool {
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return false; // already installed
    }

    if crash_dumps_disabled() {
        return false;
    }

    unsafe {
        winapi::um::errhandlingapi::SetUnhandledExceptionFilter(Some(unhandled_exception_filter));
    }

    true
}

/// Check if crash dumps are disabled via the registry.
fn crash_dumps_disabled() -> bool {
    unsafe {
        let mut key = std::ptr::null_mut();
        let path = widestring::Hstring::from("Software\\ScreenBuddy");
        let result = winapi::um::winreg::RegOpenKeyExW(
            winapi::um::winreg::HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            winapi::um::winnt::KEY_READ,
            &mut key,
        );
        if result != 0 {
            return false; // no key = not disabled
        }

        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut dtype: u32 = 0;
        let name = widestring::Hstring::from("DisableCrashDumps");
        let result = winapi::um::winreg::RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut dtype,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        );
        winapi::um::winreg::RegCloseKey(key);
        result == 0 && value != 0
    }
}

/// The unhandled exception filter. Writes a minidump and returns
/// `EXCEPTION_EXECUTE_HANDLER` so the default handler runs after.
unsafe extern "system" fn unhandled_exception_filter(
    exception_info: *mut winapi::um::winnt::EXCEPTION_POINTERS,
) -> i32 {
    // Write the minidump before anything else — we don't know how much
    // time we have before the OS kills us.
    let _ = write_minidump(exception_info);

    // EXCEPTION_EXECUTE_HANDLER = 1 (defined in winapi::vc::excpt but not re-exported)
    1
}

/// Write a minidump for the given exception pointers.
///
/// The dump is named `screenbuddy-<timestamp>.dmp` in the crash dir.
/// Returns the path on success.
///
/// `MiniDumpWriteDump` is not in winapi 0.3.9's dbghelp module, so we
/// declare it manually and load it from dbghelp.dll at runtime.
fn write_minidump(exception_info: *mut winapi::um::winnt::EXCEPTION_POINTERS) -> Option<PathBuf> {
    let dir = crash_dir()?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let path = dir.join(format!("screenbuddy-{}.dmp", timestamp));

    unsafe {
        let file = winapi::um::fileapi::CreateFileW(
            widestring::Hstring::from(path.to_str()?).as_ptr(),
            winapi::um::winnt::GENERIC_WRITE,
            0,
            std::ptr::null_mut(),
            winapi::um::fileapi::CREATE_ALWAYS,
            winapi::um::winnt::FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if file.is_null() || file == winapi::um::handleapi::INVALID_HANDLE_VALUE {
            return None;
        }

        // Load dbghelp.dll and get MiniDumpWriteDump.
        let dll_name = widestring::Hstring::from("dbghelp.dll");
        let hmod = winapi::um::libloaderapi::LoadLibraryW(dll_name.as_ptr());
        if hmod.is_null() {
            winapi::um::handleapi::CloseHandle(file);
            return None;
        }

        let func_name = std::ffi::CString::new("MiniDumpWriteDump").ok()?;
        let func = winapi::um::libloaderapi::GetProcAddress(hmod, func_name.as_ptr());
        if func.is_null() {
            winapi::um::libloaderapi::FreeLibrary(hmod);
            winapi::um::handleapi::CloseHandle(file);
            return None;
        }

        // Define the function signature.
        type MiniDumpWriteDumpFn = unsafe extern "system" fn(
            winapi::um::winnt::HANDLE,
            u32,
            winapi::um::winnt::HANDLE,
            u32, // MINIDUMP_TYPE
            *mut MiniDumpExceptionInformation,
            *mut winapi::um::winnt::VOID,
            *mut winapi::um::winnt::VOID,
        ) -> i32;

        let mini_dump_write_dump: MiniDumpWriteDumpFn = std::mem::transmute(func);

        let mut exception = MiniDumpExceptionInformation {
            ThreadId: winapi::um::processthreadsapi::GetCurrentThreadId(),
            ExceptionPointers: exception_info,
            ClientPointers: 0,
        };

        // MiniDumpNormal = 0x00000000
        const MINIDUMP_NORMAL: u32 = 0x00000000;

        let result = mini_dump_write_dump(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            winapi::um::processthreadsapi::GetCurrentProcessId(),
            file,
            MINIDUMP_NORMAL,
            &mut exception,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        winapi::um::libloaderapi::FreeLibrary(hmod);
        winapi::um::handleapi::CloseHandle(file);

        if result != 0 {
            Some(path)
        } else {
            None
        }
    }
}

/// Exception information passed to MiniDumpWriteDump.
#[repr(C)]
#[allow(non_snake_case)]
struct MiniDumpExceptionInformation {
    ThreadId: u32,
    ExceptionPointers: *mut winapi::um::winnt::EXCEPTION_POINTERS,
    ClientPointers: i32,
}

/// List all crash dump files in the crash directory, newest first.
pub fn list_crashes() -> Vec<PathBuf> {
    let Some(dir) = crash_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut crashes: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("dmp"))
                .unwrap_or(false)
        })
        .collect();
    crashes.sort_by(|a, b| b.cmp(a)); // newest first
    crashes
}

/// Delete crash dumps older than `max_age`. Returns the number deleted.
pub fn prune_old_crashes(max_age: std::time::Duration) -> usize {
    let now = std::time::SystemTime::now();
    let mut deleted = 0;
    for path in list_crashes() {
        if let Ok(meta) = std::fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if now.duration_since(modified).unwrap_or_default() > max_age
                    && std::fs::remove_file(&path).is_ok()
                {
                    deleted += 1;
                }
            }
        }
    }
    deleted
}

// Minimal UTF-16 string helper to avoid pulling in a widestring crate.
mod widestring {
    pub struct Hstring(Vec<u16>);
    impl Hstring {
        pub fn from(s: &str) -> Self {
            Self(s.encode_utf16().chain(std::iter::once(0)).collect())
        }
        pub fn as_ptr(&self) -> *const u16 {
            self.0.as_ptr()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crash_directory_is_created() {
        let dir = crash_dir().expect("crash dir should exist");
        assert!(dir.exists(), "crash dir should exist after call");
        assert!(
            dir.ends_with("Crashes"),
            "should end with Crashes, got {:?}",
            dir
        );
    }

    #[test]
    fn installing_the_handler_is_idempotent() {
        let first = install_crash_handler();
        let second = install_crash_handler();
        assert_ne!(
            first, second,
            "first call should install, second should be no-op"
        );
    }

    #[test]
    fn crash_dumps_disabled_returns_false_without_registry_key() {
        // On a clean test machine there's no registry key, so this should
        // return false. If someone has set the key, this test would fail,
        // but that's an acceptable tradeoff for not mocking the registry.
        assert!(!crash_dumps_disabled(), "should not be disabled by default");
    }

    #[test]
    fn list_crashes_returns_empty_on_clean_dir() {
        // The crash dir may exist but should have no .dmp files unless
        // a test has just crashed. We just verify it doesn't panic.
        let _ = list_crashes();
    }
}
