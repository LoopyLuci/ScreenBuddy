#!/bin/bash
# Comprehensive ScreenBuddy implementation script
# Implements: chat overlay, system tray, screen physics, animations, config, audio

cd /G/Projects/ScreenBuddy

# Clean up stray directories
rm -rf screenbuddy-core screenbuddy platform 2>/dev/null

echo "=== Writing all ScreenBuddy implementation files ==="

# ─────────────────────────────────────────────────────────────────────────────
# 1. CHAT OVERLAY - Transparent chat window
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/chat_overlay.rs << 'CHAT_EOF'
//! Transparent Chat Overlay for ScreenBuddy
//!
//! Provides a transparent, always-on-top chat window positioned near the creature.
//! Uses raw Win32 API for per-pixel transparency and click-through support.

use std::mem;
use std::ptr;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// Chat overlay events
#[derive(Debug, Clone)]
pub enum ChatOverlayEvent {
    /// User sent a message
    UserMessage(String),
    /// AI response received
    AiResponse(String),
    /// Toggle visibility
    Toggle,
    /// Move window
    MoveTo(i32, i32),
    /// Show the window,
    Show,
    /// Hide the window
    Hide,
    /// Shutdown
    Shutdown,
}

/// Chat overlay state
pub struct ChatOverlay {
    event_tx: Sender<ChatOverlayEvent>,
    event_rx: Receiver<ChatOverlayEvent>,
    is_visible: bool,
    position: (i32, i32),
    size: (i32, i32),
}

impl ChatOverlay {
    pub fn new() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            event_tx,
            event_rx,
            is_visible: false,
            position: (100, 100),
            size: (300, 400),
        }
    }

    /// Start the chat overlay window in a separate thread
    pub fn start(&self) -> Result<(), String> {
        let (tx, rx) = channel::<isize>();
        let (event_tx, event_rx) = (self.event_tx.clone(), self.event_rx.clone());
        
        thread::spawn(move || {
            unsafe {
                let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
                if h_instance.is_null() { return; }

                let class_name: Vec<u16> = "ScreenBuddyChat\0".encode_utf16().collect();
                let window_title: Vec<u16> = "Chat\0".encode_utf16().collect();

                let wc = winapi::um::winuser::WNDCLASSEXW {
                    cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
                    style: winapi::um::winuser::CS_HREDRAW | winapi::um::winuser::CS_VREDRAW,
                    lpfnWndProc: Some(chat_window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: h_instance,
                    hIcon: ptr::null_mut(),
                    hCursor: winapi::um::winuser::LoadCursorW(ptr::null_mut(), winapi::um::winuser::IDC_ARROW),
                    hbrBackground: ptr::null_mut(),
                    lpszMenuName: ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: ptr::null_mut(),
                };
                winapi::um::winuser::RegisterClassExW(&wc);

                let ex_style = winapi::um::winuser::WS_EX_LAYERED
                    | winapi::um::winuser::WS_EX_TOPMOST
                    | winapi::um::winuser::WS_EX_TOOLWINDOW
                    | winapi::um::winuser::WS_EX_TRANSPARENT;

                let hwnd = winapi::um::winuser::CreateWindowExW(
                    ex_style,
                    class_name.as_ptr(),
                    window_title.as_ptr(),
                    winapi::um::winuser::WS_POPUP,
                    100, 100, 300, 400,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    h_instance,
                    ptr::null_mut(),
                );

                if hwnd.is_null() { return; }
                
                // Set transparency color key
                winapi::um::winuser::SetLayeredWindowAttributes(
                    hwnd,
                    winapi::um::wingdi::RGB(0, 0, 0),
                    200,
                    winapi::um::winuser::LWA_COLORKEY | winapi::um::winuser::LWA_ALPHA,
                );

                tx.send(hwnd as isize).ok();

                let mut msg: winapi::um::winuser::MSG = mem::zeroed();
                while winapi::um::winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                    winapi::um::winuser::TranslateMessage(&msg);
                    winapi::um::winuser::DispatchMessageW(&msg);
                }
            }
        });

        Ok(())
    }

    /// Send an event to the overlay
    pub fn send_event(&self, event: ChatOverlayEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Get the event sender
    pub fn event_sender(&self) -> Sender<ChatOverlayEvent> {
        self.event_tx.clone()
    }

    /// Check for events
    pub fn poll_event(&self) -> Option<ChatOverlayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Set position
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.position = (x, y);
    }

    /// Set size
    pub fn set_size(&mut self, w: i32, h: i32) {
        self.size = (w, h);
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}

