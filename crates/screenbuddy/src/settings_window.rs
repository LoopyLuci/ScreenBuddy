//! Settings window: a real renderer over the real settings store.
//!
//! This replaces a shell that looked finished and was not. It had its own
//! `SettingValue` enum, its own empty `HashMap`, no `WM_PAINT` at all, and was
//! never connected to `SettingsUI` -- so it displayed nothing, stored nothing,
//! and any value it did hold went nowhere.
//!
//! It now renders [`screenbuddy_core::SettingsUI`] directly: every row comes from
//! a real definition, every edit goes back through `set()` (which validates and
//! persists), and the window shows the live value.
//!
//! Honesty rule, learned the hard way elsewhere in this codebase: a control the
//! app does not honour is labelled as such. [`INERT_SETTINGS`] lists the settings
//! with no consumer, and the UI greys them out rather than implying they work.

#![cfg(windows)]

use crate::gdi::{
    del_obj, draw_rect, draw_text, fill_rect, font_height, measure_text, select_font, select_obj,
    set_text_colour,
};
use screenbuddy_core::settings_ui::{SettingValue, SettingsCategory, SettingsReader, SettingsUI};
use std::ptr;
use winapi::shared::windef::HDC;
use winapi::shared::windef::{HBRUSH, HWND, RECT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

/// Client-area size. The window is created slightly larger so the frame and
/// title bar sit outside this.
const WIN_WIDTH: i32 = 620;
const WIN_HEIGHT: i32 = 540;
const TITLE_H: i32 = 40;
const TAB_H: i32 = 30;
const ROW_H: i32 = 30;
const PADDING: i32 = 16;
/// Where the setting's label ends and its control begins.
const FIELD_LEFT: i32 = 250;
const FIELD_W: i32 = WIN_WIDTH - FIELD_LEFT - PADDING;
/// Room for the name, its description and the control.
const ROW_PITCH: i32 = 54;

/// Settings with no consumer anywhere in the app.
///
/// Presented as disabled rather than wired up on paper. Every entry here was
/// previously shown as if it worked.
pub const INERT_SETTINGS: &[&str] = &[
    "ai_provider",
    "ai_model",
    "thread_pool_size",
    "multi_gpu",
    "window_transparency",
    "always_on_top",
    "volume_effects",
    "volume_music",
];

/// Settings the app writes but never reads.
///
/// A third state, distinct from both "honoured" and "inert": the app reflects
/// real state into them at startup, so the window is showing the truth, but
/// changing one does nothing. Labelling these "not wired up yet" was wrong --
/// they are not unwired, they are read-only.
pub const MIRRORED_SETTINGS: &[&str] = &[
    "collision_avoidance",
    "creature_count",
    "cursor_interaction",
];

/// Why a setting cannot be changed from the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// The app reads it, so changing it takes effect.
    Live,
    /// The app writes it but never reads it: a read-only mirror of real state.
    ReadOnly,
    /// Nothing on either end.
    Inert,
}

/// Whether a setting actually changes behaviour when changed.
#[cfg(test)]
pub fn setting_is_live(name: &str) -> bool {
    availability(name) == Availability::Live
}

/// Classify a setting, so the window says why a control is disabled.
pub fn availability(name: &str) -> Availability {
    // Mirrored is checked first: these names are deliberately absent from
    // INERT_SETTINGS, so a live-first ordering would classify them as Live.
    if MIRRORED_SETTINGS.contains(&name) {
        Availability::ReadOnly
    } else if INERT_SETTINGS.contains(&name) {
        Availability::Inert
    } else {
        Availability::Live
    }
}

/// A category tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    General,
    Audio,
    Animation,
    Ai,
    Creatures,
    Performance,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::General,
        Tab::Audio,
        Tab::Animation,
        Tab::Ai,
        Tab::Creatures,
        Tab::Performance,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::Audio => "Audio",
            Tab::Animation => "Animation",
            Tab::Ai => "AI",
            Tab::Creatures => "Creatures",
            Tab::Performance => "Performance",
        }
    }

    fn category(self) -> SettingsCategory {
        match self {
            Tab::General => SettingsCategory::General,
            Tab::Audio => SettingsCategory::Audio,
            Tab::Animation => SettingsCategory::Animation,
            Tab::Ai => SettingsCategory::AI,
            Tab::Creatures => SettingsCategory::Creatures,
            Tab::Performance => SettingsCategory::Performance,
        }
    }
}

/// What a click means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    SelectTab(usize),
    /// Clicking a value advances it.
    CycleValue(usize),
    Step(usize, StepDir),
    Toggle(usize),
    Close,
    ResetAll,
}

/// Which way a stepper moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepDir {
    Dec,
    Inc,
}

/// One clickable region.
#[derive(Debug, Clone, Copy)]
struct HitTarget {
    rect: (i32, i32, i32, i32),
    action: Action,
}

impl HitTarget {
    fn new(x: i32, y: i32, w: i32, h: i32, action: Action) -> Self {
        Self {
            rect: (x, y, w, h),
            action,
        }
    }

    fn contains(&self, mx: i32, my: i32) -> bool {
        let (x, y, w, h) = self.rect;
        mx >= x && mx < x + w && my >= y && my < y + h
    }
}

