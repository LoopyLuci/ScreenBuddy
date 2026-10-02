use std::mem;
use std::ptr;
use std::sync::mpsc::{self, channel, Sender};
use std::thread;

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

pub struct SystemTray {
    event_tx: Sender<TrayEvent>,
    hwnd: Option<winapi::shared::windef::HWND>,
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

    pub fn start(&mut self) -> Result<(), String> {
        let (init_tx, init_rx) = mpsc::channel::<isize>();
        let _event_tx = self.event_tx.clone();

        thread::spawn(move || unsafe {
            let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
            if h_instance.is_null() {
                return;
            }

            let class_name: Vec<u16> = "ScreenBuddyTray\0".encode_utf16().collect();
            let wc = winapi::um::winuser::WNDCLASSEXW {
                cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(tray_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: ptr::null_mut(),
                hCursor: winapi::um::winuser::LoadCursorW(
                    ptr::null_mut(),
                    winapi::um::winuser::IDC_ARROW,
                ),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
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
                ptr::null_mut(),
                ptr::null_mut(),
                h_instance,
                ptr::null_mut(),
            );
            if hwnd.is_null() {
                return;
            }

            init_tx.send(hwnd as isize).ok();

            let mut msg: winapi::um::winuser::MSG = mem::zeroed();
            while winapi::um::winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                winapi::um::winuser::TranslateMessage(&msg);
                winapi::um::winuser::DispatchMessageW(&msg);
            }
        });

        match init_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(hwnd) => {
                self.hwnd = Some(hwnd as winapi::shared::windef::HWND);
                self.visible = true;
                Ok(())
            }
            Err(_) => Err("Timeout waiting for tray window".into()),
        }
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
    pub fn hwnd(&self) -> Option<winapi::shared::windef::HWND> {
        self.hwnd
    }

    pub fn poll_event(&self) -> Option<TrayEvent> {
        None
    }
}

const WM_APP: u32 = 0x8000;

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
                ptr::null(),
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

    let mut point: winapi::shared::windef::POINT = mem::zeroed();
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
        ptr::null(),
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
}