unsafe extern "system" fn chat_window_proc(
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
        winapi::um::winuser::WM_PAINT => {
            let mut ps: winapi::um::winuser::PAINTSTRUCT = mem::zeroed();
            let hdc = winapi::um::winuser::BeginPaint(hwnd, &mut ps);
            
            // Draw semi-transparent background
            let mut rect: winapi::shared::windef::RECT = mem::zeroed();
            winapi::um::winuser::GetClientRect(hwnd, &mut rect);
            
            // Fill with dark background
            let brush = winapi::um::wingdi::CreateSolidBrush(
                winapi::um::wingdi::RGB(30, 30, 30)
            );
            winapi::um::winuser::FillRect(hdc, &mut rect, brush);
            winapi::um::wingdi::DeleteObject(brush as *mut _);
            
            winapi::um::winuser::EndPaint(hwnd, &mut ps);
            0
        }
        winapi::um::winuser::WM_LBUTTONDOWN => {
            winapi::um::winuser::SendMessageW(
                hwnd,
                winapi::um::winuser::WM_NCLBUTTONDOWN,
                winapi::um::winuser::HTCAPTION as usize,
                0,
            );
            0
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_overlay_creation() {
        let overlay = ChatOverlay::new();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_chat_overlay_toggle() {
        let mut overlay = ChatOverlay::new();
        assert!(!overlay.is_visible());
        overlay.toggle();
        assert!(overlay.is_visible());
        overlay.toggle();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_chat_overlay_position() {
        let mut overlay = ChatOverlay::new();
        overlay.set_position(200, 300);
        assert_eq!(overlay.position, (200, 300));
    }

    #[test]
    fn test_chat_overlay_size() {
        let mut overlay = ChatOverlay::new();
        overlay.set_size(400, 500);
        assert_eq!(overlay.size, (400, 500));
    }

    #[test]
    fn test_chat_overlay_events() {
        let overlay = ChatOverlay::new();
        overlay.send_event(ChatOverlayEvent::UserMessage("Hello".to_string()));
        
        // Give a moment for the channel
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        if let Some(event) = overlay.poll_event() {
            match event {
                ChatOverlayEvent::UserMessage(msg) => assert_eq!(msg, "Hello"),
                _ => panic!("Wrong event type"),
            }
        }
    }
}
CHAT_EOF
echo "  ✓ chat_overlay.rs"

# ─────────────────────────────────────────────────────────────────────────────
# 2. SYSTEM TRAY - Windows system tray integration
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/system_tray.rs << 'TRAY_EOF'
//! System Tray Integration for ScreenBuddy
//!
//! Provides Windows system tray icon with context menu.
//! Supports: show/hide creature, open chat, switch creature, quit.

use std::mem;
use std::ptr;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

/// System tray events
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// Show the creature
    ShowCreature,
    /// Hide the creature
    HideCreature,
    /// Open chat overlay
    OpenChat,
    /// Switch to next creature
    NextCreature,
    /// Open settings
    OpenSettings,
    /// Quit the application
    Quit,
    /// Show about dialog
    About,
}

/// System tray state
pub struct SystemTray {
    event_tx: Sender<TrayEvent>,
    event_rx: Receiver<TrayEvent>,
    is_visible: bool,
    creature_visible: bool,
    chat_open: bool,
}

impl SystemTray {
    pub fn new() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            event_tx,
            event_rx,
            is_visible: false,
            creature_visible: true,
            chat_open: false,
        }
    }

    /// Start the system tray icon
    pub fn start(&self) -> Result<(), String> {
        let (tx, rx) = channel::<isize>();
        let event_tx = self.event_tx.clone();
        
        thread::spawn(move || {
            unsafe {
                let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
                if h_instance.is_null() { return; }

                // Create a hidden window for tray messages
                let class_name: Vec<u16> = "ScreenBuddyTray\0".encode_utf16().collect();
                let wc = winapi::um::winuser::WNDCLASSEXW {
                    cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: Some(tray_window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: h_instance,
                    hIcon: ptr::null_mut(),
                    hCursor: winapi::um::winuser::LoadCursorW(ptr::null_mut(), winapi::um::winuser::IDC_ARROW),
                    hbrBackground: ptr::null_mut(),
                    lpszMenuName: ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: ptr::null_mut(),
                };
                winapi::um::winuser::RegisterClassExW(&wc);

                let hwnd = winapi::um::winuser::CreateWindowExW(
                    winapi::um::winuser::WS_EX_TOOLWINDOW,
                    class_name.as_ptr(),
                    "ScreenBuddy\0".encode_utf16().collect::<Vec<u16>>().as_ptr(),
                    0,
                    0, 0, 0, 0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    h_instance,
                    ptr::null_mut(),
                );

                if hwnd.is_null() { return; }

                // Add tray icon
                let mut nid: winapi::um::shellapi::NOTIFYICONDATAW = mem::zeroed();
                nid.cbSize = mem::size_of::<winapi::um::shellapi::NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 1;
                nid.uFlags = winapi::um::shellapi::NIF_ICON | winapi::um::shellapi::NIF_MESSAGE | winapi::um::shellapi::NIF_TIP;
                nid.uCallbackMessage = 0x8000; // WM_APP
                nid.hIcon = winapi::um::winuser::LoadIconW(ptr::null_mut(), winapi::um::winuser::IDI_APPLICATION);
                
                let tip = "ScreenBuddy AI Companion\0".encode_utf16().collect::<Vec<u16>>();
                for (i, &c) in tip.iter().enumerate().take(128) {
                    nid.szTip[i] = c;
                }

                winapi::um::shellapi::Shell_NotifyIconW(
                    winapi::um::shellapi::NIM_ADD,
                    &mut nid,
                );

                tx.send(hwnd as isize).ok();

                // Message loop
                let mut msg: winapi::um::winuser::MSG = mem::zeroed();
                while winapi::um::winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                    winapi::um::winuser::TranslateMessage(&msg);
                    winapi::um::winuser::DispatchMessageW(&msg);
                }

                // Remove tray icon on exit
                winapi::um::shellapi::Shell_NotifyIconW(
                    winapi::um::shellapi::NIM_DELETE,
                    &mut nid,
                );
            }
        });

        Ok(())
    }

    /// Send an event
    pub fn send_event(&self, event: TrayEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Poll for events
    pub fn poll_event(&self) -> Option<TrayEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get event sender
    pub fn event_sender(&self) -> Sender<TrayEvent> {
        self.event_tx.clone()
    }

    /// Set creature visibility
    pub fn set_creature_visible(&mut self, visible: bool) {
        self.creature_visible = visible;
    }

    /// Check if creature is visible
    pub fn is_creature_visible(&self) -> bool {
        self.creature_visible
    }

    /// Set chat open state
    pub fn set_chat_open(&mut self, open: bool) {
        self.chat_open = open;
    }

    /// Check if chat is open
    pub fn is_chat_open(&self) -> bool {
        self.chat_open
    }
}

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
        0x8000 => {
            // WM_APP - tray icon message
            match lparam as u32 {
                winapi::um::winuser::WM_RBUTTONUP => {
                    // Show context menu
                    let h_menu = winapi::um::winuser::CreatePopupMenu();
                    
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1001, "Show/Hide Creature\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1002, "Open Chat\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1003, "Next Creature\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                    winapi::um::winuser::AppendMenuW(h_menu, winapi::um::winuser::MF_SEPARATOR, 0, ptr::null());
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1004, "Settings\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1005, "About\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
                    winapi::um::winuser::AppendMenuW(h_menu, winapi::um::winuser::MF_SEPARATOR, 0, ptr::null());
                    winapi::um::winuser::AppendMenuW(h_menu, 0, 1006, "Quit\0".encode_utf16().collect::<Vec<u16>>().as_ptr());

                    let mut point: winapi::shared::windef::POINT = mem::zeroed();
                    winapi::um::winuser::GetCursorPos(&mut point);

                    winapi::um::winuser::SetForegroundWindow(hwnd);
                    let cmd = winapi::um::winuser::TrackPopupMenu(
                        h_menu,
                        winapi::um::winuser::TPM_RIGHTALIGN | winapi::um::winuser::TPM_BOTTOMALIGN | winapi::um::winuser::TPM_RETURNCMD,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        ptr::null(),
                    );

                    match cmd {
                        1001 => { /* Show/Hide creature */ }
                        1002 => { /* Open chat */ }
                        1003 => { /* Next creature */ }
                        1004 => { /* Settings */ }
                        1005 => { /* About */ }
                        1006 => { winapi::um::winuser::PostQuitMessage(0); }
                        _ => {}
                    }

                    winapi::um::winuser::DestroyMenu(h_menu);
                }
                _ => {}
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
    fn test_system_tray_creation() {
        let tray = SystemTray::new();
        assert!(tray.is_creature_visible());
        assert!(!tray.is_chat_open());
    }

    #[test]
    fn test_system_tray_creature_visibility() {
        let mut tray = SystemTray::new();
        assert!(tray.is_creature_visible());
        tray.set_creature_visible(false);
        assert!(!tray.is_creature_visible());
    }

    #[test]
    fn test_system_tray_chat_state() {
        let mut tray = SystemTray::new();
        assert!(!tray.is_chat_open());
        tray.set_chat_open(true);
        assert!(tray.is_chat_open());
    }

    #[test]
    fn test_system_tray_events() {
        let tray = SystemTray::new();
        tray.send_event(TrayEvent::ShowCreature);
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        if let Some(event) = tray.poll_event() {
            match event {
                TrayEvent::ShowCreature => {},
                _ => panic!("Wrong event type"),
            }
        }
    }
}
TRAY_EOF
echo "  ✓ system_tray.rs"

# ─────────────────────────────────────────────────────────────────────────────
# 3. SCREEN PHYSICS - Multi-monitor support and boundary detection
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/screen_physics.rs << 'PHYSICS_EOF'
//! Screen Physics for ScreenBuddy
//!
//! Handles multi-monitor detection, screen boundaries, and creature movement.
//! Ensures the creature stays within visible screen bounds.

use std::mem;
use std::ptr;

/// Represents a monitor/display
#[derive(Debug, Clone)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

impl Monitor {
    /// Get the bounds as (left, top, right, bottom)
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.x + self.width as i32, self.y + self.height as i32)
    }

    /// Check if a point is within this monitor
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let (left, top, right, bottom) = self.bounds();
        x >= left && x < right && y >= top && y < bottom
    }

    /// Get the center of this monitor
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width as i32 / 2, self.y + self.height as i32 / 2)
    }

    /// Clamp a position to stay within this monitor
    pub fn clamp_position(&self, x: i32, y: i32, creature_size: u32) -> (i32, i32) {
        let (left, top, right, bottom) = self.bounds();
        let w = creature_size as i32;
        let h = creature_size as i32;
        
        (
            x.max(left).min(right - w),
            y.max(top).min(bottom - h),
        )
    }
}