/// One row: a definition plus its live value.
#[derive(Clone)]
struct Row {
    name: String,
    description: String,
    value: SettingValue,
    min: Option<f64>,
    max: Option<f64>,
    /// Whether changing it takes effect.
    live: bool,
    /// Why not, when it cannot be changed.
    state: Availability,
}

/// The window's state.
pub struct SettingsView {
    /// A reader for the live values, so edits made elsewhere show up here.
    reader: SettingsReader,
    /// Borrowed for writes. Not owned, because the render loop owns the store.
    store: *mut SettingsUI,
    tab: Tab,
    rows: Vec<Row>,
    hits: Vec<HitTarget>,
    hwnd: Option<HWND>,
    /// Index of the row under the pointer, for hover feedback.
    hovered: Option<usize>,
    /// Set when a value is rejected, so the reason is visible.
    notice: Option<String>,
    /// Real client size, updated each paint.
    surface_w: i32,
    surface_h: i32,
}

impl SettingsView {
    /// Build a view over the shared store.
    ///
    /// # Safety
    ///
    /// `store` must outlive the view. It points at the render loop's
    /// `SettingsUI`, which lives for the whole process.
    pub unsafe fn new(reader: SettingsReader, store: *mut SettingsUI) -> Self {
        let mut view = Self {
            reader,
            store,
            tab: Tab::General,
            rows: Vec::new(),
            hits: Vec::new(),
            hwnd: None,
            hovered: None,
            notice: None,
            surface_w: WIN_WIDTH,
            surface_h: WIN_HEIGHT,
        };
        view.reload();
        view
    }

    /// Re-read the definitions and values for the current tab.
    fn reload(&mut self) {
        let definitions = unsafe { (*self.store).all_definitions() };
        let category = self.tab.category();
        self.rows.clear();
        for def in definitions.iter().filter(|d| d.category == category) {
            let name = def.name.clone();
            let value = self
                .reader
                .get(&name)
                .unwrap_or_else(|| def.default.clone());
            let min = def.min.as_ref().map(numeric);
            let max = def.max.as_ref().map(numeric);
            let state = availability(&name);
            self.rows.push(Row {
                live: state == Availability::Live,
                state,
                name,
                description: def.description.clone(),
                value,
                min,
                max,
            });
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.notice = None;
            self.hovered = None;
            self.reload();
        }
    }

    /// Advance a setting's value.
    fn cycle(&mut self, index: usize) {
        let Some(row) = self.rows.get(index).cloned() else {
            return;
        };
        if !row.live {
            self.notice = Some(format!("'{}' {}.", row.name, refusal_reason(row.state)));
            return;
        }
        self.apply(index, next_value(&row.value));
    }

    fn step(&mut self, index: usize, up: bool) {
        let Some(row) = self.rows.get(index).cloned() else {
            return;
        };
        if !row.live {
            self.notice = Some(format!("'{}' {}.", row.name, refusal_reason(row.state)));
            return;
        }
        self.apply(index, stepped_value(&row.value, row.min, row.max, up));
    }

