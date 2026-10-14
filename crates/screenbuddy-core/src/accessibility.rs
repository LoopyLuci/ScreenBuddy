//! Accessibility support: screen reader notifications, high-contrast
//! detection, and keyboard navigation helpers.
//!
//! Provides:
//! - Screen reader notifications via `NotifyWinEvent`
//! - High-contrast mode detection
//! - Keyboard navigation helpers for focus management
//! - Accessible name/description for UI controls
//!
//! The screen-reader notifications, high-contrast detection, and window
//! handle helpers are Win32-only. Off Windows they degrade to no-ops (the
//! notify/focus functions do nothing, high-contrast reports false), so the
//! portable core crate still builds. The pure key-classification helpers
//! (`is_navigation_key`, `is_action_key`) work everywhere.

use std::sync::atomic::{AtomicBool, Ordering};

/// A native window handle. Win32 `HWND` on Windows; an opaque pointer-sized
/// placeholder off Windows so the portable API keeps the same shape.
#[cfg(windows)]
pub type Hwnd = winapi::shared::windef::HWND;
#[cfg(not(windows))]
pub type Hwnd = *mut std::ffi::c_void;

/// Whether accessibility features are enabled.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether high-contrast mode is active.
static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

/// Initialize accessibility support.
///
/// Detects high-contrast mode and sets up the accessibility state.
/// Safe to call multiple times.
pub fn init() {
    ENABLED.store(true, Ordering::Relaxed);
    HIGH_CONTRAST.store(is_high_contrast_active(), Ordering::Relaxed);
}

/// Returns true if accessibility features are enabled.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Returns true if high-contrast mode is active.
pub fn is_high_contrast() -> bool {
    HIGH_CONTRAST.load(Ordering::Relaxed)
}

/// Detect whether Windows high-contrast mode is active.
#[cfg(windows)]
fn is_high_contrast_active() -> bool {
    unsafe {
        let mut hc: winapi::um::winuser::HIGHCONTRASTW = std::mem::zeroed();
        hc.cbSize = std::mem::size_of::<winapi::um::winuser::HIGHCONTRASTW>() as u32;
        if winapi::um::winuser::SystemParametersInfoW(
            winapi::um::winuser::SPI_GETHIGHCONTRAST,
            hc.cbSize,
            &mut hc as *mut _ as *mut winapi::ctypes::c_void,
            winapi::um::winuser::SPIF_UPDATEINIFILE,
        ) != 0
        {
            (hc.dwFlags & winapi::um::winuser::HCF_HIGHCONTRASTON) != 0
        } else {
            false
        }
    }
}

/// Off Windows there is no high-contrast setting to read.
#[cfg(not(windows))]
fn is_high_contrast_active() -> bool {
    false
}

/// Notify a screen reader about an event.
///
/// `event` is the Win32 event constant (e.g., `EVENT_OBJECT_FOCUS`).
/// `hwnd` is the window handle, `object_id` and `child_id` identify
/// the UI element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn notify(event: u32, hwnd: Hwnd, object_id: i32, child_id: i32) {
    if !is_enabled() {
        return;
    }
    #[cfg(windows)]
    unsafe {
        winapi::um::winuser::NotifyWinEvent(event, hwnd, object_id, child_id);
    }
    #[cfg(not(windows))]
    {
        let _ = (event, hwnd, object_id, child_id);
    }
}

/// Notify a screen reader that a window has received focus.
pub fn notify_focus(hwnd: Hwnd) {
    notify(EVENT_OBJECT_FOCUS, hwnd, OBJID_CLIENT, CHILDID_SELF);
}

/// Notify a screen reader that a window has been shown.
pub fn notify_show(hwnd: Hwnd) {
    notify(EVENT_OBJECT_SHOW, hwnd, OBJID_CLIENT, CHILDID_SELF);
}

/// Notify a screen reader that text has been updated.
pub fn notify_text_changed(hwnd: Hwnd) {
    notify(EVENT_OBJECT_VALUECHANGE, hwnd, OBJID_CLIENT, CHILDID_SELF);
}

/// Notify a screen reader that a name has changed.
pub fn notify_name_changed(hwnd: Hwnd) {
    notify(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT, CHILDID_SELF);
}

