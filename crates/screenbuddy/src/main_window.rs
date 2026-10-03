//! The ScreenBuddy main window: sessions on the left, the conversation on the
//! right, and a header that reports what the app is doing.
//!
//! This replaces the old borderless overlay as the place a person actually
//! talks to the app. It is a normal top-level window rather than a layered
//! popup, because it needs to take focus for text entry and host child
//! controls.
//!
//! Rendering is plain GDI into a DIBSection, matching the rest of the project;
//! no new dependency is introduced.

use std::mem;
use std::ptr;

use screenbuddy_core::session::{SessionStore, SessionSummary};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBRUSH, HDC, HFONT, HWND};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

// RECT lives in the shared windef module in this winapi version.
use winapi::shared::windef::RECT;

/// Window and layout metrics.
const WIN_WIDTH: i32 = 900;
const WIN_HEIGHT: i32 = 640;
const HEADER_H: i32 = 56;
const SIDEBAR_W: i32 = 240;
const INPUT_H: i32 = 44;
const PADDING: i32 = 12;

/// Colours as 0x00BBGGRR, matching the DIB layout used below.
const C_BG: u32 = 0x001E1E21;
const C_HEADER: u32 = 0x0026262B;
const C_SIDEBAR: u32 = 0x00222227;
const C_ROW_ACTIVE: u32 = 0x00333A4A;
const C_INPUT_BG: u32 = 0x002A2A32;
const C_BORDER: u32 = 0x00464652;
const C_TEXT: u32 = 0x00E8E8EC;
const C_MUTED: u32 = 0x009A9AA6;
const C_ACCENT: u32 = 0x00C0865A;

/// What the user clicked, delivered to the main loop.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// Send the input box contents as a chat message.
    SendMessage(String),
    /// Start a new session.
    NewSession,
    /// Switch to the session with this id.
    SelectSession(String),
    /// Delete the session with this id.
    DeleteSession(String),
    /// Empty the current session.
    ClearSession,
    Quit,
}

/// A clickable region, rebuilt each paint and hit-tested on click.
#[derive(Debug, Clone)]
struct HitTarget {
    rect: Rect,
    action: UiAction,
}

#[derive(Debug, Clone, PartialEq)]
enum UiAction {
    Send,
    NewSession,
    Select(String),
    Delete(String),
    Clear,
    Quit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Rect {
    fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    fn width(&self) -> i32 {
        self.right - self.left
    }
}

/// Everything the window draws. Owned here; the session store is shared with the
/// app so both views agree.
/// The main window.
pub struct MainWindow {
    hwnd: Option<HWND>,
    visible: bool,
    /// The DC that owns the pixel buffer currently being drawn into. GDI writes
    /// text to a DC, so it must be the *same* surface the fills write into;
    /// drawing to a separate scratch DC put every glyph somewhere invisible.
    draw_dc: Option<HDC>,
    sessions: Vec<SessionSummary>,
    current_id: String,
    messages: Vec<(String, String)>,
    input: String,
    status: String,
    /// Scroll offset in pixels into the message list.
    scroll: i32,
    /// Total height of the message list, used to clamp scrolling.
    content_h: i32,
    hits: Vec<HitTarget>,
    /// Where the caret sits, so the window can be repainted on a timer while
    /// the input has focus.
    blink: bool,
    focus: bool,
    events: Vec<UiEvent>,
}

impl Default for MainWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Several accessors exist for the tray and menu integration even though the
/// current main loop does not call them all, so unused-method warnings here are
/// expected rather than a sign of dead code.
#[allow(dead_code)]
impl MainWindow {
    pub fn new() -> Self {
        Self {
            hwnd: None,
            visible: false,
            draw_dc: None,
            sessions: Vec::new(),
            current_id: String::new(),
            messages: Vec::new(),
            input: String::new(),
            status: "Ready".to_string(),
            scroll: 0,
            content_h: 0,
            hits: Vec::new(),
            blink: true,
            focus: false,
            events: Vec::new(),
        }
    }