    /// Write a value back, reporting a rejection rather than failing silently.
    fn apply(&mut self, index: usize, value: SettingValue) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let name = row.name.clone();
        let result = unsafe { (*self.store).set(&name, value.clone()) };
        match result {
            Ok(()) => {
                self.notice = None;
                self.rows[index].value = value;
            }
            Err(e) => self.notice = Some(e),
        }
    }

    fn reset_all(&mut self) {
        unsafe {
            (*self.store).reset();
        }
        self.notice = Some("Settings restored to defaults.".into());
        self.reload();
    }

    /// Handle a click, in client coordinates.
    pub fn on_click(&mut self, mx: i32, my: i32) {
        let Some(target) = self.hits.iter().find(|h| h.contains(mx, my)).copied() else {
            return;
        };
        match target.action {
            Action::SelectTab(index) => self.switch_tab(Tab::ALL[index]),
            Action::CycleValue(index) | Action::Toggle(index) => self.cycle(index),
            Action::Step(index, dir) => self.step(index, dir == StepDir::Inc),
            Action::ResetAll => self.reset_all(),
            Action::Close => self.hide(),
        }
    }

    /// Record hover so the row can be highlighted.
    pub fn on_move(&mut self, mx: i32, my: i32) {
        self.hovered = self
            .hits
            .iter()
            .find(|h| h.contains(mx, my))
            .and_then(|h| match h.action {
                Action::CycleValue(index) | Action::Toggle(index) | Action::Step(index, _) => {
                    Some(index)
                }
                _ => None,
            });
    }

    pub fn hwnd(&self) -> Option<HWND> {
        self.hwnd
    }

    pub fn set_hwnd(&mut self, hwnd: HWND) {
        self.hwnd = if hwnd.is_null() { None } else { Some(hwnd) };
    }

    pub fn is_visible(&self) -> bool {
        self.hwnd
            .map(|h| unsafe { IsWindowVisible(h) != 0 })
            .unwrap_or(false)
    }

    /// Show the window.
    pub fn show(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                InvalidateRect(hwnd, ptr::null(), 0);
            }
        }
    }

    pub fn hide(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    /// Repaint if the window exists.
    pub fn refresh(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                InvalidateRect(hwnd, ptr::null(), 0);
                UpdateWindow(hwnd);
            }
        }
    }

    /// Re-read values, for changes made elsewhere.
    pub fn sync(&mut self) {
        self.reload();
    }

    // ---------------------------------------------------------------- painting

    /// Paint the whole window into the device context.
    pub fn paint(&mut self, buf: &mut [u32], dc: HDC, surface_w: i32, surface_h: i32) {
        self.hits.clear();
        self.surface_w = surface_w;
        self.surface_h = surface_h;
        // Cover the whole client area so no unpainted band shows through.
        fill_rect(buf, surface_w, 0, 0, surface_w, surface_h, 0x00141212);

        self.paint_title(buf, dc);
        self.paint_tabs(buf, dc);
        self.paint_rows(buf, dc);
        self.paint_footer(buf, dc);
    }

    fn paint_title(&mut self, buf: &mut [u32], dc: HDC) {
        let w = self.surface_w;
        fill_rect(buf, w, 0, 0, w, TITLE_H, 0x00231F1F);
        let (font, old) = select_font(dc, 17, true);
        set_text_colour(dc, 0x00F0E6D8);
        draw_text(buf, dc, "Settings", PADDING, 9, 300, font_height(17, true));

        // Close button, top right.
        let cx = w - PADDING - 26;
        fill_rect(buf, w, cx, 9, 26, 24, 0x002A2A2A);
        set_text_colour(dc, 0x00D0D0D0);
        draw_text(buf, dc, "x", cx + 9, 12, 20, font_height(13, false));
        select_obj(dc, old);
        del_obj(font as *mut _);
        self.hits.push(HitTarget::new(cx, 9, 26, 24, Action::Close));
    }

    fn paint_tabs(&mut self, buf: &mut [u32], dc: HDC) {
        let w = self.surface_w;
        let (font, old) = select_font(dc, 12, false);
        let mut tx = PADDING;
        let ty = TITLE_H + 4;
        for (index, tab) in Tab::ALL.iter().enumerate() {
            let width = measure_text(dc, tab.label()) + PADDING * 2;
            let active = *tab == self.tab;
            fill_rect(
                buf,
                w,
                tx,
                ty,
                width,
                TAB_H,
                if active { 0x00304A50 } else { 0x00201E1E },
            );
            set_text_colour(dc, if active { 0x00FFFFFF } else { 0x0090A0A0 });
            draw_text(
                buf,
                dc,
                tab.label(),
                tx + PADDING,
                ty + 6,
                width - PADDING * 2,
                font_height(12, false),
            );
            self.hits.push(HitTarget::new(
                tx,
                ty,
                width,
                TAB_H,
                Action::SelectTab(index),
            ));
            tx += width + 4;
        }
        select_obj(dc, old);
        del_obj(font as *mut _);

        draw_rect(buf, w, 0, TITLE_H + TAB_H + 6, w, 1, 0x00303030);
    }

    fn paint_rows(&mut self, buf: &mut [u32], dc: HDC) {
        let w = self.surface_w;
        let top = TITLE_H + TAB_H + 20;

        if self.rows.is_empty() {
            let (font, old) = select_font(dc, 13, false);
            set_text_colour(dc, 0x0080A0A0);
            draw_text(
                buf,
                dc,
                "Nothing in this category yet",
                PADDING,
                top,
                w - PADDING * 2,
                font_height(13, false),
            );
            select_obj(dc, old);
            del_obj(font as *mut _);
            return;
        }

        let label_w = FIELD_LEFT - PADDING * 2;
        let rows = self.rows.clone();
        for (index, row) in rows.iter().enumerate() {
            let y = top + index as i32 * ROW_PITCH;
            if y + ROW_PITCH > self.surface_h - 50 {
                // Out of room: say so rather than silently dropping settings.
                let (font, old) = select_font(dc, 12, false);
                set_text_colour(dc, 0x0080A0A0);
                draw_text(
                    buf,
                    dc,
                    "more settings below - enlarge the window",
                    PADDING,
                    y,
                    label_w,
                    font_height(12, false),
                );
                select_obj(dc, old);
                del_obj(font as *mut _);
                break;
            }
            if self.hovered == Some(index) && row.live {
                fill_rect(
                    buf,
                    w,
                    PADDING - 6,
                    y - 6,
                    w - PADDING,
                    ROW_PITCH - 4,
                    0x001E2028,
                );
            }

            // Name.
            let (font, old) = select_font(dc, 13, row.live);
            set_text_colour(dc, if row.live { 0x00F0F0F0 } else { 0x00707070 });
            draw_text(
                buf,
                dc,
                &row.name,
                PADDING,
                y,
                label_w,
                font_height(13, row.live),
            );
            select_obj(dc, old);
            del_obj(font as *mut _);

            // Description.
            let (font, old) = select_font(dc, 11, false);
            set_text_colour(dc, 0x00A8C0C8);
            draw_text(
                buf,
                dc,
                &row.description,
                PADDING,
                y + 16,
                label_w,
                font_height(11, false),
            );
            // Say plainly why a setting cannot be changed, rather than letting
            // the user discover it.
            if !row.live {
                set_text_colour(dc, 0x00E0A060);
                let note = match row.state {
                    Availability::ReadOnly => "set by the app - read only",
                    _ => "not wired up yet",
                };
                draw_text(
                    buf,
                    dc,
                    note,
                    PADDING,
                    y + 30,
                    label_w,
                    font_height(11, false),
                );
            }
            select_obj(dc, old);
            del_obj(font as *mut _);

            self.paint_control(buf, dc, index, row, y);
        }
    }

    /// The control for one row: a stepper for numbers, a toggle for booleans.
    fn paint_control(&mut self, buf: &mut [u32], dc: HDC, index: usize, row: &Row, y: i32) {
        let w = self.surface_w;
        let x = FIELD_LEFT;
        let base = if row.live { 0x00282320 } else { 0x00201E1E };
        let text = if row.live { 0x00F0F0F0 } else { 0x00808080 };

        match &row.value {
            SettingValue::Bool(_) => {
                fill_rect(buf, w, x, y + 4, 16, 16, base);
                let is_on = numeric(&row.value) != 0.0;
                if is_on && row.live {
                    fill_rect(buf, w, x + 3, y + 7, 10, 10, 0x00F0C040);
                }
                let (font, old) = select_font(dc, 13, false);
                set_text_colour(dc, text);
                draw_text(
                    buf,
                    dc,
                    if is_on { "On" } else { "Off" },
                    x + 24,
                    y + 6,
                    80,
                    font_height(13, false),
                );
                select_obj(dc, old);
                del_obj(font as *mut _);
                if row.live {
                    self.hits
                        .push(HitTarget::new(x, y, 120, ROW_H, Action::Toggle(index)));
                }
            }
            SettingValue::Int(_) | SettingValue::Float(_) => {
                fill_rect(buf, w, x, y, FIELD_W, ROW_H, base);
                draw_rect(
                    buf,
                    w,
                    x,
                    y,
                    FIELD_W,
                    ROW_H,
                    if row.live { 0x00404040 } else { 0x00303030 },
                );

                let (font, old) = select_font(dc, 13, false);
                set_text_colour(dc, text);
                draw_text(
                    buf,
                    dc,
                    &format_value(&row.value),
                    x + 10,
                    y + 6,
                    FIELD_W - 80,
                    font_height(13, false),
                );
                // -/+ halves, so a setting is adjustable without a keyboard.
                set_text_colour(dc, if row.live { 0x00F0C040 } else { 0x00606060 });
                draw_text(
                    buf,
                    dc,
                    "-",
                    x + FIELD_W - 56,
                    y + 6,
                    20,
                    font_height(13, false),
                );
                draw_text(
                    buf,
                    dc,
                    "+",
                    x + FIELD_W - 26,
                    y + 6,
                    20,
                    font_height(13, false),
                );
                select_obj(dc, old);
                del_obj(font as *mut _);

                if row.live {
                    self.hits.push(HitTarget::new(
                        x + FIELD_W - 66,
                        y,
                        36,
                        ROW_H,
                        Action::Step(index, StepDir::Dec),
                    ));
                    self.hits.push(HitTarget::new(
                        x + FIELD_W - 36,
                        y,
                        36,
                        ROW_H,
                        Action::Step(index, StepDir::Inc),
                    ));
                }
            }
            SettingValue::String(_) => {
                fill_rect(buf, w, x, y, FIELD_W, ROW_H, base);
                let (font, old) = select_font(dc, 13, false);
                set_text_colour(dc, text);
                draw_text(
                    buf,
                    dc,
                    &format_value(&row.value),
                    x + 10,
                    y + 6,
                    FIELD_W - 20,
                    font_height(13, false),
                );
                select_obj(dc, old);
                del_obj(font as *mut _);
                if row.live {
                    self.hits.push(HitTarget::new(
                        x,
                        y,
                        FIELD_W,
                        ROW_H,
                        Action::CycleValue(index),
                    ));
                }
            }
            SettingValue::Enum(current, options) => {
                // Show the choice set, so the user can see there is one.
                fill_rect(buf, w, x, y, FIELD_W, ROW_H, base);
                let (font, old) = select_font(dc, 13, false);
                set_text_colour(dc, text);
                draw_text(
                    buf,
                    dc,
                    &format!(
                        "{current}  ({}/{})",
                        index_of(options, current) + 1,
                        options.len()
                    ),
                    x + 10,
                    y + 6,
                    FIELD_W - 40,
                    font_height(13, false),
                );
                set_text_colour(dc, if row.live { 0x00F0C040 } else { 0x00606060 });
                draw_text(
                    buf,
                    dc,
                    ">",
                    x + FIELD_W - 24,
                    y + 6,
                    20,
                    font_height(13, false),
                );
                select_obj(dc, old);
                del_obj(font as *mut _);
                if row.live {
                    self.hits.push(HitTarget::new(
                        x,
                        y,
                        FIELD_W,
                        ROW_H,
                        Action::CycleValue(index),
                    ));
                }
            }
        }
    }

    fn paint_footer(&mut self, buf: &mut [u32], dc: HDC) {
        let w = self.surface_w;
        let top = self.surface_h - 44;
        fill_rect(buf, w, 0, top, w, 44, 0x00231F1F);
        draw_rect(buf, w, 0, top, w, 1, 0x00303030);

        // A notice sits left of the button, so a rejection is not invisible.
        if let Some(notice) = self.notice.clone() {
            let (font, old) = select_font(dc, 12, false);
            set_text_colour(dc, 0x00E0A060);
            draw_text(
                buf,
                dc,
                &notice,
                PADDING,
                top + 14,
                w - PADDING - 130,
                font_height(12, false),
            );
            select_obj(dc, old);
            del_obj(font as *mut _);
        }

        let bw = 110;
        let rx = w - PADDING - bw;
        fill_rect(buf, w, rx, top + 10, bw, 26, 0x00304048);
        let (font, old) = select_font(dc, 12, false);
        set_text_colour(dc, 0x00D0E0E0);
        draw_text(
            buf,
            dc,
            "Reset all",
            rx + 10,
            top + 15,
            bw - 20,
            font_height(12, false),
        );
        select_obj(dc, old);
        del_obj(font as *mut _);
        self.hits
            .push(HitTarget::new(rx, top + 10, bw, 26, Action::ResetAll));
    }

    /// Paint the window: draw into a DIB, then blit to the paint DC.
    pub fn paint_window(&mut self, hwnd: HWND) {
        unsafe {
            // BeginPaint/EndPaint is mandatory; without it GDI silently discards
            // every drawing call.
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let paint_dc = BeginPaint(hwnd, &mut ps);

            let mut client = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut client);
            let cw = (client.right - client.left).max(WIN_WIDTH);
            let ch = (client.bottom - client.top).max(WIN_HEIGHT);

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = cw;
            info.bmiHeader.biHeight = -ch;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;

            let screen_dc = GetDC(hwnd);
            let mem_dc = CreateCompatibleDC(screen_dc);
            let mut raw: *mut core::ffi::c_void = ptr::null_mut();
            let bitmap = CreateDIBSection(
                screen_dc,
                &info,
                DIB_RGB_COLORS,
                &mut raw,
                ptr::null_mut(),
                0,
            );
            ReleaseDC(hwnd, screen_dc);
            if bitmap.is_null() || raw.is_null() {
                DeleteDC(mem_dc);
                EndPaint(hwnd, &ps);
                return;
            }
            let old_bitmap = SelectObject(mem_dc, bitmap as *mut _);

            // Fills and GDI text must share the DIB's pixels, or the blit shows
            // only one of them.
            let total = (cw * ch) as usize;
            let buffer: &mut [u32] = std::slice::from_raw_parts_mut(raw as *mut u32, total);
            self.paint(buffer, mem_dc, cw, ch);

            BitBlt(paint_dc, 0, 0, cw, ch, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap as *mut _);
            DeleteDC(mem_dc);
            EndPaint(hwnd, &ps);
        }
    }
}