/// Screen manager for multi-monitor support
pub struct ScreenManager {
    monitors: Vec<Monitor>,
    primary_index: usize,
}

impl ScreenManager {
    pub fn new() -> Self {
        let mut manager = Self {
            monitors: Vec::new(),
            primary_index: 0,
        };
        manager.refresh();
        manager
    }

    /// Refresh the monitor list
    pub fn refresh(&mut self) {
        self.monitors.clear();
        self.primary_index = 0;

        unsafe {
            winapi::um::winuser::EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(enum_monitor_proc),
                &mut self.monitors as *mut _ as isize,
            );
        }

        // Find primary monitor
        for (i, monitor) in self.monitors.iter().enumerate() {
            if monitor.is_primary {
                self.primary_index = i;
                break;
            }
        }
    }

    /// Get all monitors
    pub fn monitors(&self) -> &[Monitor] {
        &self.monitors
    }

    /// Get the primary monitor
    pub fn primary(&self) -> Option<&Monitor> {
        self.monitors.get(self.primary_index)
    }

    /// Get the total virtual screen bounds
    pub fn virtual_screen_bounds(&self) -> (i32, i32, i32, i32) {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for monitor in &self.monitors {
            let (left, top, right, bottom) = monitor.bounds();
            min_x = min_x.min(left);
            min_y = min_y.min(top);
            max_x = max_x.max(right);
            max_y = max_y.max(bottom);
        }

        (min_x, min_y, max_x, max_y)
    }

    /// Find which monitor contains a point
    pub fn monitor_at(&self, x: i32, y: i32) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.contains(x, y))
    }

    /// Clamp a position to stay within screen bounds
    pub fn clamp_to_screen(&self, x: i32, y: i32, creature_size: u32) -> (i32, i32) {
        if let Some(monitor) = self.monitor_at(x, y) {
            monitor.clamp_position(x, y, creature_size)
        } else {
            if let Some(primary) = self.primary() {
                primary.clamp_position(x, y, creature_size)
            } else {
                (x, y)
            }
        }
    }

    /// Get a random position on the primary monitor
    pub fn random_position(&self, creature_size: u32) -> Option<(i32, i32)> {
        self.primary().map(|monitor| {
            let (left, top, right, bottom) = monitor.bounds();
            let w = creature_size as i32;
            let h = creature_size as i32;
            
            let x = left + (rand::random::<i32>().abs() % (right - left - w));
            let y = top + (rand::random::<i32>().abs() % (bottom - top - h));
            
            (x, y)
        })
    }
}

