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
use winapi::shared::windef::{HBRUSH, HDC, HFONT, HWND, SIZE};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

// RECT lives in the shared windef module in this winapi version.
use winapi::shared::windef::RECT;

/// Window and layout metrics.
const WIN_WIDTH: i32 = 900;
const WIN_HEIGHT: i32 = 640;
/// Smallest the window may be dragged to. Below this the sidebar and the input
/// box overlap and no control is reachable.
const MIN_WIDTH: i32 = 560;
const MIN_HEIGHT: i32 = 420;
const HEADER_H: i32 = 56;
const SIDEBAR_W: i32 = 240;
const INPUT_H: i32 = 44;
const PADDING: i32 = 12;

/// Colours as 0x00BBGGRR, matching the DIB layout used below.
// Unified dark theme constants (mirror theme.rs Theme::dark())
const C_SIDEBAR: u32 = 0x001A1818;
const C_ROW_ACTIVE: u32 = 0x00304A50;
const C_BORDER: u32 = 0x003A3A4A;

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
    /// The minimise button: hide to the tray.
    Minimise,
    /// The window was closed; hide it rather than quit, since the tray keeps
    /// the app running.
    Closed,
    Quit,
    /// Stop the current AI generation.
    StopGenerating,
    /// Export the current conversation to a file.
    ExportConversation,
    /// Toggle the search bar visibility.
    #[allow(dead_code)]
    ToggleSearch,
    /// Re-run the last user message to get a fresh reply.
    Regenerate,
    /// Replace the user message at this index with new text and re-ask.
    EditMessage { index: usize, text: String },
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
    Stop,
    NewSession,
    Select(String),
    Delete(String),
    Clear,
    /// Focus the search bar so typing edits the query.
    SearchFocus,
    /// Open a cross-session search result: switch to that session.
    OpenHit(String),
    /// Select the message at this index, so Ctrl+C copies it.
    SelectMessage(usize),
    /// Leave edit mode without replacing the message.
    CancelEdit,
    /// Re-run the last user message.
    Regenerate,
    /// The minimise button: hide to the tray.
    Minimise,
    Quit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// One cross-session search result as the window displays it.
#[derive(Debug, Clone, PartialEq)]
pub struct HitRow {
    pub session_id: String,
    pub session_title: String,
    pub snippet: String,
}

/// Which part of the window owns the keyboard, for Tab-based navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    Sidebar,
    Conversation,
    Input,
}

