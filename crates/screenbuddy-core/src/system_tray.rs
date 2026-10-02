//! System tray icon and context menu.
//!
//! The tray itself is a Win32 shell icon, so the icon/window plumbing only
//! exists on Windows. The state and event plumbing above it is portable, so
//! callers get a working (if inert) tray on other platforms rather than a build
//! failure — `hwnd()` is `None` and `start()` reports that no tray is available.

use std::sync::mpsc::{self, channel, Sender};

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ShowCreature,
    HideCreature,
    OpenChat,
    NextCreature,
    OpenSettings,
    Quit,
    About,
}

/// Handle to the native tray window, when the platform has one.
#[cfg(windows)]
pub type TrayHwnd = winapi::shared::windef::HWND;

/// No native tray window exists off Windows.
#[cfg(not(windows))]
pub type TrayHwnd = ();

pub struct SystemTray {
    event_tx: Sender<TrayEvent>,
    hwnd: Option<TrayHwnd>,
    visible: bool,
    creature_visible: bool,
    chat_open: bool,
}

impl Default for SystemTray {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemTray {
    pub fn new() -> Self {
        let (event_tx, _) = channel();
        Self {
            event_tx,
            hwnd: None,
            visible: false,
            creature_visible: true,
            chat_open: false,
        }
    }

    /// Create the tray icon and its message loop.
    ///
    /// On non-Windows platforms there is no tray to create, so this reports the
    /// feature as unavailable instead of pretending to succeed.
    #[cfg(windows)]
    pub fn start(&mut self) -> Result<(), String> {
        let (init_tx, init_rx) = mpsc::channel::<isize>();
        let _event_tx = self.event_tx.clone();

        std::thread::spawn(move || unsafe {
            let h_instance = winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null_mut());
            if h_instance.is_null() {
                return;
            }

            let class_name: Vec<u16> = "ScreenBuddyTray\0".encode_utf16().collect();
            let wc = winapi::um::winuser::WNDCLASSEXW {
                cbSize: std::mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(tray_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: std::ptr::null_mut(),
                hCursor: winapi::um::winuser::LoadCursorW(
                    std::ptr::null_mut(),
                    winapi::um::winuser::IDC_ARROW,
                ),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: std::ptr::null_mut(),
            };
            winapi::um::winuser::RegisterClassExW(&wc);

            let hwnd = winapi::um::winuser::CreateWindowExW(
                winapi::um::winuser::WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                class_name.as_ptr(),
                winapi::um::winuser::WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                h_instance,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                return;
            }

            init_tx.send(hwnd as isize).ok();

            let mut msg: winapi::um::winuser::MSG = std::mem::zeroed();
            while winapi::um::winuser::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                winapi::um::winuser::TranslateMessage(&msg);
                winapi::um::winuser::DispatchMessageW(&msg);
            }
        });

        match init_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(hwnd) => {
                self.hwnd = Some(hwnd as TrayHwnd);
                self.visible = true;
                Ok(())
            }
            Err(_) => Err("Timeout waiting for tray window".into()),
        }
    }

    #[cfg(not(windows))]
    pub fn start(&mut self) -> Result<(), String> {
        Err("system tray is only available on Windows".into())
    }

    pub fn send_event(&self, event: TrayEvent) {
        let _ = self.event_tx.send(event);
    }
    pub fn event_sender(&self) -> Sender<TrayEvent> {
        self.event_tx.clone()
    }
    pub fn set_creature_visible(&mut self, v: bool) {
        self.creature_visible = v;
    }
    pub fn is_creature_visible(&self) -> bool {
        self.creature_visible
    }
    pub fn set_chat_open(&mut self, v: bool) {
        self.chat_open = v;
    }
    pub fn is_chat_open(&self) -> bool {
        self.chat_open
    }
    pub fn hwnd(&self) -> Option<TrayHwnd> {
        self.hwnd
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn poll_event(&self) -> Option<TrayEvent> {
        None
    }
}

#[cfg(windows)]
const WM_APP: u32 = 0x8000;

#[cfg(windows)]
unsafe extern "system" fn tray_window_proc(
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
        m if m == WM_APP => {
            if lparam as u32 == winapi::um::winuser::WM_RBUTTONUP {
                show_context_menu(hwnd);
            }
            0
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
unsafe fn show_context_menu(hwnd: winapi::shared::windef::HWND) {
    let h_menu = winapi::um::winuser::CreatePopupMenu();
    let items = [
        (1001, "Show/Hide Creature"),
        (1002, "Open Chat"),
        (1003, "Next Creature"),
        (1004, "Settings"),
        (1005, "About"),
        (1006, "Quit"),
    ];
    let separators_after = [2, 5];
    for (i, (id, text)) in items.iter().enumerate() {
        if separators_after.contains(&i) {
            winapi::um::winuser::AppendMenuW(
                h_menu,
                winapi::um::winuser::MF_SEPARATOR,
                0,
                std::ptr::null(),
            );
        }
        let text_u16: Vec<u16> = format!("{}\0", text).encode_utf16().collect();
        winapi::um::winuser::AppendMenuW(
            h_menu,
            winapi::um::winuser::MF_STRING,
            *id as usize,
            text_u16.as_ptr(),
        );
    }

    let mut point: winapi::shared::windef::POINT = std::mem::zeroed();
    winapi::um::winuser::GetCursorPos(&mut point);
    winapi::um::winuser::SetForegroundWindow(hwnd);
    let _cmd = winapi::um::winuser::TrackPopupMenu(
        h_menu,
        winapi::um::winuser::TPM_RIGHTALIGN
            | winapi::um::winuser::TPM_BOTTOMALIGN
            | winapi::um::winuser::TPM_RETURNCMD,
        point.x,
        point.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    winapi::um::winuser::DestroyMenu(h_menu);
    winapi::um::winuser::PostMessageW(hwnd, winapi::um::winuser::WM_NULL, 0, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_creation() {
        let tray = SystemTray::new();
        assert!(tray.is_creature_visible());
        assert!(!tray.is_chat_open());
        assert!(!tray.is_visible());
    }

    #[test]
    fn test_tray_states() {
        let mut tray = SystemTray::new();
        tray.set_creature_visible(false);
        assert!(!tray.is_creature_visible());
        tray.set_chat_open(true);
        assert!(tray.is_chat_open());
    }

    #[test]
    fn test_tray_event_sender() {
        let tray = SystemTray::new();
        let sender = tray.event_sender();
        sender.send(TrayEvent::ShowCreature).ok();
    }

    /// Off Windows there is no tray to start, and it must say so rather than
    /// reporting success it did not achieve.
    #[cfg(not(windows))]
    #[test]
    fn test_start_reports_unavailable_off_windows() {
        let mut tray = SystemTray::new();
        assert!(tray.start().is_err());
        assert!(!tray.is_visible());
    }
}