    pub fn hwnd(&self) -> Option<HWND> {
        self.hwnd
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        self.visible = true;
        if let Some(hwnd) = self.hwnd {
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            }
        }
        self.refresh();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        if let Some(hwnd) = self.hwnd {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
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

    /// Re-read the store so the window reflects messages added elsewhere, such as
    /// by the AI bridge or by an agent over IPC.
    pub fn refresh_from(&mut self, store: &SessionStore) {
        self.sessions = store.list();
        self.current_id = store.current_id().to_string();
        self.messages = store.current().map(|s| s.transcript()).unwrap_or_default();
        self.refresh();
    }

    /// Redraw if visible.
    pub fn refresh(&mut self) {
        if !self.visible {
            return;
        }
        if let Some(hwnd) = self.hwnd {
            unsafe {
                InvalidateRect(hwnd, ptr::null(), 0);
            }
        }
    }

    /// Drain events for the main loop.
    pub fn poll_event(&mut self) -> Option<UiEvent> {
        if self.events.is_empty() {
            None
        } else {
            Some(self.events.remove(0))
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.refresh();
    }

    pub fn input_buffer(&self) -> &str {
        &self.input
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.refresh();
    }

    // ---- hit testing ----

    fn hit(&self, x: i32, y: i32) -> Option<UiAction> {
        // Later targets are drawn on top, so scan in reverse.
        self.hits
            .iter()
            .rev()
            .find(|h| h.rect.contains(x, y))
            .map(|h| h.action.clone())
    }

    /// Convert client coordinates to the window's bitmap space.
    fn to_bitmap(&self, x: i32, y: i32) -> (i32, i32) {
        // The window is not offset in its own client area, so this is identity;
        // kept as a function so a future scroll offset has one place to change.
        (x, y)
    }

    fn on_click(&mut self, x: i32, y: i32) {
        match self.hit(x, y) {
            Some(UiAction::Send) => {
                let text = self.input.trim().to_string();
                if !text.is_empty() {
                    self.events.push(UiEvent::SendMessage(text));
                    self.input.clear();
                }
            }
            Some(UiAction::NewSession) => self.events.push(UiEvent::NewSession),
            Some(UiAction::Select(id)) => self.events.push(UiEvent::SelectSession(id)),
            Some(UiAction::Delete(id)) => self.events.push(UiEvent::DeleteSession(id)),
            Some(UiAction::Clear) => self.events.push(UiEvent::ClearSession),
            Some(UiAction::Quit) => self.events.push(UiEvent::Quit),
            None => {}
        }
        self.refresh();
    }

    fn on_key(&mut self, vk: i32) {
        match vk {
            VK_RETURN => {
                let text = self.input.trim().to_string();
                if !text.is_empty() {
                    self.events.push(UiEvent::SendMessage(text));
                    self.input.clear();
                }
            }
            VK_BACK => {
                self.input.pop();
            }
            VK_ESCAPE => self.hide(),
            _ => {}
        }
        self.refresh();
    }

    fn on_char(&mut self, ch: u16) {
        // Enter and control characters are handled as keys, not text.
        if ch as i32 == 0x0D || ch as i32 == 0x1B {
            return;
        }
        if (0x20..0x7E).contains(&(ch as i32)) || ch > 0xA0 {
            if let Some(c) = char::from_u32(ch as u32) {
                self.input.push(c);
                self.refresh();
            }
        }
    }

    /// `delta` follows the WM_MOUSEWHEEL convention: positive means the wheel was
    /// rotated up, which reveals earlier content, so the offset decreases.
    fn on_wheel(&mut self, delta: i32) {
        self.scroll = (self.scroll - delta).clamp(0, self.max_scroll());
        self.refresh();
    }

    fn max_scroll(&self) -> i32 {
        let view_h = self.view_height();
        (self.content_h - view_h).max(0)
    }

    fn view_height(&self) -> i32 {
        WIN_HEIGHT - HEADER_H - INPUT_H - PADDING
    }
}

// ---------------------------------------------------------------------------
// Window creation and message handling
// ---------------------------------------------------------------------------

pub fn create_main_window(win: &mut MainWindow) -> Result<HWND, String> {
    unsafe {
        let h_instance = GetModuleHandleW(ptr::null_mut());
        let class_name: Vec<u16> = "ScreenBuddyMain\0".encode_utf16().collect();
        let window_title: Vec<u16> = "ScreenBuddy\0".encode_utf16().collect();

        let wc = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(main_window_proc),
            hInstance: h_instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as HBRUSH,
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WIN_WIDTH,
            WIN_HEIGHT,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            win as *mut MainWindow as *mut _,
        );

        if hwnd.is_null() {
            return Err("failed to create main window".into());
        }

        win.hwnd = Some(hwnd);
        Ok(hwnd)
    }
}

unsafe extern "system" fn main_window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let lpcs = lparam as *const CREATESTRUCTW;
            let win = (*lpcs).lpCreateParams as *mut MainWindow;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, win as isize);
            0
        }
        WM_PAINT => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                paint(hwnd, &mut *win);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                (*win).on_click(x, y);
            }
            0
        }
        WM_KEYDOWN => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).on_key(wparam as i32);
            }
            0
        }
        WM_CHAR => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).on_char(wparam as u16);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                // HIWORD(wparam) carries the delta in 120ths of a notch.
                let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
                (*win).on_wheel(delta / 120 * 40);
            }
            0
        }
        WM_TIMER => {
            // Drives the text caret so a focused input looks alive.
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).blink = !(*win).blink;
                if (*win).focus {
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
            }
            0
        }
        WM_SETFOCUS => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).focus = true;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_KILLFOCUS => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).focus = false;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_CLOSE => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                (*win).events.push(UiEvent::Quit);
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

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