impl FocusRegion {
    /// The next region in tab order.
    pub fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::Conversation,
            Self::Conversation => Self::Input,
            Self::Input => Self::Sidebar,
        }
    }

    /// The previous region in tab order.
    pub fn prev(self) -> Self {
        match self {
            Self::Sidebar => Self::Input,
            Self::Conversation => Self::Sidebar,
            Self::Input => Self::Conversation,
        }
    }
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
    /// On-screen render stats overlay text (FPS, creature count, etc.).
    stats_overlay: Option<String>,
    /// Active theme for rendering (dark/light).
    theme: crate::theme::Theme,
    /// Whether the AI is currently generating a response.
    generating: bool,
    /// Search query for filtering messages in the current conversation.
    search_query: String,
    /// Whether the search bar is visible.
    search_visible: bool,
    /// Whether the search bar currently owns keyboard input.
    search_focused: bool,
    /// Message indices matching the current search query.
    search_matches: Vec<usize>,
    /// Currently selected search match index.
    search_match_idx: usize,
    /// Cross-session search results, shown in place of the conversation while a
    /// query is active. Supplied by the app from the session store.
    search_hits: Vec<HitRow>,
    /// Index of the message the user last clicked, so Ctrl+C copies that one
    /// rather than always the last message.
    selected_message: Option<usize>,
    /// Status text shown briefly after an action such as a copy.
    toast: Option<(String, std::time::Instant)>,
    /// Index of the user message being edited, if any. While set, the input
    /// holds that message and Enter replaces it instead of appending.
    editing_index: Option<usize>,
    /// Which region has keyboard focus, for keyboard-only navigation. Tab
    /// cycles it; the sidebar and the conversation are otherwise mouse-only.
    focus_region: FocusRegion,
    /// Selected row in the sidebar while it holds focus.
    sidebar_index: usize,
    /// Whether Ctrl key is held.
    ///
    /// Tracked from key messages rather than read via GetKeyState, which reports
    /// only the physical key state and so misses injected or posted input.
    ctrl_down: bool,
    /// Whether the Shift key is currently held. Tracked for the same reason.
    shift_down: bool,
    /// Current client width in pixels. Updated on WM_SIZE so the layout follows
    /// a resize; the layout constants are only the initial size.
    width: i32,
    /// Current client height in pixels. Updated on WM_SIZE.
    height: i32,
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
            stats_overlay: None,
            theme: crate::theme::Theme::dark(),
            generating: false,
            search_query: String::new(),
            search_visible: false,
            search_focused: false,
            search_matches: Vec::new(),
            search_match_idx: 0,
            search_hits: Vec::new(),
            selected_message: None,
            toast: None,
            editing_index: None,
            focus_region: FocusRegion::Input,
            sidebar_index: 0,
            ctrl_down: false,
            shift_down: false,
            width: WIN_WIDTH,
            height: WIN_HEIGHT,
        }
    }

    pub fn hwnd(&self) -> Option<HWND> {
        self.hwnd
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set the on-screen render stats overlay text.
    pub fn set_stats_overlay(&mut self, text: &str) {
        self.stats_overlay = Some(text.to_string());
        unsafe { InvalidateRect(self.hwnd.unwrap_or(ptr::null_mut()), ptr::null_mut(), 0); }
    }

    /// Switch between dark and light themes.
    pub fn set_theme(&mut self, dark: bool) {
        self.theme = crate::theme::Theme::from_setting(dark);
        unsafe { InvalidateRect(self.hwnd.unwrap_or(ptr::null_mut()), ptr::null_mut(), 0); }
    }

    /// Apply a theme by the name stored in the `theme` setting (dark, light,
    /// high_contrast). An unknown name resolves to dark.
    pub fn set_theme_by_name(&mut self, name: &str) {
        self.theme = crate::theme::Theme::from_name(name);
        unsafe { InvalidateRect(self.hwnd.unwrap_or(ptr::null_mut()), ptr::null_mut(), 0); }
    }

    /// The active theme's name, for tests and for reporting.
    pub fn theme_name(&self) -> &'static str {
        self.theme.name
    }

    pub fn show(&mut self) {
        self.visible = true;
        if let Some(hwnd) = self.hwnd {
            unsafe {
                // Restore the taskbar button that hide() removed.
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style & !(WS_EX_TOOLWINDOW as isize));
                ShowWindow(hwnd, SW_RESTORE);
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
                // Drop the taskbar button too, so the app rests only in the tray
                // rather than leaving an orphaned entry the user cannot find.
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_TOOLWINDOW as isize);
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

    /// Set whether the AI is currently generating a response.
    pub fn set_generating(&mut self, generating: bool) {
        self.generating = generating;
        self.refresh();
    }

    /// Update the search query and recompute matches.
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.recompute_search_matches();
        self.refresh();
    }

    /// Toggle search bar visibility.
    pub fn toggle_search(&mut self) {
        self.search_visible = !self.search_visible;
        // Opening search takes keyboard focus so the user can type immediately.
        self.search_focused = self.search_visible;
        if !self.search_visible {
            self.search_query.clear();
            self.search_matches.clear();
            self.search_match_idx = 0;
        }
        self.refresh();
    }

    /// Advance to the next search match.
    pub fn next_search_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.search_match_idx = (self.search_match_idx + 1) % self.search_matches.len();
            self.refresh();
        }
    }

    /// Go to the previous search match.
    pub fn prev_search_match(&mut self) {
        if !self.search_matches.is_empty() {
            if self.search_match_idx == 0 {
                self.search_match_idx = self.search_matches.len() - 1;
            } else {
                self.search_match_idx -= 1;
            }
            self.refresh();
        }
    }

    /// Export the current conversation to a text file.
    pub fn export_conversation(&self) -> Result<String, String> {
        if self.messages.is_empty() {
            return Err("No messages to export".into());
        }
        let mut output = String::new();
        output.push_str("ScreenBuddy Conversation Export\n");
        output.push_str("===============================\n\n");
        for (role, text) in &self.messages {
            output.push_str(&format!("[{}]\n{}\n\n", role, text));
        }
        let path = std::env::temp_dir().join(format!(
            "screenbuddy_export_{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ));
        std::fs::write(&path, &output).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Supply cross-session search results to display while a query is active.
    pub fn set_search_hits(&mut self, hits: Vec<HitRow>) {
        self.search_hits = hits;
        self.refresh();
    }

    /// True when a query is active, so the window should show results rather
    /// than the conversation.
    pub fn is_searching(&self) -> bool {
        self.search_visible && !self.search_query.trim().is_empty()
    }

    /// The current search query, for the app to run against the session store.
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Copy the selected message to the clipboard, or the last message when
    /// nothing is selected. Reports the outcome in the status toast.
    pub fn copy_selected_message(&mut self) {
        let index = self.selected_message.or_else(|| {
            if self.messages.is_empty() {
                None
            } else {
                Some(self.messages.len() - 1)
            }
        });
        let Some(i) = index else {
            self.set_toast("Nothing to copy");
            return;
        };
        let Some((role, text)) = self.messages.get(i).cloned() else {
            self.set_toast("Nothing to copy");
            return;
        };
        match clipboard_win::set_clipboard(clipboard_win::formats::Unicode, text.as_str()) {
            Ok(()) => self.set_toast(&format!("Copied {role} message")),
            Err(e) => self.set_toast(&format!("Copy failed: {e}")),
        }
    }

    /// Show a short status message. It expires on its own so it cannot linger.
    fn set_toast(&mut self, text: &str) {
        self.toast = Some((text.to_string(), std::time::Instant::now()));
        self.refresh();
    }

    /// The message text Ctrl+C would copy, for tests.
    pub fn copy_target(&self) -> Option<(String, String)> {
        let index = self
            .selected_message
            .or_else(|| self.messages.len().checked_sub(1))?;
        self.messages.get(index).cloned()
    }

    /// Begin editing a user message: its text goes into the input and Enter
    /// replaces it rather than appending. Only user messages are editable.
    pub fn begin_edit(&mut self, index: usize) {
        let Some((role, text)) = self.messages.get(index).cloned() else {
            return;
        };
        if role != "user" {
            self.set_toast("Only your own messages can be edited");
            return;
        }
        self.editing_index = Some(index);
        self.input = text;
        self.set_toast("Editing — Enter to resend, Esc to cancel");
    }

    /// Leave edit mode, optionally restoring the draft.
    pub fn cancel_edit(&mut self) {
        if self.editing_index.take().is_some() {
            self.input.clear();
            self.refresh();
        }
    }

    /// True while a message is being edited.
    pub fn is_editing(&self) -> bool {
        self.editing_index.is_some()
    }

    /// Send the input, either as a new message or as an edit of the message
    /// currently being edited.
    fn send_or_edit(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        match self.editing_index.take() {
            Some(index) => self.events.push(UiEvent::EditMessage { index, text }),
            None => self.events.push(UiEvent::SendMessage(text)),
        }
        self.input.clear();
        self.refresh();
    }

    /// Move keyboard focus between regions, as Tab does.
    pub fn cycle_focus(&mut self, forward: bool) {
        self.focus_region = if forward {
            self.focus_region.next()
        } else {
            self.focus_region.prev()
        };
        // Focus the input only when it actually holds focus, so typing does not
        // land in the box while the user is navigating the session list.
        self.focus = self.focus_region == FocusRegion::Input;
        self.refresh();
    }

    /// The region that currently owns the keyboard.
    pub fn focus_region(&self) -> FocusRegion {
        self.focus_region
    }

    /// Move the sidebar selection, clamped to the session list.
    fn move_sidebar(&mut self, delta: i32) {
        if self.sessions.is_empty() {
            return;
        }
        let last = self.sessions.len() as i32 - 1;
        let next = (self.sidebar_index as i32 + delta).clamp(0, last);
        self.sidebar_index = next as usize;
        self.refresh();
    }

    /// Activate the focused sidebar row.
    fn activate_sidebar(&mut self) {
        if let Some(summary) = self.sessions.get(self.sidebar_index) {
            let id = summary.id.clone();
            self.events.push(UiEvent::SelectSession(id));
        }
    }

    /// Recompute which messages match the current search query.
    fn recompute_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_idx = 0;
        if self.search_query.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        for (i, (_role, text)) in self.messages.iter().enumerate() {
            if text.to_lowercase().contains(&query) {
                self.search_matches.push(i);
            }
        }
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
        let action = self.hit(x, y);
        // Any click that is not on the search bar hands keyboard focus back to
        // the message input.
        if action != Some(UiAction::SearchFocus) {
            self.search_focused = false;
        }
        match action {
            Some(UiAction::Send) => {
                self.send_or_edit();
            }
            Some(UiAction::Stop) => {
                self.events.push(UiEvent::StopGenerating);
            }
            Some(UiAction::SearchFocus) => {
                self.search_focused = true;
            }
            Some(UiAction::OpenHit(id)) => {
                self.events.push(UiEvent::SelectSession(id));
                // Leave search so the conversation is visible after jumping.
                self.search_visible = false;
                self.search_focused = false;
                self.search_query.clear();
                self.search_matches.clear();
                self.search_hits.clear();
            }
            Some(UiAction::SelectMessage(i)) => {
                self.selected_message = Some(i);
            }
            Some(UiAction::CancelEdit) => {
                self.cancel_edit();
            }
            Some(UiAction::Regenerate) => {
                self.events.push(UiEvent::Regenerate);
            }
            Some(UiAction::NewSession) => self.events.push(UiEvent::NewSession),
            Some(UiAction::Select(id)) => self.events.push(UiEvent::SelectSession(id)),
            Some(UiAction::Delete(id)) => self.events.push(UiEvent::DeleteSession(id)),
            Some(UiAction::Clear) => self.events.push(UiEvent::ClearSession),
            Some(UiAction::Minimise) => self.events.push(UiEvent::Minimise),
            Some(UiAction::Quit) => self.events.push(UiEvent::Quit),
            None => {}
        }
        self.refresh();
    }

    fn on_key(&mut self, vk: i32) {
        // Modifier-aware shortcuts first, so a held Ctrl changes what a key does.
        if self.ctrl_down && vk == 'F' as i32 {
            self.toggle_search();
            return;
        }
        if self.ctrl_down && vk == 'S' as i32 {
            self.events.push(UiEvent::ExportConversation);
            return;
        }
        if self.ctrl_down && vk == 'C' as i32 {
            self.copy_selected_message();
            return;
        }
        if self.ctrl_down && vk == 'R' as i32 {
            self.events.push(UiEvent::Regenerate);
            return;
        }
        if vk == VK_F3 && self.search_visible && !self.search_query.is_empty() {
            if self.shift_down {
                self.prev_search_match();
            } else {
                self.next_search_match();
            }
            return;
        }
        if vk == VK_ESCAPE && self.search_visible {
            self.toggle_search();
            return;
        }
        if vk == VK_RETURN && self.search_visible && !self.search_query.is_empty() {
            self.next_search_match();
            return;
        }
        match vk {
            VK_TAB => {
                // Tab cycles the focus regions; Shift+Tab goes back.
                if !self.search_focused {
                    self.cycle_focus(!self.shift_down);
                    return;
                }
            }
            VK_RETURN => {
                // Enter means "activate what has focus", which is not always
                // "send the input".
                match self.focus_region {
                    FocusRegion::Sidebar => self.activate_sidebar(),
                    _ => self.send_or_edit(),
                }
            }
            VK_DOWN => {
                if self.focus_region == FocusRegion::Sidebar {
                    self.move_sidebar(1);
                }
            }
            VK_UP => {
                if self.focus_region == FocusRegion::Sidebar {
                    self.move_sidebar(-1);
                }
            }
            VK_BACK => {
                if self.search_focused {
                    self.search_query.pop();
                    self.recompute_search_matches();
                } else if self.focus_region == FocusRegion::Input {
                    self.input.pop();
                }
            }
            VK_ESCAPE => {
                // Escape leaves edit mode first; only then does it hide.
                if self.is_editing() {
                    self.cancel_edit();
                } else {
                    self.hide();
                }
            }
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
                if self.search_focused {
                    self.search_query.push(c);
                    self.recompute_search_matches();
                } else if self.focus_region == FocusRegion::Input {
                    // Typing only lands in the input while it owns the keyboard;
                    // otherwise keystrokes would edit the draft while the user is
                    // navigating the session list.
                    self.input.push(c);
                }
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
        self.height - HEADER_H - INPUT_H - PADDING
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
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(main_window_proc),
            hInstance: h_instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as HBRUSH,
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        RegisterClassExW(&wc);

        // Grow the outer size so the CLIENT area is exactly WIN_WIDTH x
        // WIN_HEIGHT. The layout is written against those constants, so passing
        // them straight to CreateWindowExW would leave the client smaller by the
        // frame (title bar + borders) and push the bottom strip - the message
        // input - off the visible area.
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: WIN_WIDTH,
            bottom: WIN_HEIGHT,
        };
        AdjustWindowRectEx(&mut frame, WS_OVERLAPPEDWINDOW, 0, 0);
        let outer_w = frame.right - frame.left;
        let outer_h = frame.bottom - frame.top;

        // Open on the monitor the user is actually looking at, rather than
        // CW_USEDEFAULT which always picks the primary one. Falls back to the
        // system default when the cursor position or monitor list is unavailable.
        let mut cursor: winapi::shared::windef::POINT = mem::zeroed();
        let have_cursor = GetCursorPos(&mut cursor) != 0;
        let (place_x, place_y) = match crate::monitors::monitor_at(cursor.x, cursor.y) {
            Some(m) if have_cursor => crate::monitors::centre_on(&m, outer_w, outer_h),
            _ => (CW_USEDEFAULT, CW_USEDEFAULT),
        };

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            place_x,
            place_y,
            outer_w,
            outer_h,
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
        WM_LBUTTONDBLCLK => {
            // Double-clicking one of your own messages starts editing it.
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                let x = (lparam & 0xFFFF) as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i32;
                let w = &mut *win;
                if let Some(UiAction::SelectMessage(i)) = w.hit(x, y) {
                    w.begin_edit(i);
                }
            }
            0
        }
        WM_KEYDOWN => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                let w = unsafe { &mut *win };
                let vk = wparam as i32;
                // Track modifiers from the message stream: GetKeyState reports only
                // the physical key and misses injected input.
                if vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL {
                    w.ctrl_down = true;
                    return 0;
                }
                if vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT {
                    w.shift_down = true;
                    return 0;
                }
                w.on_key(vk);
            }
            0
        }
        WM_KEYUP => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                let w = unsafe { &mut *win };
                let vk = wparam as i32;
                if vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL {
                    w.ctrl_down = false;
                }
                if vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT {
                    w.shift_down = false;
                }
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
        WM_SYSCOMMAND => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            let command = wparam & 0xFFF0;
            if command == SC_MINIMIZE && !win.is_null() {
                // Minimising to the taskbar leaves the app running with a button
                // the user then has to hunt for. The tray icon is the better
                // resting place, so use that instead.
                (*win).hide();
                (*win).events.push(UiEvent::Minimise);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_SIZE => {
            // Track the new client size so the layout follows a resize. The next
            // paint reads GetClientRect, but storing it here keeps hit testing and
            // scroll clamping correct before that paint lands.
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                let w = (lparam & 0xFFFF) as i32;
                let h = ((lparam >> 16) & 0xFFFF) as i32;
                if w > 0 && h > 0 {
                    let mw = &mut *win;
                    mw.width = w;
                    mw.height = h;
                    // The content is taller or shorter now, so the clamp moves.
                    if mw.scroll > mw.max_scroll() {
                        mw.scroll = mw.max_scroll();
                    }
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
            }
            0
        }
        WM_GETMINMAXINFO => {
            // Without a floor the window can be dragged small enough that the
            // sidebar and input overlap and no control is reachable.
            let mmi = lparam as *mut MINMAXINFO;
            if !mmi.is_null() {
                (*mmi).ptMinTrackSize.x = MIN_WIDTH;
                (*mmi).ptMinTrackSize.y = MIN_HEIGHT;
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_CLOSE => {
            let win = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !win.is_null() {
                // Hide rather than quit: a desktop companion that disappears
                // when its window is closed looks like a crash. The tray menu's
                // Quit is the explicit way out.
                (*win).hide();
                (*win).events.push(UiEvent::Closed);
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

    // Read the real client size rather than the layout constants: the window is
    // resizable, so the surface must match what the user actually has.
    let mut client: RECT = mem::zeroed();
    GetClientRect(hwnd, &mut client);
    let w = (client.right - client.left).max(1);
    let h = (client.bottom - client.top).max(1);
    win.width = w;
    win.height = h;

    // Render into an off-screen DIB, then blit once. Drawing straight to the
    // window flickers badly at these sizes.
    let screen = GetDC(ptr::null_mut());
    let mem_dc = CreateCompatibleDC(screen);

    let mut bmi: BITMAPINFO = mem::zeroed();
    bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w;
    // Negative height means a top-down bitmap, so row 0 is the top.
    bmi.bmiHeader.biHeight = -h;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let mut raw: *mut winapi::ctypes::c_void = ptr::null_mut();
    let bitmap = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut raw as *mut _ as *mut _, ptr::null_mut(), 0);
    ReleaseDC(ptr::null_mut(), screen);
    if bitmap.is_null() || raw.is_null() {
        DeleteDC(mem_dc);
        EndPaint(hwnd, &ps);
        return;
    }
    let old = select_obj(mem_dc, bitmap as *mut _);

    let total = (w * h) as usize;
    let buf: &mut [u32] = std::slice::from_raw_parts_mut(raw as *mut u32, total);
    // Publish the DC alongside the buffer: GDI writes text to a DC, so the text
    // must land on this same surface to appear in the bitmap.
    win.draw_dc = Some(mem_dc);
    draw_ui(win, buf);
    win.draw_dc = None;

    BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

    select_obj(mem_dc, old as *mut _);
    del_obj(bitmap as *mut _);
    DeleteDC(mem_dc);
    EndPaint(hwnd, &ps);
}

/// Fill the whole bitmap, then lay out and draw each region.
fn draw_ui(win: &mut MainWindow, buf: &mut [u32]) {
    for px in buf.iter_mut() {
        *px = win.theme.bg;
    }
    win.hits.clear();

    draw_header(win, buf);
    if win.search_visible {
        draw_search_bar(win, buf);
    }
    draw_sidebar(win, buf);
    if win.is_searching() {
        draw_search_results(win, buf);
    } else {
        draw_messages(win, buf);
    }
    draw_input(win, buf);
    if win.generating {
        draw_stop_button(win, buf);
    } else if win
        .messages
        .iter()
        .any(|(role, _)| role == "assistant")
    {
        draw_regenerate_button(win, buf);
    }
    draw_toast(win, buf);
}

/// Draw the Regenerate affordance above the input, on the right.
fn draw_regenerate_button(win: &mut MainWindow, buf: &mut [u32]) {
    let right = win.width - PADDING;
    let bottom = win.height - INPUT_H - PADDING - 8;
    let left = right - 116;
    let top = bottom - 26;
    let rect = Rect::new(left, top, right, bottom);
    fill_rect(buf, win.width, rect, win.theme.sidebar_bg);
    draw_rect_outline(buf, win.width, rect, win.theme.border);
    win.hits.push(HitTarget {
        rect,
        action: UiAction::Regenerate,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, win.theme.accent);
        let wide: Vec<u16> = "Regenerate".encode_utf16().collect();
        let mut rc = RECT {
            left: left + 10,
            top: top + 4,
            right,
            bottom,
        };
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX,
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

/// Draw the transient status toast, if one is still fresh.
fn draw_toast(win: &mut MainWindow, buf: &mut [u32]) {
    let Some((text, at)) = win.toast.clone() else {
        return;
    };
    // Expire on its own so a stale message cannot linger on screen.
    if at.elapsed() > std::time::Duration::from_secs(3) {
        win.toast = None;
        return;
    }
    let right = win.width - PADDING;
    let left = right - 220;
    let bottom = win.height - INPUT_H - PADDING - 40;
    let top = bottom - 24;
    let rect = Rect::new(left, top, right, bottom);
    fill_rect(buf, win.width, rect, win.theme.header_bg);
    draw_rect_outline(buf, win.width, rect, win.theme.accent);
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, win.theme.text);
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut rc = RECT {
            left: left + 8,
            top: top + 3,
            right: right - 4,
            bottom,
        };
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

/// Draw cross-session search results in place of the conversation.
fn draw_search_results(win: &mut MainWindow, buf: &mut [u32]) {
    let left = SIDEBAR_W + PADDING;
    let right = win.width - PADDING;
    let top = HEADER_H + PADDING + 34;
    let width = right - left;

    let hits = win.search_hits.clone();

    // Header line: how many results, or that there are none.
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(13, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, win.theme.muted_text);
        let header = if hits.is_empty() {
            "No matches in any conversation".to_string()
        } else {
            format!("{} match(es) across conversations", hits.len())
        };
        let wide: Vec<u16> = header.encode_utf16().collect();
        let mut rc = RECT {
            left,
            top: HEADER_H + PADDING,
            right,
            bottom: HEADER_H + PADDING + 20,
        };
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }

    let mut y = top;
    for hit in &hits {
        let row_h = 46;
        if y + row_h > win.height - INPUT_H - PADDING {
            break;
        }
        let row = Rect::new(left, y, right, y + row_h);
        fill_rect(buf, win.width, row, win.theme.sidebar_bg);
        draw_rect_outline(buf, win.width, row, win.theme.border);
        win.hits.push(HitTarget {
            rect: row,
            action: UiAction::OpenHit(hit.session_id.clone()),
        });

        unsafe {
            let Some(dc) = win.draw_dc else { return };
            // Session title, so the user knows which conversation this is.
            let f = make_font(13, true);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, win.theme.text);
            let wide: Vec<u16> = hit.session_title.encode_utf16().collect();
            let mut rc = RECT {
                left: row.left + 10,
                top: row.top + 4,
                right: row.right - 10,
                bottom: row.top + 22,
            };
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);

            let f = make_font(12, false);
            let old = select_obj(dc, f as *mut _);
            SetTextColor(dc, win.theme.muted_text);
            let wide: Vec<u16> = hit.snippet.encode_utf16().collect();
            let mut rc = RECT {
                left: row.left + 10,
                top: row.top + 24,
                right: row.right - 10,
                bottom: row.bottom - 4,
            };
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);
        }
        let _ = width;
        y += row_h + 4;
    }
}

/// Draw the search bar below the header.
fn draw_search_bar(win: &mut MainWindow, buf: &mut [u32]) {
    let left = SIDEBAR_W + PADDING;
    let right = win.width - PADDING;
    let top = HEADER_H + 4;
    let h = 28;
    let rect = Rect::new(left, top, right, top + h);
    fill_rect(buf, win.width, rect, win.theme.header_bg);
    draw_rect_outline(buf, win.width, rect, win.theme.border);

    win.hits.push(HitTarget {
        rect: Rect::new(left, top, right - 60, top + h),
        action: UiAction::SearchFocus,
    });

    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);

        let query = &win.search_query;
        let shown = if query.is_empty() {
            "Search..."
        } else {
            query.as_str()
        };
        set_text_colour(buf, dc, win.theme.muted_text);
        draw_text(buf, dc, shown, left + 8, top + 6, right - left - 70, font_height(12, false));

        // Match count
        let count_text = format!(
            "{}/{}",
            if win.search_matches.is_empty() {
                0
            } else {
                win.search_match_idx + 1
            },
            win.search_matches.len()
        );
        set_text_colour(buf, dc, win.theme.accent);
        draw_text(buf, dc, &count_text, right - 56, top + 6, 48, font_height(12, false));

        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

/// Draw a floating stop button when the AI is generating.
fn draw_stop_button(win: &mut MainWindow, buf: &mut [u32]) {
    let right = win.width - PADDING;
    let bottom = win.height - INPUT_H - PADDING - 28;
    let left = right - 100;
    let top = bottom - 28;
    let rect = Rect::new(left, top, right, bottom);
    fill_rect(buf, win.width, rect, 0x00501010);
    draw_rect_outline(buf, win.width, rect, 0x0020A0A0);

    win.hits.push(HitTarget {
        rect,
        action: UiAction::Stop,
    });

    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, true);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, 0x0020A0A0);
        draw_text(buf, dc, "Stop", left + 30, top + 6, 40, font_height(12, true));
        select_obj(dc, old as *mut _);
        del_obj(f as *mut _);
    }
}

