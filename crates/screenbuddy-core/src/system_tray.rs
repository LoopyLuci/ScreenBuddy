//! System tray icon and context menu.
//!
//! The tray itself is a Win32 shell icon, so the icon/window plumbing only
//! exists on Windows. The state and event plumbing above it is portable, so
//! callers get a working (if inert) tray on other platforms rather than a build
//! failure — `hwnd()` is `None` and `start()` reports that no tray is available.

use std::sync::mpsc::{self, channel, Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Bring the main window back, creating it if it was closed.
    OpenWindow,
    /// Hide the main window, leaving only the tray icon.
    MinimizeToTray,
    ShowCreature,
    HideCreature,
    OpenChat,
    NextCreature,
    OpenSettings,
    /// Open the agent editor.
    OpenAgents,
    Quit,
    About,
}

/// Message the shell posts to the tray window when the icon is clicked.
#[cfg(windows)]
const WM_TRAY_CALLBACK: u32 = 0x8000 + 1;

/// Menu command ids. Grouped so the window procedure and the menu builder cannot
/// drift apart; an unmapped id would be a silently dead menu item.
#[cfg(windows)]
mod cmd {
    pub const OPEN_WINDOW: u32 = 1001;
    pub const MINIMIZE: u32 = 1002;
    pub const SHOW_CREATURE: u32 = 1003;
    pub const HIDE_CREATURE: u32 = 1004;
    pub const OPEN_AGENTS: u32 = 1010;
    pub const OPEN_CHAT: u32 = 1005;
    pub const NEXT_CREATURE: u32 = 1006;
    pub const OPEN_SETTINGS: u32 = 1007;
    pub const ABOUT: u32 = 1008;
    pub const QUIT: u32 = 1009;
}

/// Map a menu id onto the event it means.
#[cfg(windows)]
fn event_for_command(command: u32) -> Option<TrayEvent> {
    Some(match command {
        cmd::OPEN_WINDOW => TrayEvent::OpenWindow,
        cmd::MINIMIZE => TrayEvent::MinimizeToTray,
        cmd::SHOW_CREATURE => TrayEvent::ShowCreature,
        cmd::HIDE_CREATURE => TrayEvent::HideCreature,
        cmd::OPEN_CHAT => TrayEvent::OpenChat,
        cmd::NEXT_CREATURE => TrayEvent::NextCreature,
        cmd::OPEN_AGENTS => TrayEvent::OpenAgents,
        cmd::OPEN_SETTINGS => TrayEvent::OpenSettings,
        cmd::ABOUT => TrayEvent::About,
        cmd::QUIT => TrayEvent::Quit,
        _ => return None,
    })
}

/// Handle to the native tray window, when the platform has one.
#[cfg(windows)]
pub type TrayHwnd = winapi::shared::windef::HWND;

/// No native tray window exists off Windows.
#[cfg(not(windows))]
pub type TrayHwnd = ();

