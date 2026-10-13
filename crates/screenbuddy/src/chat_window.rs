//! Chat Window Overlay for ScreenBuddy
//!
//! A transparent layered window with message history display and input field.
//! Uses GDI (TextOutW, DrawTextW) for text rendering and UpdateLayeredWindow
//! for transparent compositing.

use std::mem;
use std::ptr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Chat window state
pub struct ChatWindow {
    hwnd: Option<isize>,
    visible: bool,
    messages: Vec<(String, String, String)>, // (role, text, timestamp)
    input_buffer: String,
    scroll_offset: usize,
    typing: bool,
    typing_frame: u32,
    send_btn_rect: Option<(i32, i32, i32, i32)>,
    btn_hover: bool,
    event_sender: Sender<ChatWindowEvent>,
    event_receiver: Arc<Mutex<Receiver<ChatWindowEvent>>>,
}

/// Events from the chat window
// Close/Toggle are dispatched by the Win32 wndproc but not yet consumed by the
// main loop, which currently polls the window state directly.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ChatWindowEvent {
    Input(String),
    Close,
    Toggle,
}

impl ChatWindow {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            hwnd: None,
            visible: false,
            messages: Vec::new(),
            input_buffer: String::new(),
            scroll_offset: 0,
            typing: false,
            typing_frame: 0,
            send_btn_rect: None,
            btn_hover: false,
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(rx)),
        }
    }

    #[allow(dead_code)] // part of the public chat API; main loop polls instead
    pub fn event_sender(&self) -> Sender<ChatWindowEvent> {
        self.event_sender.clone()
    }

    pub fn poll_event(&self) -> Option<ChatWindowEvent> {
        self.event_receiver.lock().unwrap().try_recv().ok()
    }

    pub fn add_message(&mut self, role: &str, text: &str) {
        let ts = chrono::Local::now().format("%H:%M").to_string();
        self.messages.push((role.to_string(), text.to_string(), ts));
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
        self.scroll_offset = 0;
        self.invalidate();
    }

    pub fn messages(&self) -> &[(String, String, String)] {
        &self.messages
    }

    #[allow(dead_code)]
    pub fn set_typing(&mut self, typing: bool) {
        self.typing = typing;
        self.typing_frame = 0;
        self.invalidate();
    }

    #[allow(dead_code)] // public chat API
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        self.visible = true;
        if let Some(hwnd) = self.hwnd {
            unsafe {
                winapi::um::winuser::ShowWindow(hwnd as *mut _, winapi::um::winuser::SW_SHOW);
            }
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
        if let Some(hwnd) = self.hwnd {
            unsafe {
                winapi::um::winuser::ShowWindow(hwnd as *mut _, winapi::um::winuser::SW_HIDE);
            }
        }
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn set_hwnd(&mut self, hwnd: isize) {
        self.hwnd = Some(hwnd);
    }

    #[allow(dead_code)] // part of the public chat API
    pub fn hwnd(&self) -> Option<isize> {
        self.hwnd
    }

    pub fn append_input(&mut self, ch: char) {
        self.input_buffer.push(ch);
        self.invalidate();
    }

    pub fn backspace(&mut self) {
        self.input_buffer.pop();
        self.invalidate();
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.invalidate();
    }

    pub fn input_buffer(&self) -> &str {
        &self.input_buffer
    }

    fn invalidate(&self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                winapi::um::winuser::InvalidateRect(hwnd as *mut _, ptr::null(), 1);
            }
        }
    }
}

// ============================================================================
// Windows Platform Implementation
// ============================================================================
#[cfg(windows)]
pub mod platform {
    use super::*;
    use std::ffi::c_void;
    use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
    use winapi::shared::windef::{HWND, POINT, RECT, SIZE};
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    const CHAT_WIDTH: i32 = 400;
    const CHAT_HEIGHT: i32 = 600;
    const INPUT_HEIGHT: i32 = 36;
    const PADDING: i32 = 10;
    const LINE_GAP: i32 = 2;