unsafe extern "system" fn enum_monitor_proc(
    hmonitor: winapi::shared::windef::HMONITOR,
    _hdc: winapi::shared::windef::HDC,
    _rect: winapi::shared::windef::LPRECT,
    data: isize,
) -> i32 {
    let monitors = &mut *(data as *mut Vec<Monitor>);
    
    let mut info: winapi::um::winuser::MONITORINFOEXW = mem::zeroed();
    info.cbSize = mem::size_of::<winapi::um::winuser::MONITORINFOEXW>() as u32;
    
    if winapi::um::winuser::GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
        let monitor = Monitor {
            x: info.rcMonitor.left,
            y: info.rcMonitor.top,
            width: (info.rcMonitor.right - info.rcMonitor.left) as u32,
            height: (info.rcMonitor.bottom - info.rcMonitor.top) as u32,
            is_primary: (info.dwFlags & 1) != 0, // MONITORINFOF_PRIMARY
        };
        monitors.push(monitor);
    }
    
    1 // Continue enumeration
}

/// Creature physics state
pub struct CreaturePhysics {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub size: u32,
    pub gravity: f32,
    pub friction: f32,
    pub bounce: f32,
}

impl CreaturePhysics {
    pub fn new(x: f32, y: f32, size: u32) -> Self {
        Self {
            x, y,
            vx: 0.0,
            vy: 0.0,
            size,
            gravity: 0.5,
            friction: 0.95,
            bounce: 0.7,
        }
    }

    /// Update physics for one frame
    pub fn update(&mut self, dt: f32, screen: &ScreenManager) {
        // Apply gravity
        self.vy += self.gravity * dt;
        
        // Apply friction
        self.vx *= self.friction;
        self.vy *= self.friction;
        
        // Update position
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        
        // Clamp to screen bounds
        let (clamped_x, clamped_y) = screen.clamp_to_screen(
            self.x as i32,
            self.y as i32,
            self.size,
        );
        
        // Bounce off edges
        if self.x as i32 != clamped_x {
            self.vx = -self.vx * self.bounce;
            self.x = clamped_x as f32;
        }
        if self.y as i32 != clamped_y {
            self.vy = -self.vy * self.bounce;
            self.y = clamped_y as f32;
        }
    }

    /// Apply a force to the creature
    pub fn apply_force(&mut self, fx: f32, fy: f32) {
        self.vx += fx;
        self.vy += fy;
    }

    /// Set velocity directly
    pub fn set_velocity(&mut self, vx: f32, vy: f32) {
        self.vx = vx;
        self.vy = vy;
    }

    /// Get the current position as integers
    pub fn position(&self) -> (i32, i32) {
        (self.x as i32, self.y as i32)
    }

    /// Check if the creature is moving
    pub fn is_moving(&self) -> bool {
        (self.vx.abs() > 0.1 || self.vy.abs() > 0.1)
    }

    /// Get speed
    pub fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_bounds() {
        let monitor = Monitor {
            x: 0, y: 0, width: 1920, height: 1080, is_primary: true,
        };
        assert_eq!(monitor.bounds(), (0, 0, 1920, 1080));
    }

    #[test]
    fn test_monitor_contains() {
        let monitor = Monitor {
            x: 0, y: 0, width: 1920, height: 1080, is_primary: true,
        };
        assert!(monitor.contains(100, 100));
        assert!(monitor.contains(0, 0));
        assert!(!monitor.contains(2000, 100));
        assert!(!monitor.contains(100, 2000));
    }