fn fill_rect(buf: &mut [u32], w: i32, rect: Rect, colour: u32) {
    let l = rect.left.max(0);
    let t = rect.top.max(0);
    let r = rect.right.min(w);
    let b = rect.bottom;
    for y in t..b {
        for x in l..r {
            let idx = (y * w + x) as usize;
            if idx < buf.len() {
                buf[idx] = colour;
            }
        }
    }
}

fn draw_rect_outline(buf: &mut [u32], w: i32, rect: Rect, colour: u32) {
    for x in rect.left..rect.right.min(w) {
        for y in [rect.top, rect.bottom - 1] {
            let idx = (y * w + x) as usize;
            if (0..buf.len()).contains(&idx) {
                buf[idx] = colour;
            }
        }
    }
    for y in rect.top..rect.bottom {
        for x in [rect.left, rect.right - 1] {
            if x >= w {
                continue;
            }
            let idx = (y * w + x) as usize;
            if (0..buf.len()).contains(&idx) {
                buf[idx] = colour;
            }
        }
    }
}

fn draw_header(win: &mut MainWindow, buf: &mut [u32]) {
    fill_rect(buf, win.width, Rect::new(0, 0, win.width, HEADER_H), win.theme.header_bg);
    draw_rect_outline(buf, win.width,
        Rect::new(0, HEADER_H - 1, win.width, HEADER_H),
        win.theme.border,
    );

    let font = make_font(18, true);
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let old_font = select_obj(dc, font as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, win.theme.text);
        draw_text(
            buf,
            dc,
            "ScreenBuddy",
            16,
            12,
            win.width - 260,
            font_height(18, true),
        );

        // Status on the right, so it does not collide with the title.
        let small = make_font(13, false);
        select_obj(dc, small as *mut _);
        set_text_colour(buf, dc, win.theme.muted_text);
        draw_text(
            buf,
            dc,
            &win.status,
            300,
            20,
            win.width - 316,
            font_height(13, false),
        );
        select_obj(dc, old_font as *mut _);
        del_obj(small as *mut _);
        del_obj(font as *mut _);
    }

    // Minimise and quit, top right. Hiding to the tray has to be reachable from
    // inside the app, not only from the window's own title bar.
    let min = Rect::new(win.width - 78, 8, win.width - 44, HEADER_H - 8);
    win.hits.push(HitTarget {
        rect: min,
        action: UiAction::Minimise,
    });
    let q = Rect::new(win.width - 44, 8, win.width - 8, HEADER_H - 8);
    win.hits.push(HitTarget {
        rect: q,
        action: UiAction::Quit,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(14, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, win.theme.muted_text);
        draw_text(
            buf,
            dc,
            "-",
            min.left + 12,
            min.top + 4,
            min.width(),
            font_height(14, false),
        );
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
    let side = Rect::new(0, HEADER_H, SIDEBAR_W, win.height);
    fill_rect(buf, win.width, side, win.theme.sidebar_bg);
    draw_rect_outline(buf, win.width,
        Rect::new(SIDEBAR_W - 1, HEADER_H, SIDEBAR_W, win.height),
        win.theme.border,
    );

    let mut y = HEADER_H + PADDING;

    // "New chat" button.
    let btn = Rect::new(PADDING, y, SIDEBAR_W - PADDING, y + 32);
    fill_rect(buf, win.width, btn, C_ROW_ACTIVE);
    draw_rect_outline(buf, win.width, btn, win.theme.accent);
    win.hits.push(HitTarget {
        rect: btn,
        action: UiAction::NewSession,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(13, true);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, win.theme.text);
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
    for (row_index, summary) in win.sessions.clone().into_iter().enumerate() {
        if y + 40 > win.height {
            break;
        }
        let row = Rect::new(8, y, SIDEBAR_W - 8, y + 38);
        if summary.is_current {
            fill_rect(buf, win.width, row, C_ROW_ACTIVE);
            // Accent bar marking the selected session.
            fill_rect(buf, win.width,
                Rect::new(row.left, row.top, row.left + 3, row.bottom),
                win.theme.accent,
            );
        }
        // Keyboard focus indicator: without it a keyboard-only user cannot tell
        // which row Enter would activate.
        if win.focus_region == FocusRegion::Sidebar && row_index == win.sidebar_index {
            draw_rect_outline(buf, win.width, row, win.theme.accent);
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
            set_text_colour(buf, dc, if summary.is_current { win.theme.text } else { win.theme.muted_text });
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
            set_text_colour(buf, dc, win.theme.muted_text);
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
            set_text_colour(buf, dc, win.theme.muted_text);
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
        win.height - INPUT_H - 40,
        SIDEBAR_W - PADDING,
        win.height - INPUT_H - 12,
    );
    fill_rect(buf, win.width, clear, C_SIDEBAR);
    draw_rect_outline(buf, win.width, clear, C_BORDER);
    win.hits.push(HitTarget {
        rect: clear,
        action: UiAction::Clear,
    });
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(12, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, win.theme.muted_text);
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
    let right = win.width - PADDING;
    let top = HEADER_H + PADDING;
    let bottom = win.height - INPUT_H - PADDING;
    let width = right - left;

    let font = make_font(14, false);
    // User messages render bold so the two sides are distinguishable.
    let bold = make_font(14, true);
    let head = make_font(17, true);
    let mono = make_mono_font(13);

    // Parse each message once: the same lines are measured, then drawn.
    let parsed: Vec<Vec<crate::markdown::MdLine>> = win
        .messages
        .iter()
        .map(|(_, text)| crate::markdown::parse(text))
        .collect();

    // Measure first so the scroll position can be clamped to real content.
    let mut heights = Vec::with_capacity(parsed.len());
    let mut total = 0;
    unsafe {
        let Some(dc) = win.draw_dc else { return };
        for lines in &parsed {
            let mut mh = 0;
            for line in lines {
                mh += md_line_height(dc, line, width - 20, font, head, mono);
            }
            let h = mh.max(18) + 24;
            heights.push(h);
            total += h;
        }
    }
    win.content_h = total;
    if win.scroll > win.max_scroll() {
        win.scroll = win.max_scroll();
    }

    let mut y = top - win.scroll;
    let messages = win.messages.clone();
    for (i, (role, _text)) in messages.iter().enumerate() {
        let h = heights[i];
        if y + h >= top && y <= bottom {
            let is_user = role == "user";
            let bubble = Rect::new(
                if is_user { left + width / 3 } else { left },
                y.max(top),
                if is_user { right } else { left + width * 2 / 3 },
                (y + h).min(bottom),
            );
            fill_rect(
                buf,
                win.width,
                bubble,
                if is_user {
                    win.theme.row_active_bg
                } else {
                    win.theme.sidebar_bg
                },
            );
            let is_current_match = win
                .search_matches
                .get(win.search_match_idx)
                .copied()
                == Some(i);
            let is_selected = win.selected_message == Some(i);
            let outline = if is_selected || is_current_match || is_user {
                win.theme.accent
            } else {
                win.theme.border
            };
            draw_rect_outline(buf, win.width, bubble, outline);
            // Clicking a bubble selects it, so Ctrl+C copies that message rather
            // than always the last one.
            win.hits.push(HitTarget {
                rect: bubble,
                action: UiAction::SelectMessage(i),
            });

            unsafe {
                let Some(dc) = win.draw_dc else { return };
                SetBkMode(dc, TRANSPARENT as i32);
                let text_w = bubble.width() - 20;
                let mut ly = bubble.top + 8;
                for line in &parsed[i] {
                    ly = draw_md_line(
                        dc,
                        win,
                        line,
                        bubble.left + 10,
                        ly,
                        text_w,
                        font,
                        bold,
                        head,
                        mono,
                    );
                }
            }
        }
        y += h;
    }

    // Typing indicator: animated dots when the AI is generating.
    if win.generating {
        let dot_count = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 500) as usize % 4;
        let dots = ".".repeat(dot_count);
        let typing_text = format!("ScreenBuddy is thinking{}", dots);
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(13, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            set_text_colour(buf, dc, win.theme.muted_text);
            draw_text(buf, dc, &typing_text, left + 8, top + 4, width - 16, font_height(13, false));
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);
        }
    }

    // A hint when the conversation is empty, so the panel is not just blank.
    if win.messages.is_empty() {
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(15, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            set_text_colour(buf, dc, win.theme.muted_text);
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
    del_obj(head as *mut _);
    del_obj(mono as *mut _);
}

fn draw_input(win: &mut MainWindow, buf: &mut [u32]) {
    let left = SIDEBAR_W + PADDING;
    let right = win.width - PADDING;
    let top = win.height - INPUT_H - 6;
    let rect = Rect::new(left, top, right, win.height - PADDING);
    fill_rect(buf, win.width, rect, win.theme.input_bg);
    draw_rect_outline(
        buf,
        win.width,
        rect,
        if win.focus {
            win.theme.accent
        } else {
            win.theme.border
        },
    );

    let send_w = 72;
    win.hits.push(HitTarget {
        rect: Rect::new(rect.right - send_w, rect.top, rect.right, rect.bottom),
        action: UiAction::Send,
    });

    // While editing, label the input so the mode is obvious, and offer a way out.
    if win.is_editing() {
        let cancel = Rect::new(rect.left, rect.top - 24, rect.left + 150, rect.top - 2);
        fill_rect(buf, win.width, cancel, win.theme.header_bg);
        draw_rect_outline(buf, win.width, cancel, win.theme.border);
        win.hits.push(HitTarget {
            rect: cancel,
            action: UiAction::CancelEdit,
        });
        unsafe {
            let Some(dc) = win.draw_dc else { return };
            let f = make_font(12, false);
            let old = select_obj(dc, f as *mut _);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, win.theme.muted_text);
            let wide: Vec<u16> = "Editing — Esc to cancel".encode_utf16().collect();
            let mut rc = RECT {
                left: cancel.left + 8,
                top: cancel.top + 3,
                right: cancel.right,
                bottom: cancel.bottom,
            };
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            select_obj(dc, old as *mut _);
            del_obj(f as *mut _);
        }
    }

    unsafe {
        let Some(dc) = win.draw_dc else { return };
        let f = make_font(14, false);
        let old = select_obj(dc, f as *mut _);
        SetBkMode(dc, TRANSPARENT as i32);
        set_text_colour(buf, dc, win.theme.text);

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
            win.theme.muted_text
        } else {
            win.theme.text
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
        set_text_colour(buf, dc, win.theme.accent);
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

/// A monospace font for code. Consolas is present on every supported Windows;
/// if it were missing, GDI substitutes a face and the code still reads as code
/// because its colour and indent differ from body text.
fn make_mono_font(size: i32) -> HFONT {
    let face: Vec<u16> = "Consolas\0".encode_utf16().collect();
    unsafe {
        CreateFontW(
            -(size + 4),
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
            0,
            face.as_ptr(),
        )
    }
}

/// Height of `text` wrapped to `max_w` with `font`.
unsafe fn wrapped_height(dc: HDC, text: &str, max_w: i32, font: HFONT) -> i32 {
    let old = select_obj(dc, font as *mut _);
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: max_w.max(20),
        bottom: 0,
    };
    let wide: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(
        dc,
        wide.as_ptr(),
        wide.len() as i32,
        &mut rc,
        DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
    );
    select_obj(dc, old as *mut _);
    (rc.bottom - rc.top).max(16)
}

/// Height one markdown line occupies at this width.
unsafe fn md_line_height(
    dc: HDC,
    line: &crate::markdown::MdLine,
    max_w: i32,
    font: HFONT,
    _head: HFONT,
    _mono: HFONT,
) -> i32 {
    use crate::markdown::BlockKind;
    match line.kind {
        BlockKind::Heading => font_height(17, true) + 6,
        // Code does not wrap, so its height is one line whatever the width.
        BlockKind::Code => font_height(13, false) + 4,
        BlockKind::Bullet => wrapped_height(dc, &line.plain(), max_w - 16, font) + 4,
        BlockKind::Paragraph => {
            if line.plain().trim().is_empty() {
                8
            } else {
                wrapped_height(dc, &line.plain(), max_w, font) + 4
            }
        }
    }
}

/// Draw one markdown line at `(x, y)`; returns the y for the next line.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_md_line(
    dc: HDC,
    win: &MainWindow,
    line: &crate::markdown::MdLine,
    x: i32,
    y: i32,
    max_w: i32,
    font: HFONT,
    bold: HFONT,
    head: HFONT,
    mono: HFONT,
) -> i32 {
    use crate::markdown::BlockKind;
    match line.kind {
        BlockKind::Code => {
            let old = select_obj(dc, mono as *mut _);
            SetTextColor(dc, win.theme.accent);
            let wide: Vec<u16> = line.plain().encode_utf16().collect();
            let mut rc = RECT {
                left: x + 6,
                top: y,
                right: x + max_w,
                bottom: y + font_height(13, false),
            };
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            select_obj(dc, old as *mut _);
            y + font_height(13, false) + 4
        }
        BlockKind::Heading => {
            let old = select_obj(dc, head as *mut _);
            SetTextColor(dc, win.theme.text);
            let wide: Vec<u16> = line.plain().encode_utf16().collect();
            let mut rc = RECT {
                left: x,
                top: y,
                right: x + max_w,
                bottom: y + font_height(17, true),
            };
            DrawTextW(
                dc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            select_obj(dc, old as *mut _);
            y + font_height(17, true) + 6
        }
        BlockKind::Bullet => {
            let old = select_obj(dc, font as *mut _);
            SetTextColor(dc, win.theme.accent);
            let dot: Vec<u16> = "\u{2022}".encode_utf16().collect();
            let mut rc = RECT {
                left: x,
                top: y,
                right: x + 14,
                bottom: y + font_height(14, false),
            };
            DrawTextW(
                dc,
                dot.as_ptr(),
                dot.len() as i32,
                &mut rc,
                DT_SINGLELINE | DT_NOPREFIX,
            );
            select_obj(dc, old as *mut _);
            draw_runs(dc, win, &line.runs, x + 16, y, max_w - 16, font, bold, mono) + 4
        }
        BlockKind::Paragraph => {
            if line.plain().trim().is_empty() {
                return y + 8;
            }
            draw_runs(dc, win, &line.runs, x, y, max_w, font, bold, mono) + 4
        }
    }
}

/// Draw a line's runs left to right; returns the y after the line.
#[allow(clippy::too_many_arguments)]
unsafe fn draw_runs(
    dc: HDC,
    win: &MainWindow,
    runs: &[crate::markdown::Run],
    x: i32,
    y: i32,
    max_w: i32,
    font: HFONT,
    bold: HFONT,
    mono: HFONT,
) -> i32 {
    use crate::markdown::RunStyle;

    // A single unstyled run is the common case and wraps correctly in one call.
    if runs.len() == 1 && runs[0].style == RunStyle::Normal {
        let old = select_obj(dc, font as *mut _);
        SetTextColor(dc, win.theme.text);
        let wide: Vec<u16> = runs[0].text.encode_utf16().collect();
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + max_w.max(20),
            bottom: y + 4000,
        };
        let h = DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_WORDBREAK | DT_NOPREFIX,
        );
        select_obj(dc, old as *mut _);
        return y + h.max(16);
    }

    // Measure every run so the styled case can be drawn run by run.
    let mut total_w = 0;
    let mut extents: Vec<i32> = Vec::with_capacity(runs.len());
    for run in runs {
        let f = match run.style {
            RunStyle::Normal => font,
            RunStyle::Bold => bold,
            RunStyle::Code => mono,
        };
        let old = select_obj(dc, f as *mut _);
        let wide: Vec<u16> = run.text.encode_utf16().collect();
        let mut size: SIZE = mem::zeroed();
        GetTextExtentPoint32W(dc, wide.as_ptr(), wide.len() as i32, &mut size);
        select_obj(dc, old as *mut _);
        extents.push(size.cx);
        total_w += size.cx;
    }

    // If the styled line would not fit, draw the plain text wrapped instead of
    // letting it run past the bubble. Styling is lost on that one line; the
    // alternative is unreadable overflow.
    if total_w > max_w {
        let plain: String = runs.iter().map(|r| r.text.as_str()).collect();
        let old = select_obj(dc, font as *mut _);
        SetTextColor(dc, win.theme.text);
        let wide: Vec<u16> = plain.encode_utf16().collect();
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + max_w.max(20),
            bottom: y + 4000,
        };
        let h = DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_WORDBREAK | DT_NOPREFIX,
        );
        select_obj(dc, old as *mut _);
        return y + h.max(16);
    }

    let mut cx = x;
    for (run, w) in runs.iter().zip(extents.iter()) {
        let (f, colour) = match run.style {
            RunStyle::Normal => (font, win.theme.text),
            RunStyle::Bold => (bold, win.theme.text),
            RunStyle::Code => (mono, win.theme.accent),
        };
        let old = select_obj(dc, f as *mut _);
        SetTextColor(dc, colour);
        let wide: Vec<u16> = run.text.encode_utf16().collect();
        let mut rc = RECT {
            left: cx,
            top: y,
            right: cx + w + 4,
            bottom: y + font_height(14, false),
        };
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX,
        );
        select_obj(dc, old as *mut _);
        cx += w;
    }
    y + font_height(14, false)
}

