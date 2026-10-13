//! Accessibility support: screen reader notifications, high-contrast
//! detection, and keyboard navigation helpers.
//!
//! Provides:
//! - Screen reader notifications via `NotifyWinEvent`
//! - High-contrast mode detection
//! - Keyboard navigation helpers for focus management
//! - Accessible name/description for UI controls

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Notify a screen reader about an event.
///
/// `event` is the Win32 event constant (e.g., `EVENT_OBJECT_FOCUS`).
/// `hwnd` is the window handle, `object_id` and `child_id` identify
/// the UI element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn notify(event: u32, hwnd: winapi::shared::windef::HWND, object_id: i32, child_id: i32) {
    if !is_enabled() {
        return;
    }
    unsafe {
        winapi::um::winuser::NotifyWinEvent(event, hwnd, object_id, child_id);
    }
}

/// Notify a screen reader that a window has received focus.
pub fn notify_focus(hwnd: winapi::shared::windef::HWND) {
    notify(
        winapi::um::winuser::EVENT_OBJECT_FOCUS,
        hwnd,
        winapi::um::winuser::OBJID_CLIENT,
        winapi::um::winuser::CHILDID_SELF,
    );
}

/// Notify a screen reader that a window has been shown.
pub fn notify_show(hwnd: winapi::shared::windef::HWND) {
    notify(
        winapi::um::winuser::EVENT_OBJECT_SHOW,
        hwnd,
        winapi::um::winuser::OBJID_CLIENT,
        winapi::um::winuser::CHILDID_SELF,
    );
}

/// Notify a screen reader that text has been updated.
pub fn notify_text_changed(hwnd: winapi::shared::windef::HWND) {
    notify(
        winapi::um::winuser::EVENT_OBJECT_VALUECHANGE,
        hwnd,
        winapi::um::winuser::OBJID_CLIENT,
        winapi::um::winuser::CHILDID_SELF,
    );
}

/// Notify a screen reader that a name has changed.
pub fn notify_name_changed(hwnd: winapi::shared::windef::HWND) {
    notify(
        winapi::um::winuser::EVENT_OBJECT_NAMECHANGE,
        hwnd,
        winapi::um::winuser::OBJID_CLIENT,
        winapi::um::winuser::CHILDID_SELF,
    );
}

/// Set the accessible name for a window.
///
/// This sets the window's text, which screen readers will announce.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn set_accessible_name(hwnd: winapi::shared::windef::HWND, name: &str) {
    unsafe {
        let wide = widestring::Hstring::from(name);
        winapi::um::winuser::SetWindowTextW(hwnd, wide.as_ptr());
    }
}

/// Get the accessible name (window text) for a window.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn get_accessible_name(hwnd: winapi::shared::windef::HWND) -> String {
    unsafe {
        let len = winapi::um::winuser::GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        winapi::um::winuser::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        OsString::from_wide(&buf[..len])
            .to_string_lossy()
            .into_owned()
    }
}

/// Move focus to the next control in tab order.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn focus_next(hwnd: winapi::shared::windef::HWND) {
    unsafe {
        winapi::um::winuser::PostMessageW(
            hwnd,
            winapi::um::winuser::WM_KEYDOWN,
            winapi::um::winuser::VK_TAB as usize,
            0,
        );
    }
}

/// Move focus to the previous control in tab order.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn focus_prev(hwnd: winapi::shared::windef::HWND) {
    unsafe {
        winapi::um::winuser::PostMessageW(
            hwnd,
            winapi::um::winuser::WM_KEYDOWN,
            winapi::um::winuser::VK_TAB as usize,
            winapi::um::winuser::KF_EXTENDED as isize,
        );
    }
}

/// Check if a key is a navigation key.
pub fn is_navigation_key(vk: u32) -> bool {
    let vk = vk as i32;
    matches!(
        vk,
        winapi::um::winuser::VK_TAB
            | winapi::um::winuser::VK_LEFT
            | winapi::um::winuser::VK_RIGHT
            | winapi::um::winuser::VK_UP
            | winapi::um::winuser::VK_DOWN
            | winapi::um::winuser::VK_HOME
            | winapi::um::winuser::VK_END
            | winapi::um::winuser::VK_PRIOR
            | winapi::um::winuser::VK_NEXT
    )
}

/// Check if a key is an action key.
pub fn is_action_key(vk: u32) -> bool {
    let vk = vk as i32;
    matches!(
        vk,
        winapi::um::winuser::VK_RETURN
            | winapi::um::winuser::VK_SPACE
            | winapi::um::winuser::VK_ESCAPE
    )
}

// Minimal UTF-16 string helper.
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
    fn accessibility_can_be_initialized() {
        init();
        assert!(is_enabled());
    }

    #[test]
    fn high_contrast_detection_returns_bool() {
        init();
        // Just verify it doesn't panic and returns a bool.
        let _ = is_high_contrast();
    }

    #[test]
    fn navigation_keys_are_recognized() {
        assert!(is_navigation_key(winapi::um::winuser::VK_TAB as u32));
        assert!(is_navigation_key(winapi::um::winuser::VK_LEFT as u32));
        assert!(is_navigation_key(winapi::um::winuser::VK_RIGHT as u32));
        assert!(is_navigation_key(winapi::um::winuser::VK_UP as u32));
        assert!(is_navigation_key(winapi::um::winuser::VK_DOWN as u32));
        assert!(!is_navigation_key(winapi::um::winuser::VK_RETURN as u32));
        assert!(!is_navigation_key(winapi::um::winuser::VK_SPACE as u32));
    }

    #[test]
    fn action_keys_are_recognized() {
        assert!(is_action_key(winapi::um::winuser::VK_RETURN as u32));
        assert!(is_action_key(winapi::um::winuser::VK_SPACE as u32));
        assert!(is_action_key(winapi::um::winuser::VK_ESCAPE as u32));
        assert!(!is_action_key(winapi::um::winuser::VK_TAB as u32));
        assert!(!is_action_key(winapi::um::winuser::VK_LEFT as u32));
    }

    #[test]
    fn notify_does_not_panic_when_disabled() {
        // Reset to disabled state for this test.
        ENABLED.store(false, Ordering::Relaxed);
        // These should be no-ops when disabled.
        notify_focus(std::ptr::null_mut());
        notify_show(std::ptr::null_mut());
        notify_text_changed(std::ptr::null_mut());
        notify_name_changed(std::ptr::null_mut());
        // Re-enable for other tests.
        init();
    }
}