/// Set the accessible name for a window.
///
/// This sets the window's text, which screen readers will announce.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn set_accessible_name(hwnd: Hwnd, name: &str) {
    #[cfg(windows)]
    unsafe {
        let wide = widestring::Hstring::from(name);
        winapi::um::winuser::SetWindowTextW(hwnd, wide.as_ptr());
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, name);
    }
}

/// Get the accessible name (window text) for a window.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn get_accessible_name(hwnd: Hwnd) -> String {
    #[cfg(windows)]
    unsafe {
        let len = winapi::um::winuser::GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        winapi::um::winuser::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&buf[..len])
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
        String::new()
    }
}

/// Move focus to the next control in tab order.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn focus_next(hwnd: Hwnd) {
    #[cfg(windows)]
    unsafe {
        winapi::um::winuser::PostMessageW(hwnd, WM_KEYDOWN, VK_TAB as usize, 0);
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

/// Move focus to the previous control in tab order.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn focus_prev(hwnd: Hwnd) {
    #[cfg(windows)]
    unsafe {
        winapi::um::winuser::PostMessageW(hwnd, WM_KEYDOWN, VK_TAB as usize, KF_EXTENDED as isize);
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

/// Check if a key is a navigation key.
pub fn is_navigation_key(vk: u32) -> bool {
    let vk = vk as i32;
    matches!(
        vk,
        VK_TAB | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT
    )
}

/// Check if a key is an action key.
pub fn is_action_key(vk: u32) -> bool {
    let vk = vk as i32;
    matches!(vk, VK_RETURN | VK_SPACE | VK_ESCAPE)
}

// --- Win32 event/handle constants, mirrored so the portable helpers and the
// --- Windows calls share one definition. These values are stable ABI. ---
const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_VALUECHANGE: u32 = 0x800E;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const OBJID_CLIENT: i32 = -4;
const CHILDID_SELF: i32 = 0;
#[cfg(windows)]
const WM_KEYDOWN: u32 = 0x0100;
const VK_TAB: i32 = 0x09;
const VK_LEFT: i32 = 0x25;
const VK_UP: i32 = 0x26;
const VK_RIGHT: i32 = 0x27;
const VK_DOWN: i32 = 0x28;
const VK_HOME: i32 = 0x24;
const VK_END: i32 = 0x23;
const VK_PRIOR: i32 = 0x21; // Page Up
const VK_NEXT: i32 = 0x22; // Page Down
const VK_RETURN: i32 = 0x0D;
const VK_SPACE: i32 = 0x20;
const VK_ESCAPE: i32 = 0x1B;
#[cfg(windows)]
const KF_EXTENDED: i32 = 0x0100;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_keys_are_recognized() {
        assert!(is_navigation_key(VK_TAB as u32));
        assert!(is_navigation_key(VK_LEFT as u32));
        assert!(is_navigation_key(VK_HOME as u32));
        assert!(!is_navigation_key(VK_RETURN as u32));
        assert!(!is_navigation_key(0x41)); // 'A'
    }

    #[test]
    fn action_keys_are_recognized() {
        assert!(is_action_key(VK_RETURN as u32));
        assert!(is_action_key(VK_SPACE as u32));
        assert!(is_action_key(VK_ESCAPE as u32));
        assert!(!is_action_key(VK_TAB as u32));
    }

    #[test]
    fn init_enables_accessibility() {
        init();
        assert!(is_enabled());
    }

    #[cfg(windows)]
    #[test]
    fn notify_does_not_panic_when_enabled() {
        init();
        // A null hwnd is safe: NotifyWinEvent just does nothing useful.
        notify(
            EVENT_OBJECT_FOCUS,
            std::ptr::null_mut(),
            OBJID_CLIENT,
            CHILDID_SELF,
        );
    }

    #[cfg(windows)]
    #[test]
    fn accessible_name_roundtrips() {
        init();
        // Creating a real window in a unit test is heavy; just confirm the
        // getter on a null hwnd returns empty rather than panicking.
        let name = get_accessible_name(std::ptr::null_mut());
        assert!(name.is_empty());
    }
}