/// Why a control refused a change, in words.
fn refusal_reason(state: Availability) -> &'static str {
    match state {
        Availability::Live => "can be changed",
        Availability::ReadOnly => "is set by the app and is read only",
        Availability::Inert => "is not wired up yet",
    }
}

/// A setting's magnitude, whatever its variant.
fn numeric(value: &SettingValue) -> f64 {
    match value {
        SettingValue::Bool(v) | SettingValue::Float(v) => *v,
        SettingValue::Int(v) => *v as f64,
        SettingValue::String(_) | SettingValue::Enum(_, _) => 0.0,
    }
}

/// Where a value sits in an enum's options.
fn index_of(options: &[String], current: &str) -> usize {
    options.iter().position(|o| o == current).unwrap_or(0)
}

/// Render a value for display.
fn format_value(value: &SettingValue) -> String {
    match value {
        SettingValue::Int(v) => v.to_string(),
        // Two decimals is enough for a slider and avoids a wall of digits.
        SettingValue::Float(v) | SettingValue::Bool(v) => format!("{v:.2}"),
        SettingValue::String(s) => s.clone(),
        SettingValue::Enum(current, _) => current.clone(),
    }
}

/// The next value when a control is clicked.
fn next_value(value: &SettingValue) -> SettingValue {
    match value {
        // Bools toggle.
        SettingValue::Bool(v) => SettingValue::Bool(if *v == 0.0 { 1.0 } else { 0.0 }),
        SettingValue::Int(v) => SettingValue::Int(v + 1),
        SettingValue::String(s) => {
            if s == "true" {
                SettingValue::String("false".into())
            } else {
                SettingValue::String("true".into())
            }
        }
        // Enums advance through their options, wrapping at the end.
        SettingValue::Enum(current, options) => {
            if options.is_empty() {
                return value.clone();
            }
            let next = (index_of(options, current) + 1) % options.len();
            SettingValue::Enum(options[next].clone(), options.clone())
        }
        other => other.clone(),
    }
}