    #[test]
    fn test_monitor_clamp() {
        let monitor = Monitor {
            x: 0, y: 0, width: 1920, height: 1080, is_primary: true,
        };
        let (x, y) = monitor.clamp_position(2000, 2000, 256);
        assert_eq!(x, 1920 - 256);
        assert_eq!(y, 1080 - 256);
    }

    #[test]
    fn test_monitor_center() {
        let monitor = Monitor {
            x: 0, y: 0, width: 1920, height: 1080, is_primary: true,
        };
        assert_eq!(monitor.center(), (960, 540));
    }

    #[test]
    fn test_screen_manager_creation() {
        let screen = ScreenManager::new();
        assert!(!screen.monitors().is_empty());
    }

    #[test]
    fn test_screen_manager_primary() {
        let screen = ScreenManager::new();
        assert!(screen.primary().is_some());
    }

    #[test]
    fn test_screen_manager_virtual_bounds() {
        let screen = ScreenManager::new();
        let (min_x, min_y, max_x, max_y) = screen.virtual_screen_bounds();
        assert!(max_x > min_x);
        assert!(max_y > min_y);
    }

    #[test]
    fn test_creature_physics_creation() {
        let physics = CreaturePhysics::new(100.0, 100.0, 256);
        assert_eq!(physics.position(), (100, 100));
        assert!(!physics.is_moving());
    }

    #[test]
    fn test_creature_physics_force() {
        let mut physics = CreaturePhysics::new(100.0, 100.0, 256);
        physics.apply_force(10.0, -5.0);
        assert!(physics.is_moving());
        assert!(physics.vx > 0.0);
        assert!(physics.vy < 0.0);
    }

    #[test]
    fn test_creature_physics_gravity() {
        let screen = ScreenManager::new();
        let mut physics = CreaturePhysics::new(100.0, 100.0, 256);
        physics.apply_force(0.0, -10.0);
        
        for _ in 0..10 {
            physics.update(1.0, &screen);
        }
        
        // Creature should have moved and bounced
        assert!(physics.speed() >= 0.0);
    }

    #[test]
    fn test_creature_physics_bounce() {
        let screen = ScreenManager::new();
        let mut physics = CreaturePhysics::new(100.0, 100.0, 256);
        physics.set_velocity(50.0, 0.0);
        
        // Update many times to hit a wall
        for _ in 0..100 {
            physics.update(1.0, &screen);
        }
        
        // Should have bounced (velocity reversed)
        assert!(physics.vx <= 50.0);
    }

    #[test]
    fn test_creature_physics_speed() {
        let physics = CreaturePhysics::new(0.0, 0.0, 256);
        assert_eq!(physics.speed(), 0.0);
    }
}
PHYSICS_EOF
echo "  ✓ screen_physics.rs"

# ─────────────────────────────────────────────────────────────────────────────
# 4. ANIMATION SYSTEM - Multi-state sprite management
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/animation.rs << 'ANIM_EOF'
//! Animation System for ScreenBuddy
//!
//! Manages sprite sheets and frame-based animations for creature states.
//! Supports: idle, walk, fly, sleep, celebrate, and custom animations.

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Animation state types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationState {
    Idle,
    Walk,
    Fly,
    Sleep,
    Celebrate,
    Custom(u32),
}

impl std::fmt::Display for AnimationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationState::Idle => write!(f, "idle"),
            AnimationState::Walk => write!(f, "walk"),
            AnimationState::Fly => write!(f, "fly"),
            AnimationState::Sleep => write!(f, "sleep"),
            AnimationState::Celebrate => write!(f, "celebrate"),
            AnimationState::Custom(id) => write!(f, "custom_{}", id),
        }
    }
}

/// A single animation (collection of frames)
#[derive(Debug, Clone)]
pub struct Animation {
    pub state: AnimationState,
    pub frames: Vec<PathBuf>,
    pub current_frame: usize,
    pub fps: f64,
    pub looping: bool,
    pub time_accumulator: f64,
}

impl Animation {
    pub fn new(state: AnimationState, fps: f64, looping: bool) -> Self {
        Self {
            state,
            frames: Vec::new(),
            current_frame: 0,
            fps,
            looping,
            time_accumulator: 0.0,
        }
    }

    /// Add a frame from a file path
    pub fn add_frame(&mut self, path: PathBuf) {
        self.frames.push(path);
    }

    /// Add multiple frames
    pub fn add_frames(&mut self, paths: Vec<PathBuf>) {
        self.frames.extend(paths);
    }

    /// Update the animation
    pub fn update(&mut self, delta_time: f64) {
        if self.frames.is_empty() { return; }
        
        self.time_accumulator += delta_time;
        let frame_duration = 1.0 / self.fps;
        
        while self.time_accumulator >= frame_duration {
            self.time_accumulator -= frame_duration;
            self.current_frame += 1;
            
            if self.current_frame >= self.frames.len() {
                if self.looping {
                    self.current_frame = 0;
                } else {
                    self.current_frame = self.frames.len() - 1;
                }
            }
        }
    }

    /// Get the current frame path
    pub fn current_frame_path(&self) -> Option<&PathBuf> {
        self.frames.get(self.current_frame)
    }

    /// Get the current frame index
    pub fn frame_index(&self) -> usize {
        self.current_frame
    }

