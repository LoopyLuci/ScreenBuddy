//! Settings Window for ScreenBuddy
//!
//! A transparent settings panel for runtime configuration.

use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::{Arc, Mutex};

/// Settings window state
#[allow(dead_code)] // Win32 HWND handle; wired up when the window is shown
pub struct SettingsWindow {
    hwnd: Option<isize>,
    visible: bool,
    settings: Arc<Mutex<HashMap<String, SettingValue>>>,
}

/// A setting value
#[allow(dead_code)] // variants are populated by the settings UI when built out
#[derive(Debug, Clone)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// Setting change event
#[allow(dead_code)] // emitted once the settings UI dispatches changes
#[derive(Debug, Clone)]
pub enum SettingsEvent {
    Changed(String, SettingValue),
    Close,
    Toggle,
    Reset,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            hwnd: None,
            visible: false,
            settings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn set(&self, key: &str, value: SettingValue) {
        self.settings.lock().unwrap().insert(key.to_string(), value);
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        self.settings.lock().unwrap().get(key).cloned()
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn show(&mut self) {
        self.visible = true;
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    #[allow(dead_code)] // public settings API, consumed by the settings UI
    pub fn settings(&self) -> Arc<Mutex<HashMap<String, SettingValue>>> {
        self.settings.clone()
    }
}

/// Create a settings window (Windows-specific)
#[cfg(windows)]
pub fn create_settings_window() -> Result<isize, String> {
    unsafe {
        let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
        let class_name: Vec<u16> = "ScreenBuddySettings\0".encode_utf16().collect();
        let window_title: Vec<u16> = "ScreenBuddy Settings\0".encode_utf16().collect();

        let wc = winapi::um::winuser::WNDCLASSEXW {
            cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
            style: winapi::um::winuser::CS_HREDRAW | winapi::um::winuser::CS_VREDRAW,
            lpfnWndProc: Some(settings_window_proc),
            hInstance: h_instance,
            hCursor: winapi::um::winuser::LoadCursorW(
                ptr::null_mut(),
                winapi::um::winuser::IDC_ARROW,
            ),
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        winapi::um::winuser::RegisterClassExW(&wc);

        let ex_style = winapi::um::winuser::WS_EX_LAYERED
            | winapi::um::winuser::WS_EX_TOPMOST
            | winapi::um::winuser::WS_EX_TOOLWINDOW;
        let hwnd = winapi::um::winuser::CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            window_title.as_ptr(),
            winapi::um::winuser::WS_POPUP | winapi::um::winuser::WS_VISIBLE,
            600,
            150,
            350,
            500,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create settings window".to_string());
        }

        Ok(hwnd as isize)
    }
}

/// Settings window procedure
#[cfg(windows)]
unsafe extern "system" fn settings_window_proc(
    hwnd: winapi::shared::windef::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        winapi::um::winuser::WM_DESTROY => {
            winapi::um::winuser::PostQuitMessage(0);
            0
        }
        winapi::um::winuser::WM_KEYDOWN => {
            if wparam == winapi::um::winuser::VK_ESCAPE as usize {
                winapi::um::winuser::ShowWindow(hwnd, winapi::um::winuser::SW_HIDE);
            }
            0
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_window_new() {
        let sw = SettingsWindow::new();
        assert!(!sw.is_visible());
    }

    #[test]
    fn test_settings_window_set_get() {
        let sw = SettingsWindow::new();
        sw.set("volume", SettingValue::Float(0.8));
        match sw.get("volume") {
            Some(SettingValue::Float(v)) => assert_eq!(v, 0.8),
            _ => panic!("Expected Float"),
        }
    }

    #[test]
    fn test_settings_window_toggle() {
        let mut sw = SettingsWindow::new();
        assert!(!sw.is_visible());
        sw.toggle();
        assert!(sw.is_visible());
    }

    #[test]
    fn test_settings_window_show_hide() {
        let mut sw = SettingsWindow::new();
        sw.show();
        assert!(sw.is_visible());
        sw.hide();
        assert!(!sw.is_visible());
    }

    #[test]
    fn test_settings_window_missing() {
        let sw = SettingsWindow::new();
        assert!(sw.get("nonexistent").is_none());
    }
}