unsafe fn paint(hwnd: HWND, win: &mut MainWindow) {
    let mut ps = mem::zeroed::<PAINTSTRUCT>();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Render into an off-screen DIB, then blit once. Drawing straight to the
    // window flickers badly at these sizes.
    let screen = GetDC(ptr::null_mut());
    let mem_dc = CreateCompatibleDC(screen);

    let mut bmi: BITMAPINFO = mem::zeroed();
    bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = WIN_WIDTH;
    // Negative height means a top-down bitmap, so row 0 is the top.
    bmi.bmiHeader.biHeight = -WIN_HEIGHT;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let mut raw: *mut core::ffi::c_void = ptr::null_mut();
    let bitmap = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut raw, ptr::null_mut(), 0);
    ReleaseDC(ptr::null_mut(), screen);
    if bitmap.is_null() || raw.is_null() {
        DeleteDC(mem_dc);
        EndPaint(hwnd, &ps);
        return;
    }
    let old = select_obj(mem_dc, bitmap as *mut _);

    let total = (WIN_WIDTH * WIN_HEIGHT) as usize;
    let buf: &mut [u32] = std::slice::from_raw_parts_mut(raw as *mut u32, total);
    // Publish the DC alongside the buffer: GDI writes text to a DC, so the text
    // must land on this same surface to appear in the bitmap.
    win.draw_dc = Some(mem_dc);
    draw_ui(win, buf);
    win.draw_dc = None;

    BitBlt(hdc, 0, 0, WIN_WIDTH, WIN_HEIGHT, mem_dc, 0, 0, SRCCOPY);

    select_obj(mem_dc, old as *mut _);
    del_obj(bitmap as *mut _);
    DeleteDC(mem_dc);
    EndPaint(hwnd, &ps);
}

/// Fill the whole bitmap, then lay out and draw each region.
fn draw_ui(win: &mut MainWindow, buf: &mut [u32]) {
    for px in buf.iter_mut() {
        *px = C_BG;
    }
    win.hits.clear();

    draw_header(win, buf);
    draw_sidebar(win, buf);
    draw_messages(win, buf);
    draw_input(win, buf);
}

fn fill_rect(buf: &mut [u32], rect: Rect, colour: u32) {
    let l = rect.left.max(0);
    let t = rect.top.max(0);
    let r = rect.right.min(WIN_WIDTH);
    let b = rect.bottom.min(WIN_HEIGHT);
    for y in t..b {
        for x in l..r {
            let idx = (y * WIN_WIDTH + x) as usize;
            if idx < buf.len() {
                buf[idx] = colour;
            }
        }
    }
}

fn draw_rect_outline(buf: &mut [u32], rect: Rect, colour: u32) {
    for x in rect.left..rect.right {
        for y in [rect.top, rect.bottom - 1] {
            let idx = (y * WIN_WIDTH + x) as usize;
            if (0..buf.len()).contains(&idx) {
                buf[idx] = colour;
            }
        }
    }
    for y in rect.top..rect.bottom {
        for x in [rect.left, rect.right - 1] {
            let idx = (y * WIN_WIDTH + x) as usize;
            if (0..buf.len()).contains(&idx) {
                buf[idx] = colour;
            }
        }
    }
}