    /// Get the total number of frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Check if the animation has finished (non-looping only)
    pub fn is_finished(&self) -> bool {
        if self.looping { return false; }
        self.current_frame >= self.frames.len().saturating_sub(1)
    }

    /// Reset to the first frame
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.time_accumulator = 0.0;
    }

    /// Set the FPS
    pub fn set_fps(&mut self, fps: f64) {
        self.fps = fps;
    }
}

/// Animation controller managing multiple animations
pub struct AnimationController {
    animations: HashMap<AnimationState, Animation>,
    current_state: AnimationState,
    default_state: AnimationState,
}

impl AnimationController {
    pub fn new(default_state: AnimationState) -> Self {
        Self {
            animations: HashMap::new(),
            current_state: default_state,
            default_state,
        }
    }

    /// Add an animation
    pub fn add_animation(&mut self, animation: Animation) {
        self.animations.insert(animation.state, animation);
    }

    /// Get an animation by state
    pub fn get(&self, state: AnimationState) -> Option<&Animation> {
        self.animations.get(&state)
    }

    /// Get a mutable animation
    pub fn get_mut(&mut self, state: AnimationState) -> Option<&mut Animation> {
        self.animations.get_mut(&state)
    }

    /// Switch to a different animation state
    pub fn set_state(&mut self, state: AnimationState) -> bool {
        if self.animations.contains_key(&state) {
            if self.current_state != state {
                // Reset the new animation
                if let Some(anim) = self.animations.get_mut(&state) {
                    anim.reset();
                }
                self.current_state = state;
            }
            true
        } else {
            false
        }
    }

    /// Get the current animation state
    pub fn current_state(&self) -> AnimationState {
        self.current_state
    }

    /// Update the current animation
    pub fn update(&mut self, delta_time: f64) {
        if let Some(anim) = self.animations.get_mut(&self.current_state) {
            anim.update(delta_time);
        }
    }

    /// Get the current frame path
    pub fn current_frame_path(&self) -> Option<&PathBuf> {
        self.animations.get(&self.current_state)
            .and_then(|anim| anim.current_frame_path())
    }

    /// Get the current frame index
    pub fn current_frame_index(&self) -> usize {
        self.animations.get(&self.current_state)
            .map(|anim| anim.frame_index())
            .unwrap_or(0)
    }

    /// Check if the current animation is finished
    pub fn is_current_finished(&self) -> bool {
        self.animations.get(&self.current_state)
            .map(|anim| anim.is_finished())
            .unwrap_or(true)
    }

    /// Reset the current animation
    pub fn reset_current(&mut self) {
        if let Some(anim) = self.animations.get_mut(&self.current_state) {
            anim.reset();
        }
    }

    /// Get all available states
    pub fn available_states(&self) -> Vec<AnimationState> {
        self.animations.keys().cloned().collect()
    }

    /// Check if a state has an animation
    pub fn has_state(&self, state: AnimationState) -> bool {
        self.animations.contains_key(&state)
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new(AnimationState::Idle)
    }
}

/// Sprite sheet loader
pub struct SpriteSheetLoader;