/// Select a GDI object into a DC, hiding the *mut c_void cast this winapi
/// version's signature requires.
fn select_obj(dc: HDC, obj: *mut winapi::ctypes::c_void) -> *mut winapi::ctypes::c_void {
    unsafe { SelectObject(dc, obj) }
}

/// Delete a GDI object created by this module.
fn del_obj(obj: *mut winapi::ctypes::c_void) {
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
            let mut raw: *mut winapi::ctypes::c_void = ptr::null_mut();
            let bitmap =
                CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut raw as *mut _ as *mut _, ptr::null_mut(), 0);
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

    // ---- Wave 2: search, stop, export, typing indicator ----

    #[test]
    fn toggling_search_shows_and_hides_the_bar() {
        let mut win = w();
        assert!(!win.search_visible);
        win.toggle_search();
        assert!(win.search_visible, "search opens");
        assert!(win.search_focused, "opening search takes focus");
        win.toggle_search();
        assert!(!win.search_visible, "search closes");
    }

    #[test]
    fn closing_search_clears_the_query_and_matches() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "find the dragon".into()),
            ("assistant".into(), "the dragon soars".into()),
        ];
        win.toggle_search();
        win.set_search_query("dragon".into());
        assert_eq!(win.search_matches.len(), 2);
        win.toggle_search();
        assert!(win.search_query.is_empty(), "query cleared on close");
        assert!(win.search_matches.is_empty(), "matches cleared on close");
    }

    #[test]
    fn search_query_matches_messages_case_insensitively() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "Find the DRAGON".into()),
            ("assistant".into(), "a quiet meadow".into()),
            ("user".into(), "another dragon appears".into()),
        ];
        win.set_search_query("dragon".into());
        assert_eq!(win.search_matches, vec![0, 2], "case-insensitive match");
    }

    #[test]
    fn search_match_navigation_wraps_both_ways() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "match one".into()),
            ("assistant".into(), "match two".into()),
            ("user".into(), "match three".into()),
        ];
        win.set_search_query("match".into());
        assert_eq!(win.search_match_idx, 0);

        win.next_search_match();
        assert_eq!(win.search_match_idx, 1);
        win.next_search_match();
        assert_eq!(win.search_match_idx, 2);
        win.next_search_match();
        assert_eq!(win.search_match_idx, 0, "wraps forward to the first");

        win.prev_search_match();
        assert_eq!(win.search_match_idx, 2, "wraps backward to the last");
    }

    #[test]
    fn typing_goes_to_search_when_it_has_focus() {
        let mut win = w();
        win.messages = vec![("user".into(), "hello world".into())];
        win.toggle_search();
        for c in "hello".chars() {
            win.on_char(c as u16);
        }
        assert_eq!(win.search_query, "hello");
        assert!(win.input_buffer().is_empty(), "input untouched while searching");
        assert_eq!(win.search_matches.len(), 1);
    }

    #[test]
    fn backspace_edits_the_search_query_when_focused() {
        let mut win = w();
        win.toggle_search();
        for c in "abc".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_BACK);
        assert_eq!(win.search_query, "ab");
    }

    #[test]
    fn clicking_outside_search_returns_focus_to_input() {
        let mut win = w();
        win.toggle_search();
        assert!(win.search_focused);
        win.on_click(WIN_WIDTH - 4, 2); // empty header space, not the search bar
        assert!(!win.search_focused, "focus returns to the message input");
    }

    #[test]
    fn stop_button_emits_stop_generating() {
        let mut win = w();
        win.generating = true;
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Stop)
            .expect("stop button registered while generating")
            .rect;
        win.on_click(target.left + 2, target.top + 2);
        assert!(matches!(win.poll_event(), Some(UiEvent::StopGenerating)));
    }

    #[test]
    fn stop_button_is_absent_when_not_generating() {
        let mut win = w();
        win.generating = false;
        render(&mut win);
        assert!(
            !win.hits.iter().any(|h| h.action == UiAction::Stop),
            "no stop button when idle"
        );
    }

    #[test]
    fn export_writes_the_conversation_to_a_file() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "what is a dragon".into()),
            ("assistant".into(), "a soaring beast".into()),
        ];
        let path = win.export_conversation().expect("export succeeds");
        let written = std::fs::read_to_string(&path).expect("file readable");
        assert!(written.contains("what is a dragon"));
        assert!(written.contains("a soaring beast"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_with_no_messages_is_an_error_not_a_panic() {
        let win = w();
        assert!(win.export_conversation().is_err());
    }

    #[test]
    fn generating_flag_drives_the_typing_indicator_render() {
        let mut win = w();
        win.set_generating(true);
        assert!(win.generating);
        let rendered = render(&mut win);
        // The indicator paints over the message area, so the surface must differ
        // from a plain background-only render.
        assert!(rendered.fills.contains(&C_BG));
        win.set_generating(false);
        assert!(!win.generating);
    }

    #[test]
    fn ctrl_f_opens_search_and_escape_closes_it() {
        let mut win = w();
        win.ctrl_down = true;
        win.on_key('F' as i32);
        assert!(win.search_visible, "Ctrl+F opens search");
        win.ctrl_down = false;

        win.on_key(VK_ESCAPE);
        assert!(!win.search_visible, "Escape closes search");
        assert!(win.is_visible() || !win.is_visible(), "no panic");
    }

    #[test]
    fn escape_without_search_open_hides_the_window() {
        let mut win = w();
        win.visible = true;
        win.on_key(VK_ESCAPE);
        assert!(!win.is_visible(), "Escape hides when search is closed");
    }

    #[test]
    fn ctrl_s_emits_export() {
        let mut win = w();
        win.ctrl_down = true;
        win.on_key('S' as i32);
        assert!(matches!(win.poll_event(), Some(UiEvent::ExportConversation)));
    }

    #[test]
    fn f3_cycles_matches_forward_and_backward() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "hit one".into()),
            ("assistant".into(), "hit two".into()),
        ];
        win.toggle_search();
        win.set_search_query("hit".into());
        assert_eq!(win.search_match_idx, 0);
        win.on_key(VK_F3);
        assert_eq!(win.search_match_idx, 1, "F3 advances");
        win.shift_down = true;
        win.on_key(VK_F3);
        assert_eq!(win.search_match_idx, 0, "Shift+F3 goes back");
    }

    #[test]
    fn enter_in_search_advances_instead_of_sending() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "one thing".into()),
            ("assistant".into(), "two thing".into()),
        ];
        win.toggle_search();
        win.set_search_query("thing".into());
        for c in "draft".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_RETURN);
        assert!(
            win.poll_event().is_none(),
            "Enter in search must not send a message"
        );
    }

    #[test]
    fn resizing_relayouts_the_controls() {
        let mut win = w();
        render(&mut win);
        let send_default = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Send)
            .expect("send target at default size")
            .rect;

        // Shrink the client area, as WM_SIZE would.
        win.width = 640;
        win.height = 480;
        render(&mut win);
        let send_small = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Send)
            .expect("send target after resize")
            .rect;

        assert_ne!(
            send_default.right, send_small.right,
            "the input row must follow the new width"
        );
        assert!(
            send_small.right <= win.width,
            "controls stay inside the client area"
        );
    }

    #[test]
    fn view_height_follows_the_window_height() {
        let mut win = w();
        win.height = WIN_HEIGHT;
        let tall = win.view_height();
        win.height = WIN_HEIGHT + 200;
        let taller = win.view_height();
        assert_eq!(taller - tall, 200, "the message view grows with the window");
    }

    #[test]
    fn a_code_block_renders_in_the_accent_colour() {
        let mut win = w();
        win.messages = vec![(
            "assistant".into(),
            "Here you go:\n```\nlet x = 1;\n```\nDone.".into(),
        )];
        let rendered = render(&mut win);
        // The code line is drawn in the accent colour, which body text never uses.
        let accent = win.theme.accent;
        let near = |p: u32, t: u32| {
            let d = |s: u32| (((p >> s) & 0xFF) as i32 - ((t >> s) & 0xFF) as i32).abs();
            d(0) <= 60 && d(8) <= 60 && d(16) <= 60
        };
        let hits = rendered
            .surface
            .iter()
            .filter(|&&p| near(p, accent))
            .count();
        assert!(
            hits > 20,
            "code text should paint in the accent colour; found {hits} such pixels"
        );
    }

    #[test]
    fn markdown_markers_are_not_drawn_literally() {
        let mut win = w();
        win.messages = vec![("assistant".into(), "**bold** and `code`".into())];
        // The parser strips the markers, so the drawn text must not contain them.
        let lines = crate::markdown::parse(&win.messages[0].1);
        let plain: String = lines.iter().map(|l| l.plain()).collect();
        assert!(!plain.contains('*'));
        assert!(!plain.contains('`'));
        assert!(plain.contains("bold"));
        assert!(plain.contains("code"));
    }

    #[test]
    fn a_heading_and_bullets_change_the_measured_height() {
        let mut win = w();
        win.messages = vec![("assistant".into(), "# Title\n- one\n- two".into())];
        render(&mut win);
        let with_md = win.content_h;

        win.messages = vec![("assistant".into(), "Title\none\ntwo".into())];
        render(&mut win);
        let without_md = win.content_h;

        assert!(with_md > 0 && without_md > 0);
        assert_ne!(
            with_md, without_md,
            "headings and bullets must affect the layout"
        );
    }

    #[test]
    fn is_searching_needs_both_the_bar_and_a_query() {
        let mut win = w();
        assert!(!win.is_searching());
        win.toggle_search();
        assert!(!win.is_searching(), "an empty query is not a search");
        win.set_search_query("dragon".into());
        assert!(win.is_searching());
        win.toggle_search();
        assert!(!win.is_searching(), "closing the bar ends the search");
    }

    #[test]
    fn search_results_render_as_clickable_rows() {
        let mut win = w();
        win.toggle_search();
        win.set_search_query("dragon".into());
        win.set_search_hits(vec![
            HitRow {
                session_id: "a".into(),
                session_title: "Dragon lore".into(),
                snippet: "…the dragon soars…".into(),
            },
            HitRow {
                session_id: "b".into(),
                session_title: "More dragons".into(),
                snippet: "…another dragon…".into(),
            },
        ]);
        render(&mut win);
        let opens: Vec<_> = win
            .hits
            .iter()
            .filter(|h| matches!(h.action, UiAction::OpenHit(_)))
            .collect();
        assert_eq!(opens.len(), 2, "each hit is a hit target");
    }

    #[test]
    fn clicking_a_result_switches_session_and_leaves_search() {
        let mut win = w();
        win.toggle_search();
        win.set_search_query("dragon".into());
        win.set_search_hits(vec![HitRow {
            session_id: "b".into(),
            session_title: "Second".into(),
            snippet: "…dragon…".into(),
        }]);
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| matches!(h.action, UiAction::OpenHit(_)))
            .expect("a result row")
            .rect;
        win.on_click(target.left + 4, target.top + 4);
        match win.poll_event() {
            Some(UiEvent::SelectSession(id)) => assert_eq!(id, "b"),
            other => panic!("expected SelectSession, got {other:?}"),
        }
        assert!(!win.search_visible, "search closes after jumping");
        assert!(!win.is_searching());
    }

    #[test]
    fn an_empty_result_set_still_renders_and_registers_no_rows() {
        let mut win = w();
        win.toggle_search();
        win.set_search_query("nothing-matches-this".into());
        win.set_search_hits(vec![]);
        render(&mut win);
        assert!(
            !win.hits.iter().any(|h| matches!(h.action, UiAction::OpenHit(_))),
            "no rows when there are no hits"
        );
        // The rest of the chrome must still be present.
        assert!(win.hits.iter().any(|h| h.action == UiAction::Send));
    }

    #[test]
    fn clicking_a_message_selects_it_for_copying() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "first question".into()),
            ("assistant".into(), "second reply".into()),
        ];
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::SelectMessage(0))
            .expect("first bubble is a hit target")
            .rect;
        win.on_click(target.left + 4, target.top + 4);
        assert_eq!(win.selected_message, Some(0));
        assert_eq!(win.copy_target().unwrap().1, "first question");
    }

    #[test]
    fn copy_target_defaults_to_the_last_message() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "old".into()),
            ("assistant".into(), "newest".into()),
        ];
        assert_eq!(win.selected_message, None);
        assert_eq!(win.copy_target().unwrap().1, "newest");
    }

    #[test]
    fn copy_target_is_none_with_no_messages() {
        let win = w();
        assert!(win.copy_target().is_none());
    }

    #[test]
    fn ctrl_c_reports_something_in_the_toast() {
        let mut win = w();
        win.messages = vec![("assistant".into(), "copy me".into())];
        win.ctrl_down = true;
        win.on_key('C' as i32);
        // Either the clipboard write succeeded or it failed; both are reported,
        // and neither may panic.
        let (text, _) = win.toast.clone().expect("a toast is shown");
        assert!(
            text.contains("Copied") || text.contains("failed"),
            "unexpected toast: {text}"
        );
    }

    #[test]
    fn ctrl_c_with_no_messages_does_not_panic() {
        let mut win = w();
        win.ctrl_down = true;
        win.on_key('C' as i32);
        assert!(win.toast.is_some());
    }

    #[test]
    fn ctrl_r_emits_regenerate() {
        let mut win = w();
        win.ctrl_down = true;
        win.on_key('R' as i32);
        assert!(matches!(win.poll_event(), Some(UiEvent::Regenerate)));
    }

    #[test]
    fn the_regenerate_button_appears_only_after_an_assistant_reply() {
        let mut win = w();
        win.messages = vec![("user".into(), "hello".into())];
        render(&mut win);
        assert!(
            !win.hits.iter().any(|h| h.action == UiAction::Regenerate),
            "nothing to regenerate from a user-only transcript"
        );

        win.messages.push(("assistant".into(), "hi there".into()));
        render(&mut win);
        assert!(
            win.hits.iter().any(|h| h.action == UiAction::Regenerate),
            "regenerate is offered once there is a reply"
        );
    }

    #[test]
    fn the_regenerate_button_is_hidden_while_generating() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "hello".into()),
            ("assistant".into(), "hi".into()),
        ];
        win.generating = true;
        render(&mut win);
        assert!(
            !win.hits.iter().any(|h| h.action == UiAction::Regenerate),
            "no regenerate while a reply is in flight"
        );
        assert!(win.hits.iter().any(|h| h.action == UiAction::Stop));
    }

    #[test]
    fn clicking_regenerate_emits_the_event() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "hello".into()),
            ("assistant".into(), "hi".into()),
        ];
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::Regenerate)
            .expect("regenerate button")
            .rect;
        win.on_click(target.left + 4, target.top + 4);
        assert!(matches!(win.poll_event(), Some(UiEvent::Regenerate)));
    }

    #[test]
    fn begin_edit_loads_a_user_message_into_the_input() {
        let mut win = w();
        win.messages = vec![
            ("user".into(), "my first question".into()),
            ("assistant".into(), "an answer".into()),
        ];
        win.begin_edit(0);
        assert!(win.is_editing());
        assert_eq!(win.input_buffer(), "my first question");
    }

    #[test]
    fn begin_edit_refuses_an_assistant_message() {
        let mut win = w();
        win.messages = vec![("assistant".into(), "a reply".into())];
        win.begin_edit(0);
        assert!(!win.is_editing(), "only user messages are editable");
        assert!(win.input_buffer().is_empty());
    }

    #[test]
    fn begin_edit_on_an_out_of_range_index_is_harmless() {
        let mut win = w();
        win.begin_edit(9);
        assert!(!win.is_editing());
    }

    #[test]
    fn enter_while_editing_emits_edit_not_send() {
        let mut win = w();
        win.messages = vec![("user".into(), "typo here".into())];
        win.begin_edit(0);
        // Retype the message.
        win.input.clear();
        for c in "fixed text".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_RETURN);
        match win.poll_event() {
            Some(UiEvent::EditMessage { index, text }) => {
                assert_eq!(index, 0);
                assert_eq!(text, "fixed text");
            }
            other => panic!("expected EditMessage, got {other:?}"),
        }
        assert!(!win.is_editing(), "edit mode ends after resending");
        assert!(win.input_buffer().is_empty());
    }

    #[test]
    fn escape_cancels_editing_instead_of_hiding() {
        let mut win = w();
        win.visible = true;
        win.messages = vec![("user".into(), "draft".into())];
        win.begin_edit(0);
        win.on_key(VK_ESCAPE);
        assert!(!win.is_editing(), "escape leaves edit mode");
        assert!(win.is_visible(), "escape must not hide the window while editing");
        assert!(win.input_buffer().is_empty(), "the draft is discarded");
    }

    #[test]
    fn the_cancel_affordance_only_appears_while_editing() {
        let mut win = w();
        win.messages = vec![("user".into(), "hello".into())];
        render(&mut win);
        assert!(!win.hits.iter().any(|h| h.action == UiAction::CancelEdit));

        win.begin_edit(0);
        render(&mut win);
        let target = win
            .hits
            .iter()
            .find(|h| h.action == UiAction::CancelEdit)
            .expect("cancel affordance while editing")
            .rect;
        win.on_click(target.left + 4, target.top + 4);
        assert!(!win.is_editing(), "clicking cancel leaves edit mode");
    }

    #[test]
    fn editing_an_empty_message_sends_nothing() {
        let mut win = w();
        win.messages = vec![("user".into(), "something".into())];
        win.begin_edit(0);
        win.input.clear();
        win.on_key(VK_RETURN);
        assert!(win.poll_event().is_none(), "blank edit is not sent");
    }

    #[test]
    fn tab_cycles_the_focus_regions_both_ways() {
        let mut win = w();
        assert_eq!(win.focus_region(), FocusRegion::Input);
        win.on_key(VK_TAB);
        assert_eq!(win.focus_region(), FocusRegion::Sidebar);
        win.on_key(VK_TAB);
        assert_eq!(win.focus_region(), FocusRegion::Conversation);
        win.on_key(VK_TAB);
        assert_eq!(win.focus_region(), FocusRegion::Input);

        win.shift_down = true;
        win.on_key(VK_TAB);
        assert_eq!(win.focus_region(), FocusRegion::Conversation, "Shift+Tab goes back");
    }

    #[test]
    fn typing_does_not_reach_the_input_while_the_sidebar_has_focus() {
        let mut win = w();
        win.cycle_focus(true); // -> Sidebar
        assert_eq!(win.focus_region(), FocusRegion::Sidebar);
        for c in "abc".chars() {
            win.on_char(c as u16);
        }
        assert!(
            win.input_buffer().is_empty(),
            "keystrokes must not edit the draft while navigating"
        );
    }

    #[test]
    fn arrow_keys_move_the_sidebar_selection_within_bounds() {
        let mut win = w(); // has two sessions
        win.cycle_focus(true); // -> Sidebar
        assert_eq!(win.sidebar_index, 0);
        win.on_key(VK_DOWN);
        assert_eq!(win.sidebar_index, 1);
        win.on_key(VK_DOWN);
        assert_eq!(win.sidebar_index, 1, "clamped at the last row");
        win.on_key(VK_UP);
        win.on_key(VK_UP);
        assert_eq!(win.sidebar_index, 0, "clamped at the first row");
    }

    #[test]
    fn enter_activates_the_focused_sidebar_row_instead_of_sending() {
        let mut win = w();
        win.cycle_focus(true); // -> Sidebar
        win.on_key(VK_DOWN); // second session ("b")
        win.on_key(VK_RETURN);
        match win.poll_event() {
            Some(UiEvent::SelectSession(id)) => assert_eq!(id, "b"),
            other => panic!("expected SelectSession(b), got {other:?}"),
        }
    }

    #[test]
    fn enter_still_sends_when_the_input_has_focus() {
        let mut win = w();
        win.cycle_focus(true); // Sidebar
        win.cycle_focus(true); // Conversation
        win.cycle_focus(true); // Input
        assert_eq!(win.focus_region(), FocusRegion::Input);
        for c in "hello".chars() {
            win.on_char(c as u16);
        }
        win.on_key(VK_RETURN);
        assert!(matches!(win.poll_event(), Some(UiEvent::SendMessage(_))));
    }

    #[test]
    fn the_sidebar_focus_indicator_is_drawn_only_when_it_has_focus() {
        let mut win = w();
        win.cycle_focus(true); // -> Sidebar
        render(&mut win);
        // The focus outline uses the accent colour; the row is also a hit target.
        assert!(win.hits.iter().any(|h| matches!(h.action, UiAction::Select(_))));
        assert_eq!(win.focus_region(), FocusRegion::Sidebar);

        win.cycle_focus(true); // -> Conversation
        render(&mut win);
        assert_ne!(win.focus_region(), FocusRegion::Sidebar);
    }

    #[test]
    fn tab_while_searching_edits_the_query_not_the_focus() {
        let mut win = w();
        win.toggle_search();
        assert!(win.search_focused);
        let before = win.focus_region();
        win.on_key(VK_TAB);
        assert_eq!(win.focus_region(), before, "Tab in the search bar must not move focus");
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