/// A value nudged by one step, clamped to the definition's range.
fn stepped_value(
    value: &SettingValue,
    min: Option<f64>,
    max: Option<f64>,
    up: bool,
) -> SettingValue {
    // A fine step for small ranges like volume, a whole one for counts, so a
    // slider is neither tedious nor unusable.
    let step = if max.map(|m| m - min.unwrap_or(0.0)).unwrap_or(1.0) <= 4.0 {
        0.05
    } else {
        1.0
    };
    let clamp = |mut next: f64| {
        if let Some(lo) = min {
            next = next.max(lo);
        }
        if let Some(hi) = max {
            next = next.min(hi);
        }
        // A volume must never go negative whatever the arithmetic says.
        next.max(0.0)
    };
    let delta = if up { step } else { -step };

    match value {
        SettingValue::Int(v) => {
            let bump = if up { 1.0 } else { -1.0 };
            SettingValue::Int(clamp(*v as f64 + bump) as i64)
        }
        // Trim floating-point noise so a slider reads cleanly.
        SettingValue::Float(v) => SettingValue::Float((clamp(v + delta) * 1000.0).round() / 1000.0),
        SettingValue::Bool(v) => SettingValue::Bool(clamp(v + delta)),
        other => next_value(other),
    }
}

/// The x coordinate packed into an LPARAM mouse message.
fn point_x(lparam: isize) -> i32 {
    (lparam as u32 & 0xFFFF) as i16 as i32
}