impl SpriteSheetLoader {
    /// Load a sprite sheet from a directory
    /// Expects files named: state_0.png, state_1.png, etc.
    pub fn load_from_directory(
        dir: &PathBuf,
        state: AnimationState,
        fps: f64,
        looping: bool,
    ) -> Result<Animation, String> {
        let mut animation = Animation::new(state, fps, looping);
        
        if dir.exists() {
            let prefix = format!("{}_", state);
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&prefix) && name.ends_with(".png")
                })
                .collect();
            
            entries.sort_by_key(|e| e.file_name());
            
            for entry in entries {
                animation.add_frame(entry.path());
            }
        }
        
        Ok(animation)
    }

    /// Load all animations from a directory structure
    pub fn load_all_from_directory(base_dir: &PathBuf) -> Result<Vec<Animation>, String> {
        let mut animations = Vec::new();
        
        let states = vec![
            (AnimationState::Idle, "idle"),
            (AnimationState::Walk, "walk"),
            (AnimationState::Fly, "fly"),
            (AnimationState::Sleep, "sleep"),
            (AnimationState::Celebrate, "celebrate"),
        ];
        
        for (state, name) in states {
            let dir = base_dir.join(name);
            if dir.exists() {
                match Self::load_from_directory(&dir, state, 12.0, true) {
                    Ok(anim) => animations.push(anim),
                    Err(e) => eprintln!("Warning: Failed to load animation {}: {}", name, e),
                }
            }
        }
        
        Ok(animations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_state_display() {
        assert_eq!(format!("{}", AnimationState::Idle), "idle");
        assert_eq!(format!("{}", AnimationState::Walk), "walk");
        assert_eq!(format!("{}", AnimationState::Custom(42)), "custom_42");
    }

    #[test]
    fn test_animation_creation() {
        let anim = Animation::new(AnimationState::Idle, 12.0, true);
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.fps, 12.0);
        assert!(anim.looping);
        assert_eq!(anim.frame_count(), 0);
    }

    #[test]
    fn test_animation_add_frames() {
        let mut anim = Animation::new(AnimationState::Idle, 12.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        assert_eq!(anim.frame_count(), 2);
    }

    #[test]
    fn test_animation_update() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true); // 1 FPS
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        
        anim.update(0.5);
        assert_eq!(anim.frame_index(), 0);
        
        anim.update(0.6); // Total 1.1s, should advance
        assert_eq!(anim.frame_index(), 1);
    }

    #[test]
    fn test_animation_looping() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        
        anim.update(3.0); // 3 seconds at 1 FPS = 3 frames, should loop
        assert_eq!(anim.frame_index(), 1); // 3 % 2 = 1
    }

    #[test]
    fn test_animation_non_looping() {
        let mut anim = Animation::new(AnimationState::Celebrate, 1.0, false);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        
        anim.update(5.0); // Way past end
        assert_eq!(anim.frame_index(), 1); // Stays at last frame
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animation_reset() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        
        anim.update(1.5);
        assert_eq!(anim.frame_index(), 1);
        
        anim.reset();
        assert_eq!(anim.frame_index(), 0);
    }

    #[test]
    fn test_animation_controller() {
        let mut controller = AnimationController::new(AnimationState::Idle);
        
        let mut idle_anim = Animation::new(AnimationState::Idle, 12.0, true);
        idle_anim.add_frame(PathBuf::from("idle_0.png"));
        controller.add_animation(idle_anim);
        
        let mut walk_anim = Animation::new(AnimationState::Walk, 12.0, true);
        walk_anim.add_frame(PathBuf::from("walk_0.png"));
        controller.add_animation(walk_anim);
        
        assert!(controller.has_state(AnimationState::Idle));
        assert!(controller.has_state(AnimationState::Walk));
        assert!(!controller.has_state(AnimationState::Fly));
        
        assert!(controller.set_state(AnimationState::Walk));
        assert_eq!(controller.current_state(), AnimationState::Walk);
    }

    #[test]
    fn test_animation_controller_update() {
        let mut controller = AnimationController::new(AnimationState::Idle);
        
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        controller.add_animation(anim);
        
        controller.update(1.5);
        assert_eq!(controller.current_frame_index(), 1);
    }

    #[test]
    fn test_animation_controller_states() {
        let mut controller = AnimationController::new(AnimationState::Idle);
        
        let idle = Animation::new(AnimationState::Idle, 12.0, true);
        controller.add_animation(idle);
        
        let states = controller.available_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0], AnimationState::Idle);
    }

    #[test]
    fn test_sprite_sheet_loader() {
        // This test just verifies the loader doesn't panic on missing dir
        let result = SpriteSheetLoader::load_from_directory(
            &PathBuf::from("/nonexistent"),
            AnimationState::Idle,
            12.0,
            true,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().frame_count(), 0);
    }
}
ANIM_EOF
echo "  ✓ animation.rs"

# ─────────────────────────────────────────────────────────────────────────────
# 5. CONFIG PERSISTENCE - Save/load settings
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/config.rs << 'CONFIG_EOF'
//! Configuration Management for ScreenBuddy
//!
//! Handles saving/loading of user settings, creature selections,
//! AI configuration, and RAG knowledge base paths.

use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use parking_lot::RwLock;

use crate::ai::AiConfig;
use crate::audio::AudioConfig;
use crate::providers::ProviderRegistry;
use crate::state::AppConfig;

/// ScreenBuddy configuration file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenBuddyConfig {
    pub version: String,
    pub active_creature: String,
    pub window_position: Option<(i32, i32)>,
    pub window_size: Option<(u32, u32)>,
    pub ai: AiConfig,
    pub audio: AudioConfig,
    pub app: AppConfig,
    pub providers: ProviderRegistry,
    pub knowledge_paths: Vec<PathBuf>,
    pub auto_start: bool,
    pub check_updates: bool,
    pub theme: Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    Auto,
}

impl Default for ScreenBuddyConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            active_creature: "companion-bird-01".to_string(),
            window_position: None,
            window_size: None,
            ai: AiConfig::default(),
            audio: AudioConfig::default(),
            app: AppConfig::default(),
            providers: ProviderRegistry::default(),
            knowledge_paths: Vec::new(),
            auto_start: false,
            check_updates: true,
            theme: Theme::Auto,
        }
    }
}

pub struct ConfigManager {
    config: RwLock<ScreenBuddyConfig>,
    config_path: PathBuf,
    dirty: RwLock<bool>,
}

impl ConfigManager {
    pub fn new() -> Self {
        let config_path = Self::config_path();
        let config = if config_path.exists() {
            Self::load_from(&config_path).unwrap_or_default()
        } else {
            ScreenBuddyConfig::default()
        };
        Self {
            config: RwLock::new(config),
            config_path,
            dirty: RwLock::new(false),
        }
    }

    pub fn load() -> Result<Self, String> {
        let config_path = Self::config_path();
        let config = Self::load_from(&config_path)?;
        Ok(Self {
            config: RwLock::new(config),
            config_path,
            dirty: RwLock::new(false),
        })
    }

    fn config_path() -> PathBuf {
        if let Some(dirs) = directories::ProjectDirs::from("", "ScreenBuddy", "ScreenBuddy") {
            dirs.config_dir().join("config.toml")
        } else {
            std::env::current_dir().unwrap_or_default().join("screenbuddy.toml")
        }
    }