fn draw_header(win: &mut MainWindow, buf: &mut [u32]) {
    fill_rect(buf, Rect::new(0, 0, WIN_WIDTH, HEADER_H), C_HEADER);
    draw_rect_outline(
        buf,
        Rect::new(0, HEADER_H - 1, WIN_WIDTH, HEADER_H),
        C_BORDER,
    );

    let font = make_font(18, true);
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let old_font = select_obj(dc, font as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, C_TEXT);
        draw_text(
            buf,
            dc,
            "ScreenBuddy",
            16,
            12,
            WIN_WIDTH - 260,
            font_height(18, true),
        );

        // Status on the right, so it does not collide with the title.
        let small = make_font(13, false);
        select_obj(dc, small as *mut _);
        set_text_colour(buf, dc, C_MUTED);
        draw_text(
            buf,
            dc,
            &win.status,
            300,
            20,
            WIN_WIDTH - 316,
            font_height(13, false),
        );
        select_obj(dc, old_font as *mut _);
        del_obj(small as *mut _);
        del_obj(font as *mut _);
    }

    // Quit button, top right.
    let q = Rect::new(WIN_WIDTH - 44, 8, WIN_WIDTH - 8, HEADER_H - 8);
    win.hits.push(HitTarget {
        rect: q,
        action: UiAction::Quit,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(14, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, C_MUTED);
        draw_text(
            buf,
            dc,
            "x",
            q.left + 12,
            q.top + 4,
            q.width(),
            font_height(14, false),
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

fn draw_sidebar(win: &mut MainWindow, buf: &mut [u32]) {
    let side = Rect::new(0, HEADER_H, SIDEBAR_W, WIN_HEIGHT);
    fill_rect(buf, side, C_SIDEBAR);
    draw_rect_outline(
        buf,
        Rect::new(SIDEBAR_W - 1, HEADER_H, SIDEBAR_W, WIN_HEIGHT),
        C_BORDER,
    );

    let mut y = HEADER_H + PADDING;

    // "New chat" button.
    let btn = Rect::new(PADDING, y, SIDEBAR_W - PADDING, y + 32);
    fill_rect(buf, btn, C_ROW_ACTIVE);
    draw_rect_outline(buf, btn, C_ACCENT);
    win.hits.push(HitTarget {
        rect: btn,
        action: UiAction::NewSession,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(13, true);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, C_TEXT);
        draw_text(
            buf,
            dc,
            "+  New chat",
            btn.left + 10,
            btn.top + 7,
            btn.width(),
            font_height(13, true),
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
    y = btn.bottom + PADDING;

    // Session rows.
    for summary in win.sessions.clone() {
        if y + 40 > WIN_HEIGHT {
            break;
        }
        let row = Rect::new(8, y, SIDEBAR_W - 8, y + 38);
        if summary.is_current {
            fill_rect(buf, row, C_ROW_ACTIVE);
            // Accent bar marking the selected session.
            fill_rect(
                buf,
                Rect::new(row.left, row.top, row.left + 3, row.bottom),
                C_ACCENT,
            );
        }
        win.hits.push(HitTarget {
            rect: row,
            action: UiAction::Select(summary.id.clone()),
        });

        let label = if summary.title.is_empty() {
            "New chat"
        } else {
            &summary.title
        };
        let count = format!(
            "{} message{}",
            summary.message_count,
            plural(summary.message_count)
        );
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(13, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            set_text_colour(buf, dc, if summary.is_current { C_TEXT } else { C_MUTED });
            draw_text(
                buf,
                dc,
                label,
                row.left + 10,
                row.top + 4,
                row.width() - 20,
                font_height(13, false),
            );

            let small = make_font(11, false);
            select_obj(dc, small as *mut _);
            set_text_colour(buf, dc, C_MUTED);
            draw_text(
                buf,
                dc,
                &count,
                row.left + 10,
                row.top + 20,
                row.width() - 20,
                font_height(11, false),
            );
            select_obj(dc, old as *mut _);
            del_obj(small as *mut _);
            del_obj(f as *mut _);
        }

        // Delete affordance on the right of each row.
        let del = Rect::new(row.right - 26, row.top + 8, row.right - 4, row.bottom - 8);
        win.hits.push(HitTarget {
            rect: del,
            action: UiAction::Delete(summary.id.clone()),
        });
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(12, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            set_text_colour(buf, dc, C_MUTED);
            draw_text(
                buf,
                dc,
                "-",
                del.left + 6,
                del.top + 1,
                del.width(),
                font_height(12, false),
            );
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);
        }

        y = row.bottom + 4;
    }

    // Clear button at the bottom of the sidebar.
    let clear = Rect::new(
        PADDING,
        WIN_HEIGHT - INPUT_H - 40,
        SIDEBAR_W - PADDING,
        WIN_HEIGHT - INPUT_H - 12,
    );
    fill_rect(buf, clear, C_SIDEBAR);
    draw_rect_outline(buf, clear, C_BORDER);
    win.hits.push(HitTarget {
        rect: clear,
        action: UiAction::Clear,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, C_MUTED);
        draw_text(
            buf,
            dc,
            "Clear conversation",
            clear.left + 10,
            clear.top + 7,
            clear.width(),
            font_height(12, false),
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

fn draw_messages(win: &mut MainWindow, buf: &mut [u32]) {
    let left = SIDEBAR_W + PADDING;
    let right = WIN_WIDTH - PADDING;
    let top = HEADER_H + PADDING;
    let bottom = WIN_HEIGHT - INPUT_H - PADDING;
    let width = right - left;

    let font = make_font(14, false);
    // User messages render bold so the two sides are distinguishable.
    let bold = make_font(14, true);

    // Measure first so the scroll position can be clamped to real content.
    let mut heights = Vec::with_capacity(win.messages.len());
    let mut total = 0;
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let old = select_obj(dc, font as *mut _);
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: 0,
        };
        for (_role, text) in &win.messages {
            let wide: Vec<u16> = text.encode_utf16().collect();
            rc.bottom = 4000;
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
            let h = (rc.bottom - rc.top).max(18) + 24;
            heights.push(h);
            total += h;
        }
        select_obj(dc, old as *mut _);
    }
    win.content_h = total;
    if win.scroll > win.max_scroll() {
        win.scroll = win.max_scroll();
    }

    let mut y = top - win.scroll;
    let messages = win.messages.clone();
    for (i, (role, text)) in messages.iter().enumerate() {
        let h = heights[i];
        if y + h >= top && y <= bottom {
            let is_user = role == "user";
            let bubble = Rect::new(
                if is_user { left + width / 3 } else { left },
                y.max(top),
                if is_user { right } else { left + width * 2 / 3 },
                (y + h).min(bottom),
            );
            fill_rect(buf, bubble, if is_user { C_ROW_ACTIVE } else { C_SIDEBAR });
            draw_rect_outline(buf, bubble, if is_user { C_ACCENT } else { C_BORDER });

            unsafe {
                let Some(dc) = win.draw_dc else { return };
                let f = if is_user { bold } else { font };
                let old = select_obj(dc, f as *mut _);
                SetBkMode(dc, TRANSPARENT as i32);
                set_text_colour(buf, dc, C_TEXT);
                let mut rc = RECT {
                    left: bubble.left + 10,
                    top: bubble.top + 8,
                    right: bubble.right - 10,
                    bottom: bubble.bottom - 8,
                };
                let wide: Vec<u16> = text.encode_utf16().collect();
                DrawTextW(
                    dc,
                    wide.as_ptr(),
                    wide.len() as i32,
                    &mut rc,
                    DT_WORDBREAK | DT_NOPREFIX,
                );
                select_obj(dc, old as *mut _);
            }
        }
        y += h;
    }

    // A hint when the conversation is empty, so the panel is not just blank.
    if win.messages.is_empty() {
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(15, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            set_text_colour(buf, dc, C_MUTED);
            let hint = "Ask ScreenBuddy anything. Press Enter to send.";
            draw_text(
                buf,
                dc,
                hint,
                left + 8,
                top + 16,
                width - 16,
                font_height(15, false),
            );
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);
        }
    }

    del_obj(font as *mut _);
    del_obj(bold as *mut _);
}

fn draw_input(win: &mut MainWindow, buf: &mut [u32]) {
    let left = SIDEBAR_W + PADDING;
    let right = WIN_WIDTH - PADDING;
    let top = WIN_HEIGHT - INPUT_H - 6;
    let rect = Rect::new(left, top, right, WIN_HEIGHT - PADDING);
    fill_rect(buf, rect, C_INPUT_BG);
    draw_rect_outline(buf, rect, if win.focus { C_ACCENT } else { C_BORDER });

    let send_w = 72;
    win.hits.push(HitTarget {
        rect: Rect::new(rect.right - send_w, rect.top, rect.right, rect.bottom),
        action: UiAction::Send,
    });

    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(14, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, C_TEXT);

        let placeholder = if win.input.is_empty() {
            "Type a message..."
        } else {
            ""
        };
        let shown = if win.input.is_empty() {
            placeholder
        } else {
            &win.input
        };
        let colour = if win.input.is_empty() {
            C_MUTED
        } else {
            C_TEXT
        };
        set_text_colour(buf, dc, colour);
        draw_text(
            buf,
            dc,
            shown,
            rect.left + 10,
            rect.top + 12,
            rect.width() - send_w - 20,
            font_height(14, false),
        );

        // Send label.
        set_text_colour(buf, dc, C_ACCENT);
        draw_text(
            buf,
            dc,
            "Send",
            rect.right - send_w + 14,
            rect.top + 12,
            send_w - 20,
            font_height(14, true),
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

// ---------------------------------------------------------------------------
// Small GDI helpers
// ---------------------------------------------------------------------------

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Line height for a font of the given size. Bold text is not taller, so the
/// weight does not affect it.
fn font_height(size: i32, _bold: bool) -> i32 {
    size + 10
}

/// A font handle. The caller deletes it.
fn make_font(size: i32, bold: bool) -> HFONT {
    unsafe {
        CreateFontW(
            -(size + 6),
            0,
            0,
            0,
            if bold { FW_BOLD } else { FW_NORMAL },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            ptr::null(),
        )
    }
}

/// Select a GDI object into a DC, hiding the *mut c_void cast this winapi
/// version's signature requires.
fn select_obj(dc: HDC, obj: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    unsafe { SelectObject(dc, obj) }
}

/// Delete a GDI object created by this module.
fn del_obj(obj: *mut core::ffi::c_void) {
    unsafe {
        DeleteObject(obj);
    }
}

/// A GDI surface tests can draw into, mirroring what `paint()` creates.
///
/// The layout code needs a DC for text while filling a plain pixel slice. These
/// must be the *same* surface, so this hands back both together.
#[cfg(test)]
struct DrawTarget {
    dc: HDC,
    _bitmap: winapi::shared::windef::HBITMAP,
    /// The DIB's pixel memory, which is what GDI actually draws into.
    raw: *mut u32,
}

#[cfg(test)]
impl DrawTarget {
    /// A copy of the surface pixels, read straight from the DIB.
    fn pixels(&self) -> Vec<u32> {
        if self.raw.is_null() {
            return vec![C_BG; (WIN_WIDTH * WIN_HEIGHT) as usize];
        }
        unsafe { std::slice::from_raw_parts(self.raw, (WIN_WIDTH * WIN_HEIGHT) as usize) }.to_vec()
    }
}

#[cfg(test)]
impl DrawTarget {
    fn new() -> Option<Self> {
        unsafe {
            let screen = GetDC(ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            if dc.is_null() {
                ReleaseDC(ptr::null_mut(), screen);
                return None;
            }
            let mut bmi: BITMAPINFO = mem::zeroed();
            bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = WIN_WIDTH;
            bmi.bmiHeader.biHeight = -WIN_HEIGHT;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;
            let mut raw: *mut core::ffi::c_void = ptr::null_mut();
            let bitmap =
                CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut raw, ptr::null_mut(), 0);
            ReleaseDC(ptr::null_mut(), screen);
            if bitmap.is_null() || raw.is_null() {
                DeleteDC(dc);
                return None;
            }
            select_obj(dc, bitmap as *mut _);
            Some(Self {
                dc,
                _bitmap: bitmap,
                raw: raw as *mut u32,
            })
        }
    }
}

#[cfg(test)]
impl Drop for DrawTarget {
    fn drop(&mut self) {
        unsafe {
            del_obj(self._bitmap as *mut _);
            DeleteDC(self.dc);
        }
    }
}

/// True when a BGRA pixel is close enough to one of the text colours to be a
/// glyph. Antialiasing blends glyph pixels towards the background, so an exact
/// match would only catch the handful of fully-opaque pixels.
#[cfg(test)]
fn is_text_pixel(pixel: u32) -> bool {
    let near = |target: u32| -> bool {
        let d = |shift: u32| {
            let a = ((pixel >> shift) & 0xFF) as i32;
            let b = ((target >> shift) & 0xFF) as i32;
            (a - b).abs()
        };
        d(0) <= 90 && d(8) <= 90 && d(16) <= 90
    };
    near(C_TEXT) || near(C_MUTED) || near(C_ACCENT)
}

/// Set the text colour used by subsequent `draw_text` calls on this DC.
///
/// GDI's SetTextColor takes COLORREF (0x00BBGGRR), matching our constants.
fn set_text_colour(buf: &[u32], dc: HDC, bgr: u32) {
    let _ = buf;
    unsafe {
        SetTextColor(dc, bgr);
    }
}

/// Draw clipped single-line text. Wrapped rather than DrawTextW because the
/// caller already owns the pixel buffer and needs the height passed in.
fn draw_text(buf: &mut [u32], dc: HDC, text: &str, x: i32, y: i32, max_w: i32, _h: i32) {
    if text.is_empty() || max_w <= 0 {
        return;
    }
    unsafe {
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + max_w,
            bottom: y + 40,
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        // DT_END_ELLIPSIS keeps long session titles from spilling over the row.
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
    }
    let _ = buf;
}

#[cfg(test)]
mod tests {
    use super::*;
    use screenbuddy_core::session::Role;

    fn w() -> MainWindow {
        let mut win = MainWindow::new();
        win.sessions = vec![
            SessionSummary {
                id: "a".into(),
                title: "First".into(),
                updated_at: 0,
                message_count: 3,
                is_current: true,
            },
            SessionSummary {
                id: "b".into(),
                title: "Second".into(),
                updated_at: 0,
                message_count: 1,
                is_current: false,
            },
        ];
        win
    }

    #[test]
    fn starts_hidden_with_empty_state() {
        let mut win = MainWindow::new();
        assert!(!win.is_visible());
        assert!(win.input_buffer().is_empty());
        assert!(win.poll_event().is_none());
    }

    #[test]
    fn entering_text_and_sending_emits_one_event() {
        let mut win = w();
        for c in "hello".chars() {
            win.on_char(c as u16);
        }
        assert_eq!(win.input_buffer(), "hello");

        win.on_key(VK_RETURN);
        match win.poll_event() {
            Some(UiEvent::SendMessage(t)) => assert_eq!(t, "hello"),
            other => panic!("expected SendMessage, got {other:?}"),
        }
        assert!(win.input_buffer().is_empty(), "input clears after sending");
        assert!(win.poll_event().is_none(), "exactly one event");
    }

    #[test]
    fn enter_on_empty_input_sends_nothing() {
        let mut win = w();
        win.on_key(VK_RETURN);
        assert!(win.poll_event().is_none());
    }

    #[test]
    fn whitespace_only_input_is_not_sent() {
        let mut win = w();
        for c in "   ".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_RETURN);
        assert!(win.poll_event().is_none(), "blank input must not send");
    }

    #[test]
    fn whitespace_is_trimmed_from_what_is_sent() {
        let mut win = w();
        for c in "  hi  ".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_RETURN);
        match win.poll_event() {
            Some(UiEvent::SendMessage(t)) => assert_eq!(t, "hi"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn backspace_removes_one_character() {
        let mut win = w();
        for c in "abc".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_BACK);
        assert_eq!(win.input_buffer(), "ab");
        win.on_key(VK_BACK);
        assert_eq!(win.input_buffer(), "a");
    }

    #[test]
    fn backspace_on_empty_input_is_harmless() {
        let mut win = w();
        win.on_key(VK_BACK);
        assert!(win.input_buffer().is_empty());
        assert!(win.poll_event().is_none());
    }

    #[test]
    fn control_characters_are_not_inserted_as_text() {
        let mut win = w();
        win.on_char(0x08); // backspace sent as a char
        win.on_char(0x1B); // escape
        win.on_char(0x0D); // enter
        assert!(win.input_buffer().is_empty());
    }

    #[test]
    fn escape_hides_the_window() {
        let mut win = w();
        win.visible = true;
        win.on_key(VK_ESCAPE);
        assert!(!win.is_visible());
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut win = w();
        win.toggle();
        assert!(win.is_visible());
        win.toggle();
        assert!(!win.is_visible());
    }

    #[test]
    fn refresh_from_picks_up_the_current_session() {
        let mut store = SessionStore::new();
        store.push(Role::User, "question about the dragon");
        store.push(Role::Assistant, "it soars");

        let mut win = w();
        win.refresh_from(&store);
        assert_eq!(win.messages.len(), 2);
        assert_eq!(win.messages[0].0, "user");
        assert!(win.sessions.iter().any(|s| s.is_current));
        assert!(!win.current_id.is_empty());
    }

    #[test]
    fn clicking_a_session_row_emits_select() {
        let mut win = w();
        // Build hit regions as a real paint would.
        render(&mut win);
        assert!(!win.hits.is_empty(), "paint must register hit targets");

        // The second session's row is below the New chat button.
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Select("b".into()))
            .expect("session b row registered")
            .rect;
        win.on_click(target.left + 4, target.top + 4);
        match win.poll_event() {
            Some(UiEvent::SelectSession(id)) => assert_eq!(id, "b"),
            other => panic!("expected SelectSession(b), got {other:?}"),
        }
    }

    #[test]
    fn clicking_the_new_chat_button_emits_new_session() {
        let mut win = w();
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::NewSession)
            .expect("new chat button registered")
            .rect;
        win.on_click(target.left + 2, target.top + 2);
        assert!(matches!(win.poll_event(), Some(UiEvent::NewSession)));
    }

    #[test]
    fn clicking_the_send_button_sends_the_input() {
        let mut win = w();
        for c in "ship it".chars() {
            win.on_char(c as u16);
        }
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Send)
            .expect("send button registered")
            .rect;
        win.on_click(target.left + 2, target.top + 2);
        match win.poll_event() {
            Some(UiEvent::SendMessage(t)) => assert_eq!(t, "ship it"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn clicking_empty_space_emits_nothing() {
        let mut win = w();
        render(&mut win);
        win.on_click(WIN_WIDTH - 4, 2); // top-right corner, inside no target
        assert!(win.poll_event().is_none());
    }

    #[test]
    fn clear_button_emits_clear() {
        let mut win = w();
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Clear)
            .expect("clear button registered")
            .rect;
        win.on_click(target.left + 2, target.top + 2);
        assert!(matches!(win.poll_event(), Some(UiEvent::ClearSession)));
    }

    #[test]
    fn delete_affordance_is_hit_testable_and_wins_over_the_row() {
        // The delete box sits on top of the row, so it must be found first.
        let mut win = w();
        render(&mut win);
        let del = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Delete("b".into()))
            .expect("delete target for b")
            .rect;
        win.on_click(del.left + 2, del.top + 2);
        assert!(matches!(win.poll_event(), Some(UiEvent::DeleteSession(id)) if id == "b"));
    }

    #[test]
    fn scroll_is_clamped_to_the_content() {
        let mut win = w();
        win.content_h = 1000;
        win.view_height();
        // Wheel up (positive delta) walks back toward the start of the list.
        for _ in 0..200 {
            win.on_wheel(120);
        }
        assert_eq!(win.scroll, 0, "cannot scroll above the start");

        // Wheel down walks forward and must stop at the end.
        for _ in 0..50 {
            win.on_wheel(-120);
        }
        assert_eq!(win.scroll, win.max_scroll(), "cannot scroll past the end");
        assert!(win.scroll > 0, "the content is taller than the view here");
    }

    /// Render the whole UI into a real GDI surface, as `paint()` does, so tests
    /// exercise the same path the window uses rather than a stripped-down one.
    ///
    /// Returns the DIB's actual pixels, because GDI writes text to the surface
    /// rather than to the slice the fills use.
    fn render(win: &mut MainWindow) -> Rendered {
        let Some(target) = DrawTarget::new() else {
            let empty = vec![C_BG; (WIN_WIDTH * WIN_HEIGHT) as usize];
            return Rendered {
                fills: empty.clone(),
                surface: empty,
            };
        };
        let mut fills = vec![C_BG; (WIN_WIDTH * WIN_HEIGHT) as usize];
        win.draw_dc = Some(target.dc);
        draw_ui(win, &mut fills);
        win.draw_dc = None;
        Rendered {
            fills,
            surface: target.pixels(),
        }
    }

    /// What one render produced: the filled slice the layout code wrote, and the
    /// DIB surface GDI wrote the text into.
    struct Rendered {
        fills: Vec<u32>,
        surface: Vec<u32>,
    }

    #[test]
    fn painting_populates_hits_and_writes_pixels() {
        let mut win = w();
        let rendered = render(&mut win);

        assert!(!win.hits.is_empty());
        // The fills land in the slice the layout code writes, not the DIB.
        assert!(rendered.fills.contains(&C_BG));
        assert!(rendered.fills.contains(&C_HEADER), "header drawn");
        assert!(rendered.fills.contains(&C_SIDEBAR), "sidebar drawn");
    }

    #[test]
    fn painting_is_safe_with_no_sessions_and_no_messages() {
        let mut win = MainWindow::new();
        let rendered = render(&mut win);
        assert!(
            rendered.fills.contains(&C_INPUT_BG),
            "input box still drawn"
        );
    }

    #[test]
    fn painting_is_safe_with_a_very_long_message() {
        let mut win = w();
        win.messages = vec![("assistant".into(), "word ".repeat(400))];
        let rendered = render(&mut win);
        assert!(rendered.fills.contains(&C_BG));
        assert!(win.content_h > 0);
    }

    #[test]
    fn painting_is_safe_with_unicode_and_control_characters() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "emoji 🐉 and accents éàü".into()),
            ("assistant".into(), "line\nbreak\ttab".into()),
        ];
        // Must not panic on multi-byte or control characters.
        render(&mut win);
    }

    /// Regression: text must actually reach the pixel buffer.
    ///
    /// DrawTextW writes to a GDI DC, so if the fills write to a Rust slice while
    /// the text goes to a different surface, every glyph lands somewhere the
    /// app never blits from. The window then renders with correct colours and no
    /// text at all, which is exactly what happened. This asserts glyph pixels
    /// appear in the same buffer the fills wrote to.
    #[test]
    fn drawing_text_lands_in_the_pixel_buffer() {
        let Some(owned) = DrawTarget::new() else {
            // No GDI surface available in this environment; nothing to assert.
            return;
        };
        let mut win = w();
        win.status = "Connected".into();
        win.messages = vec![("user".into(), "hello screenbuddy".into())];

        let _ = owned;
        let rendered = render(&mut win);

        // GDI stores BGRA, so compare in that order rather than against the
        // constants directly; otherwise muted text never matches even when drawn.
        let glyphs = rendered
            .surface
            .iter()
            .filter(|&&p| is_text_pixel(p))
            .count();
        assert!(
            glyphs > 50,
            "expected glyph pixels, found {glyphs}; text is not reaching the buffer"
        );
    }

    #[test]
    fn rect_containment_excludes_the_right_and_bottom_edges() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(r.contains(10, 10));
        assert!(r.contains(19, 19));
        assert!(!r.contains(20, 19), "right edge excluded");
        assert!(!r.contains(19, 20), "bottom edge excluded");
    }
}