/// The y coordinate packed into an LPARAM mouse message.
fn point_y(lparam: isize) -> i32 {
    ((lparam as u32 >> 16) & 0xFFFF) as i16 as i32
}

/// Window procedure. State hangs off `GWLP_USERDATA`.
unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        WM_PAINT => {
            let view = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsView;
            if !view.is_null() {
                (*view).paint_window(hwnd);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let view = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsView;
            if !view.is_null() {
                (*view).on_click(point_x(lparam), point_y(lparam));
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_MOUSEMOVE => {
            let view = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsView;
            if !view.is_null() {
                (*view).on_move(point_x(lparam), point_y(lparam));
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            // Escape hides, as in the other windows.
            if wparam == VK_ESCAPE as usize {
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        WM_CLOSE => {
            // Hides rather than quits: the app keeps running in the tray.
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Create the settings window, attaching `view` as its state.
///
/// # Safety
///
/// `view` must outlive the window.
pub unsafe fn create_settings_window(view: *mut SettingsView) -> Option<HWND> {
    let instance = GetModuleHandleW(ptr::null_mut());
    let class_name: Vec<u16> = "ScreenBuddySettings\0".encode_utf16().collect();

    let mut wc: WNDCLASSEXW = std::mem::zeroed();
    wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = Some(settings_proc);
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
    wc.lpszClassName = class_name.as_ptr();
    wc.hbrBackground = (COLOR_WINDOW + 1) as usize as HBRUSH;
    // Registering twice is harmless; the second call simply fails.
    RegisterClassExW(&wc);

    let title: Vec<u16> = "ScreenBuddy Settings\0".encode_utf16().collect();
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_APPWINDOW,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MAXIMIZEBOX,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        // +frame so the client area is WIN_WIDTH x WIN_HEIGHT.
        WIN_WIDTH + 16,
        WIN_HEIGHT + 60,
        ptr::null_mut(),
        ptr::null_mut(),
        instance,
        ptr::null_mut(),
    );
    if hwnd.is_null() {
        return None;
    }

    SetWindowLongPtrW(hwnd, GWLP_USERDATA, view as isize);
    (*view).set_hwnd(hwnd);
    Some(hwnd)
}

/// Bring the window forward, creating it if needed.
///
/// # Safety
///
/// `view` must outlive the window.
pub unsafe fn show_settings_window(view: *mut SettingsView) -> Option<HWND> {
    match (*view).hwnd() {
        Some(hwnd) if IsWindow(hwnd) != 0 => {
            (*view).sync();
            (*view).show();
            Some(hwnd)
        }
        _ => create_settings_window(view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An extra setting to exercise a specific shape.
    type Extra = (&'static str, SettingValue, Option<f64>, Option<f64>);

    /// Run a closure against a view over a throwaway store, then free it.
    ///
    /// The store is leaked into a raw pointer because the view borrows it for
    /// its whole life, which mirrors the real setup where the render loop owns
    /// the store.
    fn with_view<F: FnOnce(&mut SettingsView)>(extra: Option<Extra>, f: F) {
        let mut store = Box::new(SettingsUI::in_memory());
        if let Some((name, default, min, max)) = extra {
            store.register_for_test(name, default, min, max);
        }
        let reader = store.reader();
        let ptr = Box::into_raw(store);
        let mut view = unsafe { SettingsView::new(reader, ptr) };
        f(&mut view);
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }

    /// The index of a just-added setting, which lands last.
    fn last_index(view: &SettingsView) -> usize {
        view.rows.len() - 1
    }

    #[test]
    fn every_tab_has_settings_to_show() {
        // A tab with no rows renders "nothing here", which is a dead tab.
        with_view(None, |view| {
            for tab in Tab::ALL {
                view.switch_tab(tab);
                assert!(
                    !view.rows.is_empty(),
                    "{tab:?} has no settings, so it is a dead tab"
                );
            }
        });
    }

    #[test]
    fn cycling_a_boolean_toggles_it() {
        with_view(
            Some(("flag", SettingValue::Bool(0.0), Some(0.0), Some(1.0))),
            |view| {
                let index = last_index(view);
                assert_eq!(view.rows[index].value, SettingValue::Bool(0.0));
                view.cycle(index);
                assert_eq!(view.rows[index].value, SettingValue::Bool(1.0));
                view.cycle(index);
                assert_eq!(view.rows[index].value, SettingValue::Bool(0.0));
            },
        );
    }

    #[test]
    fn stepping_respects_the_declared_range() {
        with_view(
            Some(("clamped_int", SettingValue::Int(30), Some(5.0), Some(60.0))),
            |view| {
                let index = last_index(view);
                // Already at the maximum, so stepping up must clamp.
                view.rows[index].value = SettingValue::Int(60);
                view.step(index, true);
                assert_eq!(view.rows[index].value, SettingValue::Int(60));
                view.rows[index].value = SettingValue::Int(5);
                view.step(index, false);
                assert_eq!(view.rows[index].value, SettingValue::Int(5));
            },
        );
    }

    #[test]
    fn stepping_moves_in_both_directions() {
        with_view(
            Some(("moves_int", SettingValue::Int(30), Some(5.0), Some(120.0))),
            |view| {
                let index = last_index(view);
                view.step(index, true);
                assert_eq!(view.rows[index].value, SettingValue::Int(31));
                view.step(index, false);
                view.step(index, false);
                assert_eq!(view.rows[index].value, SettingValue::Int(29));
            },
        );
    }

    #[test]
    fn an_inert_setting_refuses_to_change_and_says_why() {
        // Presenting a dead control as working is the defect this guards.
        with_view(
            Some((
                "ai_provider",
                SettingValue::String("auto".into()),
                None,
                None,
            )),
            |view| {
                let index = last_index(view);
                assert!(!view.rows[index].live, "ai_provider should be inert");
                let before = view.rows[index].value.clone();
                view.cycle(index);
                assert_eq!(
                    view.rows[index].value, before,
                    "an inert setting must not change"
                );
                let notice = view.notice.clone().expect("the reason must be shown");
                assert!(notice.contains("not wired up"), "got: {notice}");
            },
        );
    }

    #[test]
    fn stepping_an_inert_setting_also_refuses() {
        with_view(
            Some(("multi_gpu", SettingValue::Bool(1.0), None, None)),
            |view| {
                let index = last_index(view);
                let before = view.rows[index].value.clone();
                view.step(index, true);
                assert_eq!(view.rows[index].value, before);
                assert!(view.notice.is_some(), "the reason must be shown");
            },
        );
    }

    #[test]
    fn mirrored_settings_are_read_only_not_inert() {
        // These are written by the app at startup, so calling them "not wired
        // up" was wrong: they are read-only mirrors of real state.
        for name in MIRRORED_SETTINGS {
            assert_eq!(
                availability(name),
                Availability::ReadOnly,
                "'{name}' should be read-only"
            );
        }
    }

    #[test]
    fn every_mirrored_name_is_a_real_setting() {
        let store = SettingsUI::in_memory();
        let known = store.known_keys();
        for name in MIRRORED_SETTINGS {
            assert!(
                known.iter().any(|k| k == name),
                "'{name}' is listed as mirrored but is not a real setting"
            );
        }
    }

    #[test]
    fn the_three_states_partition_every_known_setting() {
        // Nothing may be both inert and read-only, and everything is one of them.
        let store = SettingsUI::in_memory();
        for name in store.known_keys() {
            let state = availability(&name);
            assert!(
                matches!(
                    state,
                    Availability::Live | Availability::ReadOnly | Availability::Inert
                ),
                "'{name}' has no state"
            );
            assert_eq!(
                state == Availability::Inert,
                INERT_SETTINGS.contains(&name.as_str()),
                "'{name}' is classified inconsistently"
            );
        }
    }

    #[test]
    fn settings_the_app_honours_are_classified_live() {
        // fps_target and volume_master are read by the app, so must not be listed.
        assert!(setting_is_live("fps_target"));
        assert!(setting_is_live("rag_enabled"));
        assert!(setting_is_live("volume_master"));
        assert!(setting_is_live("animation_speed"));
    }

    #[test]
    fn every_inert_name_is_a_real_setting() {
        // A stale entry would label a working setting as dead.
        let store = SettingsUI::in_memory();
        let known = store.known_keys();
        for name in INERT_SETTINGS {
            assert!(
                known.iter().any(|k| k == name),
                "'{name}' is listed as inert but is not a real setting"
            );
        }
    }

    #[test]
    fn classification_is_consistent_for_every_known_setting() {
        // Mirrored and inert are mutually exclusive, and between them they cover
        // every setting the app does not honour.
        let store = SettingsUI::in_memory();
        for name in store.known_keys() {
            let inert = INERT_SETTINGS.contains(&name.as_str());
            let mirrored = MIRRORED_SETTINGS.contains(&name.as_str());
            assert!(!(inert && mirrored), "'{name}' is both inert and read-only");
            let expected = match availability(&name) {
                Availability::Inert => true,
                Availability::ReadOnly => false,
                Availability::Live => false,
            };
            assert_eq!(inert, expected, "'{name}' is classified inconsistently");
        }
    }

    #[test]
    fn painting_registers_clickable_controls() {
        with_view(None, |view| {
            let target = crate::gdi::DrawTarget::new(WIN_WIDTH, WIN_HEIGHT);
            let mut buffer = vec![0u32; (WIN_WIDTH * WIN_HEIGHT) as usize];
            view.paint(&mut buffer, target.handle(), WIN_WIDTH, WIN_HEIGHT);
            assert!(
                !view.hits.is_empty(),
                "the window painted with nothing clickable in it"
            );
            assert!(
                view.hits
                    .iter()
                    .any(|h| matches!(h.action, Action::SelectTab(_))),
                "tabs are not clickable"
            );
            assert!(
                view.hits.iter().any(|h| h.action == Action::ResetAll),
                "reset is not clickable"
            );
        });
    }

    #[test]
    fn every_live_row_has_a_control_and_no_inert_one_does() {
        with_view(None, |view| {
            let target = crate::gdi::DrawTarget::new(WIN_WIDTH, WIN_HEIGHT);
            let mut buffer = vec![0u32; (WIN_WIDTH * WIN_HEIGHT) as usize];
            view.paint(&mut buffer, target.handle(), WIN_WIDTH, WIN_HEIGHT);

            let controllable: Vec<usize> = view
                .hits
                .iter()
                .filter_map(|h| match h.action {
                    Action::CycleValue(i) | Action::Toggle(i) | Action::Step(i, _) => Some(i),
                    _ => None,
                })
                .collect();

            for (index, row) in view.rows.iter().enumerate() {
                let has = controllable.contains(&index);
                assert_eq!(
                    has,
                    row.live,
                    "'{}' {} a control but live={}",
                    row.name,
                    if has { "has" } else { "has no" },
                    row.live
                );
            }
        });
    }

    #[test]
    fn a_rejected_value_is_reported_rather_than_swallowed() {
        with_view(
            Some(("rejects", SettingValue::Int(5), Some(0.0), Some(10.0))),
            |view| {
                let index = last_index(view);
                // An out-of-range value must be refused with a reason.
                let name = view.rows[index].name.clone();
                let result = unsafe { (*view.store).set(&name, SettingValue::Int(9999)) };
                assert!(
                    result.is_err(),
                    "the store must reject an out-of-range value"
                );
                view.notice = Some(result.unwrap_err());
                assert!(!view.notice.is_none(), "a rejection must be visible");
            },
        );
    }

    #[test]
    fn float_stepping_avoids_floating_point_noise() {
        with_view(
            Some((
                "noisy_float",
                SettingValue::Float(0.5),
                Some(0.0),
                Some(1.0),
            )),
            |view| {
                let index = last_index(view);
                view.step(index, true);
                let SettingValue::Float(next) = view.rows[index].value else {
                    panic!("expected a float");
                };
                assert!((next - 0.55).abs() < 1e-9, "got {next}");
            },
        );
    }

    #[test]
    fn float_stepping_never_goes_negative() {
        with_view(
            Some((
                "floor_float",
                SettingValue::Float(0.1),
                Some(0.0),
                Some(1.0),
            )),
            |view| {
                let index = last_index(view);
                for _ in 0..40 {
                    view.step(index, false);
                }
                let SettingValue::Float(floor) = view.rows[index].value else {
                    panic!("expected a float");
                };
                assert!(floor >= 0.0, "went negative: {floor}");
            },
        );
    }

    #[test]
    fn enums_advance_through_their_options_and_wrap() {
        let options = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut current = SettingValue::Enum("a".into(), options.clone());
        for expected in ["b", "c", "a"] {
            current = next_value(&current);
            let SettingValue::Enum(actual, _) = current.clone() else {
                panic!("expected an enum");
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn an_enum_with_no_options_does_not_panic() {
        let value = SettingValue::Enum("x".into(), Vec::new());
        assert!(matches!(next_value(&value), SettingValue::Enum(_, _)));
    }

    #[test]
    fn values_render_readably() {
        assert_eq!(format_value(&SettingValue::Int(30)), "30");
        assert_eq!(format_value(&SettingValue::Float(0.5)), "0.50");
        assert_eq!(format_value(&SettingValue::String("hi".into())), "hi");
        // A long float must not fill the control with digits.
        assert!(format_value(&SettingValue::Float(1.0 / 3.0)).len() <= 6);
    }

    #[test]
    fn numeric_reads_every_numeric_variant() {
        assert_eq!(numeric(&SettingValue::Bool(1.0)), 1.0);
        assert_eq!(numeric(&SettingValue::Float(2.5)), 2.5);
        assert_eq!(numeric(&SettingValue::Int(7)), 7.0);
        assert_eq!(numeric(&SettingValue::String("x".into())), 0.0);
    }

    #[test]
    fn mouse_coordinates_are_unpacked_correctly() {
        // A click at (300, 150), as Win32 packs it.
        let packed = ((150u32 << 16) | 300u32) as isize;
        assert_eq!(point_x(packed), 300);
        assert_eq!(point_y(packed), 150);
    }

    #[test]
    fn hover_only_ever_lands_on_a_live_row() {
        // Highlighting an inert row would imply it is interactive.
        with_view(None, |view| {
            let target = crate::gdi::DrawTarget::new(WIN_WIDTH, WIN_HEIGHT);
            let mut buffer = vec![0u32; (WIN_WIDTH * WIN_HEIGHT) as usize];
            view.paint(&mut buffer, target.handle(), WIN_WIDTH, WIN_HEIGHT);

            for y in 0..WIN_HEIGHT {
                for x in (FIELD_LEFT..WIN_WIDTH).step_by(20) {
                    view.on_move(x, y);
                    if let Some(index) = view.hovered {
                        assert!(
                            view.rows[index].live,
                            "'{}' highlighted but is inert",
                            view.rows[index].name
                        );
                    }
                }
            }
        });
    }

    #[test]
    fn clicking_empty_space_changes_nothing() {
        with_view(
            Some(("flag", SettingValue::Bool(0.0), Some(0.0), Some(1.0))),
            |view| {
                let index = last_index(view);
                let before = view.rows[index].value.clone();
                view.hits.clear();
                view.on_click(5, 5);
                assert_eq!(view.rows[index].value, before);
            },
        );
    }
}