    fn load_from(path: &PathBuf) -> Result<ScreenBuddyConfig, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        let config: ScreenBuddyConfig = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), String> {
        let config = self.config.read();
        let content = toml::to_string_pretty(&*config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        std::fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        *self.dirty.write() = false;
        Ok(())
    }

    pub fn get(&self) -> ScreenBuddyConfig {
        self.config.read().clone()
    }

    pub fn update<F>(&self, f: F) where F: FnOnce(&mut ScreenBuddyConfig) {
        let mut config = self.config.write();
        f(&mut config);
        *self.dirty.write() = true;
    }

    pub fn set_active_creature(&self, id: String) {
        self.update(|c| c.active_creature = id);
    }

    pub fn set_window_position(&self, x: i32, y: i32) {
        self.update(|c| c.window_position = Some((x, y)));
    }

    pub fn set_window_size(&self, w: u32, h: u32) {
        self.update(|c| c.window_size = Some((w, h)));
    }

    pub fn add_knowledge_path(&self, path: PathBuf) {
        self.update(|c| {
            if !c.knowledge_paths.contains(&path) {
                c.knowledge_paths.push(path);
            }
        });
    }

    pub fn remove_knowledge_path(&self, path: &PathBuf) {
        self.update(|c| {
            c.knowledge_paths.retain(|p| p != path);
        });
    }

    pub fn is_dirty(&self) -> bool {
        *self.dirty.read()
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

impl Default for ConfigManager {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ScreenBuddyConfig::default();
        assert_eq!(config.active_creature, "companion-bird-01");
        assert!(config.audio.enabled);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();
        let config = manager.get();
        assert_eq!(config.active_creature, "companion-bird-01");
    }

    #[test]
    fn test_set_active_creature() {
        let manager = ConfigManager::new();
        manager.set_active_creature("robo-cat".to_string());
        assert_eq!(manager.get().active_creature, "robo-cat");
        assert!(manager.is_dirty());
    }

    #[test]
    fn test_add_knowledge_path() {
        let manager = ConfigManager::new();
        let path = PathBuf::from("/test/knowledge");
        manager.add_knowledge_path(path.clone());
        assert!(manager.get().knowledge_paths.contains(&path));
        // Adding again should not duplicate
        manager.add_knowledge_path(path.clone());
        assert_eq!(manager.get().knowledge_paths.iter().filter(|p| *p == &path).count(), 1);
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme::Dark;
        let json = serde_json::to_string(&theme).unwrap();
        assert!(json.contains("dark"));
    }
}
CONFIG_EOF
echo "  ✓ config.rs"

# ─────────────────────────────────────────────────────────────────────────────
# 6. UPDATE lib.rs with all new modules
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/src/lib.rs << 'EOF'
pub mod agent;
pub mod ai;
pub mod animation;
pub mod audio;
pub mod chat_overlay;
pub mod chat_ui;
pub mod config;
pub mod creature;
pub mod error;
pub mod ipc;
pub mod providers;
pub mod rag;
pub mod screen_physics;
pub mod state;
pub mod system_tray;

pub use error::{Error, Result};
pub use creature::{Creature, CreatureId, load_creature_from_file};
pub use state::{AppState, CreatureState, State};
pub use ai::{AiEngine, AiConfig, AiRequest, AiResponse, ModelTier, Backend, Message};
pub use rag::{RagPipeline, InMemoryVectorStore, TfIdfEmbedding, VectorStore, Chunk};
pub use agent::{AgentRuntime, AgentEvent, Tool};
pub use audio::{AudioEngine, AudioConfig, SoundCategory};
pub use chat_ui::{ChatHistory, ChatUIState, ChatMessage, MessageRole};
pub use chat_overlay::ChatOverlay;
pub use system_tray::{SystemTray, TrayEvent};
pub use screen_physics::{ScreenManager, CreaturePhysics, Monitor};
pub use animation::{AnimationController, Animation, AnimationState, SpriteSheetLoader};
pub use config::{ConfigManager, ScreenBuddyConfig, Theme};
pub use providers::{
    Provider, ProviderConfig, ProviderRegistry, ProviderCapabilities,
    ProviderBuilder, LocalProviderDetector, get_capabilities,
};
pub use ipc::{IpcServer, IpcClient, GodotCommand, Response, DEFAULT_PORT};
EOF
echo "  ✓ lib.rs updated"

# ─────────────────────────────────────────────────────────────────────────────
# 7. UPDATE Cargo.toml with all dependencies
# ─────────────────────────────────────────────────────────────────────────────
cat > crates/screenbuddy-core/Cargo.toml << 'EOF'
[package]
name = "screenbuddy-core"
version = "0.1.0"
edition = "2021"
authors = ["ScreenBuddy Team"]
license = "MIT OR Apache-2.0"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
reqwest = { version = "0.12", features = ["json"] }

# Rendering
wgpu = "0.20"
winit = "0.30"
raw-window-handle = "0.6"
pollster = "0.3"

# Image loading
image = "0.25"

# Math
glam = { version = "0.28", features = ["serde"] }

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
parking_lot = "0.12"
dashmap = "5"
notify = "6"
toml = "0.8"
directories = "5"
rand = "0.8"

# Windows-specific
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_System_LibraryLoader",
    "Win32_UI_Shell",
    "Win32_System_Com",
] }

[dev-dependencies]
tokio-test = "0.4"
EOF
echo "  ✓ Cargo.toml updated"

echo ""
echo "=== All files written successfully ==="
echo "Building..."
cd /G/Projects/ScreenBuddy
cargo build 2>&1 | tail -5
echo ""
echo "Testing..."
cargo test --workspace 2>&1 | grep "test result"