pub struct SystemTray {
    event_tx: Sender<TrayEvent>,
    event_rx: Receiver<TrayEvent>,
    hwnd: Option<TrayHwnd>,
    /// Whether the shell actually accepted the notification icon.
    ///
    /// `start()` returning Ok and the icon existing are the same thing today,
    /// but keeping them separate means the "minimise to tray" decision stays
    /// correct if the shell ever refuses the icon -- hiding the window with no
    /// tray to restore from would strand the app.
    installed: std::sync::atomic::AtomicBool,
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
        let (event_tx, event_rx) = channel();
        Self {
            event_tx,
            event_rx,
            hwnd: None,
            installed: std::sync::atomic::AtomicBool::new(false),
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

            // Register with the shell. Without this there is only a hidden
            // message window and no icon in the notification area at all.
            add_icon(hwnd, h_instance);

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
                self.installed
                    .store(true, std::sync::atomic::Ordering::SeqCst);
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

    /// Raise an event as if the matching menu item had been clicked.
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

    /// Take the next menu command, if the user chose one.
    ///
    /// The tray window runs on its own thread, so this is how a menu click
    /// reaches the render loop. It was returning None unconditionally, which
    /// meant every tray menu item did nothing.
    /// Remove the tray icon. Called on drop and on explicit shutdown.
    pub fn shutdown(&self) {
        #[cfg(windows)]
        if self.installed.load(std::sync::atomic::Ordering::SeqCst) {
            if let Some(hwnd) = self.hwnd {
                unsafe { remove_icon(hwnd) };
                self.installed
                    .store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    pub fn poll_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// True when a tray icon is really present in the notification area.
    pub fn is_installed(&self) -> bool {
        self.installed.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether the main window is currently showing.
    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }
}

/// Stable id for the shell icon. Any non-zero value works; the window handle is
/// not used so the id survives the tray window being recreated.
#[cfg(windows)]
const ICON_ID: u32 = 1;

impl Drop for SystemTray {
    fn drop(&mut self) {
        // A quit that leaves the icon behind looks like a running app the user
        // cannot get rid of.
        self.shutdown();
    }
}

/// Add the notification icon for the tray window.
#[cfg(windows)]
unsafe fn add_icon(
    hwnd: winapi::shared::windef::HWND,
    instance: winapi::shared::minwindef::HINSTANCE,
) {
    use std::mem;
    use std::ptr;
    use winapi::um::shellapi::*;

    let icon = winapi::um::winuser::LoadIconW(instance, winapi::um::winuser::MAKEINTRESOURCEW(1));
    let fallback =
        winapi::um::winuser::LoadIconW(ptr::null_mut(), winapi::um::winuser::IDI_APPLICATION);

    let mut data: NOTIFYICONDATAW = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        // NIF_TIP for the hover text, NIF_MESSAGE so clicks reach us.
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: if icon.is_null() { fallback } else { icon },
        ..mem::zeroed()
    };
    let tip: Vec<u16> = "ScreenBuddy ".encode_utf16().collect();
    for (slot, ch) in data.szTip.iter_mut().zip(tip.iter()) {
        *slot = *ch;
    }
    Shell_NotifyIconW(NIM_ADD, &mut data as *mut _);
}

/// Remove the notification icon, so quitting does not leave a ghost behind.
#[cfg(windows)]
unsafe fn remove_icon(hwnd: winapi::shared::windef::HWND) {
    use std::mem;
    use winapi::um::shellapi::*;

    let mut data: NOTIFYICONDATAW = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..mem::zeroed()
    };
    Shell_NotifyIconW(NIM_DELETE, &mut data as *mut _);
}

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
        WM_TRAY_CALLBACK => {
            match lparam as u32 {
                // Left click brings the window back, which is the whole point of
                // having a tray icon.
                winapi::um::winuser::WM_LBUTTONUP | winapi::um::winuser::WM_LBUTTONDBLCLK => {
                    deliver(TrayEvent::OpenWindow);
                }
                winapi::um::winuser::WM_RBUTTONUP | winapi::um::winuser::WM_CONTEXTMENU => {
                    // The chosen id used to be discarded, so every menu item was
                    // a no-op. Route it to the render loop now.
                    let chosen = show_context_menu(hwnd);
                    if let Some(event) = event_for_command(chosen) {
                        deliver(event);
                    }
                }
                _ => {}
            }
            0
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Hand an event to the render loop.
///
/// The tray window runs on its own thread, so there is no `self` here. Events go
/// through a process-wide slot that the loop drains on its next frame.
#[cfg(windows)]
fn deliver(event: TrayEvent) {
    if let Ok(mut slot) = PENDING.lock() {
        slot.push(event);
    }
}

/// Events raised from the tray thread, drained by the render loop.
#[cfg(windows)]
static PENDING: std::sync::Mutex<Vec<TrayEvent>> = std::sync::Mutex::new(Vec::new());

/// Take every tray event raised since the last call.
pub fn take_pending_events() -> Vec<TrayEvent> {
    #[cfg(windows)]
    {
        match PENDING.lock() {
            Ok(mut slot) => std::mem::take(&mut *slot),
            Err(_) => Vec::new(),
        }
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
/// Build the context menu and return the id the user chose (0 if cancelled).
unsafe fn show_context_menu(hwnd: winapi::shared::windef::HWND) -> u32 {
    let h_menu = winapi::um::winuser::CreatePopupMenu();
    if h_menu.is_null() {
        return 0;
    }

    let items = [
        (cmd::OPEN_WINDOW, "Open ScreenBuddy"),
        (cmd::MINIMIZE, "Minimise to tray"),
    ];
    let tail = [
        (cmd::SHOW_CREATURE, "Show creature"),
        (cmd::HIDE_CREATURE, "Hide creature"),
        (cmd::OPEN_AGENTS, "Agents"),
        (cmd::OPEN_CHAT, "Open chat"),
        (cmd::NEXT_CREATURE, "Next creature"),
        (cmd::OPEN_SETTINGS, "Settings"),
    ];
    let end = [
        (cmd::ABOUT, "About ScreenBuddy"),
        (cmd::QUIT, "Quit ScreenBuddy"),
    ];

    let push = |id: u32, text: &str| {
        let text_u16: Vec<u16> = format!("{text}\0").encode_utf16().collect();
        winapi::um::winuser::AppendMenuW(
            h_menu,
            winapi::um::winuser::MF_STRING,
            id as usize,
            text_u16.as_ptr(),
        );
    };
    let separator = || {
        winapi::um::winuser::AppendMenuW(
            h_menu,
            winapi::um::winuser::MF_SEPARATOR,
            0,
            std::ptr::null(),
        );
    };

    for (id, text) in items {
        push(id, text);
    }
    separator();
    for (id, text) in tail {
        push(id, text);
    }
    separator();
    for (id, text) in end {
        push(id, text);
    }

    let mut point: winapi::shared::windef::POINT = std::mem::zeroed();
    winapi::um::winuser::GetCursorPos(&mut point);
    winapi::um::winuser::SetForegroundWindow(hwnd);
    let chosen = winapi::um::winuser::TrackPopupMenu(
        h_menu,
        winapi::um::winuser::TPM_RIGHTALIGN
            | winapi::um::winuser::TPM_BOTTOMALIGN
            | winapi::um::winuser::TPM_RETURNCMD
            | winapi::um::winuser::TPM_NONOTIFY,
        point.x,
        point.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    winapi::um::winuser::DestroyMenu(h_menu);
    winapi::um::winuser::PostMessageW(hwnd, winapi::um::winuser::WM_NULL, 0, 0);
    // With TPM_RETURNCMD the chosen id comes back as the return value. A
    // negative result means the menu was cancelled.
    if chosen <= 0 {
        return 0;
    }
    chosen as u32
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

    /// The menu id to event mapping is what makes a tray item do anything; an
    /// unmapped id is a silently dead menu entry.
    #[cfg(windows)]
    #[test]
    fn every_menu_command_maps_to_an_event() {
        for command in [
            cmd::OPEN_WINDOW,
            cmd::MINIMIZE,
            cmd::SHOW_CREATURE,
            cmd::HIDE_CREATURE,
            cmd::OPEN_CHAT,
            cmd::NEXT_CREATURE,
            cmd::OPEN_SETTINGS,
            cmd::ABOUT,
            cmd::QUIT,
        ] {
            assert!(
                event_for_command(command).is_some(),
                "menu command {command} has no event"
            );
        }
        assert!(event_for_command(4242).is_none(), "unknown id ignored");
    }

    #[cfg(windows)]
    #[test]
    fn open_and_minimize_are_distinct_commands() {
        // Conflating these is what makes a tray useless: one would either never
        // show the window or never hide it.
        assert_ne!(
            event_for_command(cmd::OPEN_WINDOW),
            event_for_command(cmd::MINIMIZE)
        );
    }

    #[test]
    fn poll_event_yields_what_was_sent() {
        // This is the hand-off from the tray thread to the render loop; it
        // returning None unconditionally is what made every menu item a no-op.
        let tray = SystemTray::new();
        tray.send_event(TrayEvent::MinimizeToTray);
        assert_eq!(tray.poll_event(), Some(TrayEvent::MinimizeToTray));
        assert!(tray.poll_event().is_none(), "drained once only");
    }

    #[test]
    fn events_arrive_in_order() {
        let tray = SystemTray::new();
        tray.send_event(TrayEvent::OpenWindow);
        tray.send_event(TrayEvent::OpenSettings);
        tray.send_event(TrayEvent::Quit);
        assert_eq!(tray.poll_event(), Some(TrayEvent::OpenWindow));
        assert_eq!(tray.poll_event(), Some(TrayEvent::OpenSettings));
        assert_eq!(tray.poll_event(), Some(TrayEvent::Quit));
    }

    /// The queue the tray window thread writes to must survive being read from
    /// the render thread, and must not wedge when a reader panics.
    #[test]
    fn pending_events_cross_threads_without_loss() {
        let writer = std::thread::spawn(|| {
            for _ in 0..8 {
                deliver(TrayEvent::OpenWindow);
            }
        });
        writer.join().expect("writer finished");
        let drained = take_pending_events();
        assert_eq!(
            drained.len(),
            8,
            "every tray event should reach the render loop"
        );
    }

    #[test]
    fn pending_events_are_drained_exactly_once() {
        deliver(TrayEvent::Quit);
        assert_eq!(take_pending_events().len(), 1);
        assert!(
            take_pending_events().is_empty(),
            "a second drain must be empty, or events replay every frame"
        );
    }

    #[test]
    fn installed_flag_starts_false() {
        // Minimise-to-tray must be gated on this, or the window can be hidden
        // with no icon to bring it back.
        let tray = SystemTray::new();
        assert!(!tray.is_installed());
    }

    #[test]
    fn visibility_flag_is_settable() {
        let mut tray = SystemTray::new();
        assert!(!tray.is_visible());
        tray.set_visible(true);
        assert!(tray.is_visible());
        tray.set_visible(false);
        assert!(!tray.is_visible());
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
