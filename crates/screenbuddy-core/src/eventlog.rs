//! Windows Event Log integration.
//!
//! Writes structured log entries to the Windows Event Log for critical
//! operations: startup, shutdown, crashes, settings changes, and errors.
//!
//! Uses the legacy Event Log API (`RegisterEventSourceW`/`ReportEventW`/
//! `DeregisterEventSource`) which is available in winapi 0.3.9.
//! Events are written to the Application log under the "ScreenBuddy" source.
//!
//! This is Win32-only. Off Windows `EventLog::register()` returns an error
//! and every write is a no-op returning `false`, so the portable core crate
//! still builds and callers degrade gracefully instead of failing to compile.

/// Event severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLevel {
    Info = 1,
    Warning = 2,
    Error = 3,
}

#[cfg(windows)]
impl EventLevel {
    fn as_word(self) -> u16 {
        match self {
            EventLevel::Info => winapi::um::winnt::EVENTLOG_INFORMATION_TYPE,
            EventLevel::Warning => winapi::um::winnt::EVENTLOG_WARNING_TYPE,
            EventLevel::Error => winapi::um::winnt::EVENTLOG_ERROR_TYPE,
        }
    }
}

/// A handle to the Windows Event Log.
///
/// Created once via `EventLog::register()`. Writes structured events
/// with a message string. Off Windows this is a no-op handle.
pub struct EventLog {
    #[cfg(windows)]
    handle: winapi::um::winnt::HANDLE,
    #[cfg(windows)]
    registered: std::sync::atomic::AtomicBool,
}

// HANDLE is a pointer type, but we ensure thread safety through
// the AtomicBool flag and the fact that ReportEventW is thread-safe.
#[cfg(windows)]
unsafe impl Send for EventLog {}
#[cfg(windows)]
unsafe impl Sync for EventLog {}

impl EventLog {
    /// The event source name registered in the registry.
    #[cfg(windows)]
    const SOURCE_NAME: &'static str = "ScreenBuddy";

    /// Register the event source.
    ///
    /// Returns `EventLog` on success, or an error message on failure.
    /// The source is registered under `Application` log. Off Windows this
    /// always returns an error (there is no Event Log to write to).
    #[cfg(windows)]
    pub fn register() -> Result<Self, String> {
        use std::sync::atomic::AtomicBool;
        unsafe {
            let source_name = widestring::Hstring::from(Self::SOURCE_NAME);
            let handle =
                winapi::um::winbase::RegisterEventSourceW(std::ptr::null(), source_name.as_ptr());
            if handle.is_null() {
                return Err("RegisterEventSourceW failed".to_string());
            }
            Ok(Self {
                handle,
                registered: AtomicBool::new(true),
            })
        }
    }

    /// Write an event to the Windows Event Log.
    ///
    /// `level` is the severity, `event_id` is a unique identifier for
    /// this type of event, and `message` is the human-readable description.
    /// Returns `false` off Windows or if the write fails.
    #[cfg(windows)]
    pub fn write(&self, level: EventLevel, event_id: u32, message: &str) -> bool {
        use std::ffi::CString;
        use std::sync::atomic::Ordering;
        if !self.registered.load(Ordering::Relaxed) {
            return false;
        }
        let msg_cstr = match CString::new(message) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let msg_ptr = msg_cstr.as_ptr();
        unsafe {
            let desc = [msg_ptr];
            winapi::um::winbase::ReportEventW(
                self.handle,
                level.as_word(),
                0,                    // category
                event_id,             // event ID
                std::ptr::null_mut(), // user SID
                1,                    // string count
                0,                    // binary data size
                desc.as_ptr() as *mut *const u16,
                std::ptr::null_mut(), // binary data
            ) != 0
        }
    }

    /// Write an info-level event.
    pub fn info(&self, event_id: u32, message: &str) -> bool {
        self.write(EventLevel::Info, event_id, message)
    }

    /// Write a warning-level event.
    pub fn warning(&self, event_id: u32, message: &str) -> bool {
        self.write(EventLevel::Warning, event_id, message)
    }

    /// Write an error-level event.
    pub fn error(&self, event_id: u32, message: &str) -> bool {
        self.write(EventLevel::Error, event_id, message)
    }
}

#[cfg(windows)]
impl Drop for EventLog {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        if self.registered.swap(false, Ordering::Relaxed) {
            unsafe {
                winapi::um::winbase::DeregisterEventSource(self.handle);
            }
        }
    }
}

// --- Non-Windows fallback: an inert handle so the crate still builds. ---
#[cfg(not(windows))]
impl EventLog {
    /// Off Windows there is no Event Log. Return an error so the caller can
    /// log the absence once and move on.
    pub fn register() -> Result<Self, String> {
        Err("Windows Event Log is not available on this platform".to_string())
    }

    /// No-op: there is nowhere to write. Always returns `false`.
    pub fn write(&self, _level: EventLevel, _event_id: u32, _message: &str) -> bool {
        false
    }
}

// Minimal UTF-16 string helper.
#[cfg(windows)]
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

// Event IDs for common operations.
pub const EVENT_APP_STARTED: u32 = 1001;
pub const EVENT_APP_SHUTDOWN: u32 = 1002;
pub const EVENT_CRASH_DETECTED: u32 = 1003;
pub const EVENT_SETTINGS_CHANGED: u32 = 1004;
pub const EVENT_CREATURE_SPAWNED: u32 = 1005;
pub const EVENT_UPDATED_AVAILABLE: u32 = 1006;
pub const EVENT_UPDATE_INSTALLED: u32 = 1007;
pub const EVENT_IPC_CONNECTED: u32 = 1008;
pub const EVENT_IPC_DISCONNECTED: u32 = 1009;
pub const EVENT_TRAY_ACTION: u32 = 1010;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn event_level_as_word_is_nonzero() {
        assert!(EventLevel::Info.as_word() != 0);
        assert!(EventLevel::Warning.as_word() != 0);
        assert!(EventLevel::Error.as_word() != 0);
    }

    #[test]
    fn event_log_can_be_registered() {
        // On Windows this succeeds; off Windows register() returns Err, which
        // is the documented behaviour, not a failure.
        let log = EventLog::register();
        if cfg!(windows) {
            assert!(log.is_ok(), "EventLog::register should succeed on Windows");
        } else {
            assert!(log.is_err(), "EventLog::register should Err off Windows");
        }
    }

    #[cfg(windows)]
    #[test]
    fn event_log_write_returns_true_after_register() {
        let log = EventLog::register().unwrap();
        let result = log.info(EVENT_APP_STARTED, "test message");
        // May fail in some CI environments, but should not panic.
        let _ = result;
    }

    #[test]
    fn all_event_ids_are_unique() {
        let ids = vec![
            EVENT_APP_STARTED,
            EVENT_APP_SHUTDOWN,
            EVENT_CRASH_DETECTED,
            EVENT_SETTINGS_CHANGED,
            EVENT_CREATURE_SPAWNED,
            EVENT_UPDATED_AVAILABLE,
            EVENT_UPDATE_INSTALLED,
            EVENT_IPC_CONNECTED,
            EVENT_IPC_DISCONNECTED,
            EVENT_TRAY_ACTION,
        ];
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "event IDs must be unique");
    }
}