    /// Create a chat window (Windows-specific)
    pub fn create_chat_window(chat: &mut ChatWindow) -> Result<isize, String> {
        unsafe {
            let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
            let class_name: Vec<u16> = "ScreenBuddyChat\0".encode_utf16().collect();
            let window_title: Vec<u16> = "ScreenBuddy Chat\0".encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(chat_window_proc),
                hInstance: h_instance,
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..mem::zeroed()
            };
            RegisterClassExW(&wc);

            let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                window_title.as_ptr(),
                WS_POPUP,
                500,
                100,
                CHAT_WIDTH,
                CHAT_HEIGHT,
                ptr::null_mut(),
                ptr::null_mut(),
                h_instance,
                chat as *mut ChatWindow as *mut winapi::ctypes::c_void,
            );

            if hwnd.is_null() {
                return Err("Failed to create chat window".to_string());
            }

            chat.set_hwnd(hwnd as isize);

            // Initial render
            InvalidateRect(hwnd, ptr::null(), 1);
            Ok(hwnd as isize)
        }
    }

    /// Chat window procedure with GDI text rendering
    unsafe extern "system" fn chat_window_proc(
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => {
                let lpcs = lparam as *const CREATESTRUCTW;
                let chat = (*lpcs).lpCreateParams as *mut ChatWindow;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, chat as isize);
                0
            }
            WM_PAINT => {
                let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if chat.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                render_chat_window(hwnd, &*chat);
                0
            }
            WM_CHAR => {
                let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if chat.is_null() {
                    return 0;
                }
                let ch = std::char::from_u32(wparam as u32).unwrap_or('\0');
                if ch >= ' ' && ch != '\x7f' {
                    (*chat).append_input(ch);
                }
                0
            }
            WM_KEYDOWN => {
                let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if chat.is_null() {
                    return 0;
                }
                match wparam as i32 {
                    VK_RETURN => {
                        // Shift+Enter inserts a newline; Enter sends.
                        let shift_held = unsafe { GetKeyState(VK_SHIFT) < 0 };
                        if shift_held {
                            (*chat).input_buffer.push('\n');
                        } else {
                            let text = (*chat).input_buffer.trim().to_string();
                            if !text.is_empty() {
                                (*chat).event_sender.send(ChatWindowEvent::Input(text)).ok();
                                (*chat).clear_input();
                            }
                        }
                        0
                    }
                    VK_BACK => {
                        (*chat).backspace();
                        0
                    }
                    VK_ESCAPE => {
                        ShowWindow(hwnd, SW_HIDE);
                        (*chat).hide();
                        0
                    }
                    0x43 if unsafe { GetKeyState(VK_CONTROL) < 0 } => {
                        if let Some((_, text, _)) = (*chat).messages.last() {
                            let _ = clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text);
                        }
                        0
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_MOUSEWHEEL => {
                let chat_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if !chat_ptr.is_null() {
                    let delta = ((wparam >> 16) as i16) as i32;
                    let lines = 3;
                    unsafe {
                        if delta > 0 {
                            (*chat_ptr).scroll_offset = (*chat_ptr).scroll_offset.saturating_sub(lines);
                        } else {
                            (*chat_ptr).scroll_offset = ((*chat_ptr).scroll_offset + lines)
                                .min((*chat_ptr).messages.len().saturating_sub(1));
                        }
                    }
                }
                0
            }
            WM_MOUSEMOVE => {
                let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if chat.is_null() { return 0; }
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                if let Some((rx, ry, rw, rh)) = (*chat).send_btn_rect {
                    let hovering = x >= rx && x < rx + rw && y >= ry && y < ry + rh;
                    if hovering != (*chat).btn_hover {
                        (*chat).btn_hover = hovering;
                        (*chat).invalidate();
                    }
                }
                0
            }
            WM_LBUTTONDOWN => {
                let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
                if chat.is_null() { return 0; }
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                if let Some((rx, ry, rw, rh)) = (*chat).send_btn_rect {
                    if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
                        let text = (*chat).input_buffer.trim().to_string();
                        if !text.is_empty() {
                            (*chat).event_sender.send(ChatWindowEvent::Input(text)).ok();
                            (*chat).clear_input();
                        }
                        return 0;
                    }
                }
                0
            }
            WM_ERASEBKGND => 1,
            WM_DPICHANGED => {
                let rect = *(lparam as *const RECT);
                unsafe {
                    SetWindowPos(hwnd, ptr::null_mut(),
                        rect.left, rect.top,
                        rect.right - rect.left, rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE);
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Main render function: draws background, messages, and input field using GDI
    unsafe fn render_chat_window(hwnd: HWND, chat: &ChatWindow) {
        // Create DIBSection for layered window compositing
        let hdc_screen = GetDC(ptr::null_mut());
        let hdc_mem = CreateCompatibleDC(hdc_screen);

        let mut bmi: BITMAPINFO = mem::zeroed();
        bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = CHAT_WIDTH;
        bmi.bmiHeader.biHeight = -CHAT_HEIGHT;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut pixel_data: *mut c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(
            hdc_screen,
            &bmi,
            DIB_RGB_COLORS,
            &mut pixel_data as *mut _ as *mut _,
            ptr::null_mut(),
            0,
        );
        ReleaseDC(ptr::null_mut(), hdc_screen);

        if hbitmap.is_null() {
            DeleteDC(hdc_mem);
            return;
        }

        let old_bitmap = SelectObject(hdc_mem, hbitmap as *mut _);

        // Fill background with semi-transparent dark color (BGRA)
        let bg_pixel: u32 = 0xDC_1C_1C_1E; // A=220, R=30, G=30, B=30
        let total = (CHAT_WIDTH * CHAT_HEIGHT) as usize;
        let bg_ptr = pixel_data as *mut u32;
        for i in 0..total {
            *bg_ptr.add(i) = bg_pixel;
        }

        // Create font for text
        let font_name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
        let hfont = CreateFontW(
            16,
            0,
            0,
            0,
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            VARIABLE_PITCH,
            font_name.as_ptr(),
        );

        let old_font = SelectObject(hdc_mem, hfont as *mut _);
        SetBkMode(hdc_mem, TRANSPARENT as i32);

        // --- Draw message history (bottom-up) ---
        let msg_area_bottom = CHAT_HEIGHT - INPUT_HEIGHT - PADDING;
        let mut y = msg_area_bottom;

        let visible_msgs: Vec<_> = chat.messages.iter().rev().skip(chat.scroll_offset).collect();
        for (role, text, ts) in visible_msgs {
            if y < PADDING {
                break;
            }
            // Draw timestamp in muted color
            SetTextColor(hdc_mem, RGB(150, 150, 170));
            let ts_wide: Vec<u16> = ts.encode_utf16().chain(std::iter::once(0)).collect();
            let mut ts_rect = RECT { left: PADDING, top: y - 14, right: PADDING + 40, bottom: y };
            DrawTextW(hdc_mem, ts_wide.as_ptr(), -1, &mut ts_rect, DT_LEFT | DT_SINGLELINE);

            let label = if role == "user" {
                "You"
            } else if role == "assistant" {
                "AI"
            } else {
                role
            };
            let full_text = format!("{}: {}", label, text);

            // Color per role
            let color = if role == "user" {
                RGB(150, 200, 255)
            } else if role == "assistant" {
                RGB(150, 255, 150)
            } else {
                RGB(200, 200, 200)
            };
            SetTextColor(hdc_mem, color);

            let wide: Vec<u16> = full_text.encode_utf16().chain(std::iter::once(0)).collect();

            // Measure text bounds
            let mut measure_rect = RECT {
                left: PADDING,
                top: 0,
                right: CHAT_WIDTH - PADDING,
                bottom: 0,
            };
            DrawTextW(
                hdc_mem,
                wide.as_ptr(),
                wide.len() as i32,
                &mut measure_rect,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );

            let text_height = measure_rect.bottom - measure_rect.top;
            if text_height <= 0 {
                continue;
            }

            y -= text_height;

            let mut draw_rect = RECT {
                left: PADDING,
                top: y,
                right: CHAT_WIDTH - PADDING,
                bottom: y + text_height,
            };
            DrawTextW(
                hdc_mem,
                wide.as_ptr(),
                wide.len() as i32,
                &mut draw_rect,
                DT_WORDBREAK | DT_NOPREFIX,
            );

            y -= LINE_GAP;
        }

        // --- Draw input field at bottom ---
        let input_top = CHAT_HEIGHT - INPUT_HEIGHT;

        // Draw input field background (slightly lighter)
        let input_rect = RECT {
            left: PADDING,
            top: input_top,
            right: CHAT_WIDTH - PADDING,
            bottom: CHAT_HEIGHT - PADDING,
        };

        // Fill input area
        let input_bg: u32 = 0xAA_22_22_33; // darker, more transparent
        let input_buf = pixel_data as *mut u32;
        for row in input_top..(CHAT_HEIGHT - PADDING) {
            for col in PADDING..(CHAT_WIDTH - PADDING) {
                let idx = (row * CHAT_WIDTH + col) as usize;
                if idx < total {
                    *input_buf.add(idx) = input_bg;
                }
            }
        }

        // Draw border using FrameRect
        let border_brush = CreateSolidBrush(RGB(100, 100, 130));
        FrameRect(hdc_mem, &input_rect, border_brush);
        DeleteObject(border_brush as _);

        // Draw typing indicator
        if chat.typing {
            let dots = match (chat.typing_frame / 10) % 3 {
                0 => ".",
                1 => "..",
                _ => "...",
            };
            SetTextColor(hdc_mem, RGB(150, 255, 150));
            let dots_wide: Vec<u16> = dots.encode_utf16().chain(std::iter::once(0)).collect();
            let mut dots_rect = RECT { left: PADDING, top: y - 20, right: PADDING + 60, bottom: y };
            DrawTextW(hdc_mem, dots_wide.as_ptr(), -1, &mut dots_rect, DT_LEFT | DT_SINGLELINE);
        }

        // Draw send button
        let btn_w = 60i32;
        let btn_h = 24i32;
        let btn_x = CHAT_WIDTH - PADDING - btn_w;
        let btn_y = input_top + (INPUT_HEIGHT - btn_h) / 2;
        let has_input = !chat.input_buffer().is_empty();
        let btn_color: u32 = if !has_input {
            0xAA_55_55_55
        } else if chat.btn_hover {
            0xFF_6A_B0_F9
        } else {
            0xFF_4A_90_D9
        };
        let btn_buf = pixel_data as *mut u32;
        for row in btn_y..(btn_y + btn_h) {
            for col in btn_x..(btn_x + btn_w) {
                let idx = (row * CHAT_WIDTH + col) as usize;
                if idx < total {
                    *btn_buf.add(idx) = btn_color;
                }
            }
        }
        let btn_rect = RECT { left: btn_x, top: btn_y, right: btn_x + btn_w, bottom: btn_y + btn_h };
        let btn_border = CreateSolidBrush(RGB(100, 150, 220));
        FrameRect(hdc_mem, &btn_rect, btn_border);
        DeleteObject(btn_border as _);
        SetTextColor(hdc_mem, if has_input { RGB(255, 255, 255) } else { RGB(150, 150, 150) });
        let label = "Send";
        let wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
        let mut btn_text_rect = RECT { left: btn_x, top: btn_y, right: btn_x + btn_w, bottom: btn_y + btn_h };
        DrawTextW(hdc_mem, wide.as_ptr(), -1, &mut btn_text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        // Store rect for hit testing
        unsafe {
            let chat = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ChatWindow;
            if !chat.is_null() {
                (*chat).send_btn_rect = Some((btn_x, btn_y, btn_w, btn_h));
            }
        }

        // Draw input text
        SetTextColor(hdc_mem, RGB(255, 255, 255));
        let text_rect = RECT {
            left: PADDING + 4,
            top: input_top + 2,
            right: CHAT_WIDTH - PADDING - 4,
            bottom: CHAT_HEIGHT - PADDING - 2,
        };

        if !chat.input_buffer().is_empty() {
            let wide: Vec<u16> = chat
                .input_buffer()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            DrawTextW(
                hdc_mem,
                wide.as_ptr(),
                wide.len() as i32,
                &mut text_rect.clone(),
                DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_NOCLIP,
            );

            // Draw cursor after text
            let mut cursor_measure = text_rect;
            DrawTextW(
                hdc_mem,
                wide.as_ptr(),
                wide.len() as i32 - 1, // exclude null terminator
                &mut cursor_measure,
                DT_CALCRECT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
            let cursor_x = PADDING + 4 + (cursor_measure.right - cursor_measure.left).max(0);
            let cursor_top = input_top + 4;
            let cursor_bot = CHAT_HEIGHT - PADDING - 4;

            let cursor_pen = CreatePen(PS_SOLID as i32, 1, RGB(200, 200, 200));
            let old_pen = SelectObject(hdc_mem, cursor_pen as *mut _);
            MoveToEx(hdc_mem, cursor_x, cursor_top, ptr::null_mut());
            LineTo(hdc_mem, cursor_x, cursor_bot);
            SelectObject(hdc_mem, old_pen);
            DeleteObject(cursor_pen as _);
        }

        // Cleanup GDI objects
        SelectObject(hdc_mem, old_font);
        DeleteObject(hfont as _);
        SelectObject(hdc_mem, old_bitmap);

        // --- Composite to layered window ---
        let mut blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA,
        };
        let mut size = SIZE {
            cx: CHAT_WIDTH,
            cy: CHAT_HEIGHT,
        };
        let mut src_pt = POINT { x: 0, y: 0 };
        let mut dest_rect: RECT = mem::zeroed();
        GetWindowRect(hwnd, &mut dest_rect);
        let mut dest_pt = POINT {
            x: dest_rect.left,
            y: dest_rect.top,
        };

        UpdateLayeredWindow(
            hwnd,
            ptr::null_mut(),
            &mut dest_pt,
            &mut size,
            hdc_mem,
            &mut src_pt,
            0,
            &mut blend,
            ULW_ALPHA,
        );

        // Cleanup
        DeleteObject(hbitmap as _);
        DeleteDC(hdc_mem);
    }
}

/// Re-export for callers
#[cfg(windows)]
pub use platform::create_chat_window;

/// Run chat window message loop
///
/// The main loop drives its own message pump, so this standalone entry point is
/// retained for embedding ScreenBuddy's chat window in another host.
#[cfg(windows)]
#[allow(dead_code)]
pub fn run_chat_message_loop() {
    unsafe {
        let mut msg: winapi::um::winuser::MSG = mem::zeroed();
        while winapi::um::winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            winapi::um::winuser::TranslateMessage(&msg);
            winapi::um::winuser::DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_window_new() {
        let cw = ChatWindow::new();
        assert!(!cw.is_visible());
    }

    #[test]
    fn test_chat_window_add_message() {
        let mut cw = ChatWindow::new();
        cw.add_message("user", "Hello");
        cw.add_message("assistant", "Hi!");
        assert_eq!(cw.messages().len(), 2);
    }

    #[test]
    fn test_chat_window_limit() {
        let mut cw = ChatWindow::new();
        for i in 0..60 {
            cw.add_message("user", &format!("msg {}", i));
        }
        assert_eq!(cw.messages().len(), 50);
    }

    #[test]
    fn test_chat_window_toggle() {
        let mut cw = ChatWindow::new();
        assert!(!cw.is_visible());
        cw.toggle();
        assert!(cw.is_visible());
        cw.toggle();
        assert!(!cw.is_visible());
    }

    #[test]
    fn test_chat_window_show_hide() {
        let mut cw = ChatWindow::new();
        cw.show();
        assert!(cw.is_visible());
        cw.hide();
        assert!(!cw.is_visible());
    }

    #[test]
    fn test_chat_window_event_sender() {
        let cw = ChatWindow::new();
        let sender = cw.event_sender();
        sender.send(ChatWindowEvent::Input("test".to_string())).ok();
        match cw.poll_event() {
            Some(ChatWindowEvent::Input(text)) => assert_eq!(text, "test"),
            _ => panic!("Expected Input event"),
        }
    }

    #[test]
    fn test_chat_window_input() {
        let mut cw = ChatWindow::new();
        cw.append_input('H');
        cw.append_input('i');
        assert_eq!(cw.input_buffer(), "Hi");
        cw.backspace();
        assert_eq!(cw.input_buffer(), "H");
        cw.clear_input();
        assert_eq!(cw.input_buffer(), "");
    }
}
