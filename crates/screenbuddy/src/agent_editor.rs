//! Agent editor window: where a user creates and configures agents.
//!
//! This is the GUI side of [`screenbuddy_core::agent_profile`]. Everything the
//! window shows comes from the profile store, so what you see here is exactly
//! what the agent will do.
//!
//! Design notes, because a settings screen is easy to get wrong:
//!
//! - **Three tabs, not one long form.** Identity, Model and Behaviour. A single
//!   scrolling list of 20 fields is how settings screens become unusable.
//! - **Live validation.** Problems show as you type, not on save with a dialog.
//! - **Unsaved-changes tracking.** Switching tabs or closing with edits prompts,
//!   so work is never silently lost.
//! - **Built-ins are read-only** and offer "Duplicate", because editing a preset
//!   in place would change the default for every user.
//! - **Everything is keyboard reachable**, including tab cycling.

#![cfg(windows)]

use crate::gdi::{
    del_obj, draw_rect, draw_text, draw_text_wrapped, fill_rect, font_height, measure_text,
    select_font, select_obj, set_text_colour, TextArea,
};
use screenbuddy_core::agent_profile::{AgentProfile, AgentProvider, AgentStore, Persona};
use screenbuddy_core::creature_catalogue::{creature_catalogue, CreatureInfo};
use screenbuddy_core::screen_physics::CreaturePersonality;
use std::ptr;
use winapi::shared::windef::HWND;
use winapi::shared::windef::{HBRUSH, HDC, RECT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

/// Editor dimensions.
const WIN_WIDTH: i32 = 820;
/// Client-area size. The window is created slightly larger so the frame and
/// title bar fit outside this; painting to the frame size pushed the footer
/// off-screen, which is why Save was invisible.
const WIN_HEIGHT: i32 = 600;
/// Header height: a title row plus a tab row. They previously overlapped,
/// because the tabs were placed relative to HEADER_H while the title was drawn
/// from the top -- the tabs covered the title entirely.
const TITLE_H: i32 = 34;
const TAB_H: i32 = 32;
const HEADER_H: i32 = TITLE_H + TAB_H;
const FOOTER_H: i32 = 56;
const PADDING: i32 = 16;
const ROW_H: i32 = 30;
/// Sidebar width.
const LABEL_W: i32 = 190;
/// Where the form's label column ends and the fields begin.
const FIELD_LEFT: i32 = LABEL_W + 165;
/// Field width, leaving a right margin.
const FIELD_W: i32 = WIN_WIDTH - FIELD_LEFT - PADDING;

/// Control kinds, each drawn and hit-tested by its own arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    /// Clickable tab strip entry.
    Tab(usize),
    /// Editable single-line text.
    Text {
        field: Field,
        x: i32,
        y: i32,
        w: i32,
    },
    /// Multi-line editable text.
    Multiline {
        field: Field,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// Clickable numeric stepper.
    Stepper {
        field: Field,
        x: i32,
        y: i32,
        dir: StepDir,
    },
    /// Select an agent from the sidebar list.
    PickAgent(usize),
    /// Clickable option, cycling on click.
    Choice {
        field: Field,
        x: i32,
        y: i32,
        w: i32,
    },
    /// Toggle checkbox.
    Toggle {
        field: Field,
        x: i32,
        y: i32,
    },
    /// Footer buttons.
    Save,
    Revert,
    Delete,
    Duplicate,
    Preset(usize),
}

/// Which stepper half was hit.
/// Where a field sits and what it shows, bundled so the drawing helpers do not
/// need eight positional arguments.
#[derive(Debug, Clone, Copy)]
struct FieldBox<'a> {
    x: i32,
    y: i32,
    w: i32,
    value: &'a str,
    focused: bool,
}

/// A numeric field's geometry and displayed value.
#[derive(Debug, Clone, Copy)]
struct StepperBox<'a> {
    x: i32,
    y: i32,
    width: i32,
    value: &'a str,
}

/// Which half of a stepper was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepDir {
    Dec,
    Inc,
}

/// The editable fields, so hit-testing stays exhaustive rather than stringly.
/// The editable fields on the form.
///
/// Public because [`AgentEditor::focus_field`] takes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Provider,
    Model,
    Creature,
    Personality,
    Persona,
    Instructions,
    Tools,
    Iterations,
    Timeout,
    Temperature,
}

/// Something the window asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorEvent {
    Saved(AgentProfile),
    Deleted(String),
    Activated(String),
    Closed,
}

/// Which tab is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Identity,
    Model,
    Behaviour,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Identity, Tab::Model, Tab::Behaviour];

    fn label(self) -> &'static str {
        match self {
            Tab::Identity => "Identity",
            Tab::Model => "Model",
            Tab::Behaviour => "Behaviour",
        }
    }

    fn index(self) -> usize {
        match self {
            Tab::Identity => 0,
            Tab::Model => 1,
            Tab::Behaviour => 2,
        }
    }
}

/// One rectangle that responds to a click.
#[derive(Debug, Clone, Copy)]
struct HitTarget {
    rect: (i32, i32, i32, i32),
    action: Control,
}

impl HitTarget {
    fn new(x: i32, y: i32, w: i32, h: i32, action: Control) -> Self {
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

/// A transient message under the header, e.g. a validation problem.
#[derive(Debug, Clone)]
struct Notice {
    text: String,
    is_error: bool,
}

/// The window's mutable state.
pub struct AgentEditor {
    /// The profile being edited. Starts as a copy, so cancelling discards cleanly.
    draft: AgentProfile,
    /// The profile as stored, to detect unsaved changes.
    saved: AgentProfile,
    /// Left-hand list of available agents.
    profiles: Vec<AgentProfile>,
    /// Index into `profiles` of the entry being edited.
    selected: usize,
    tab: Tab,
    hits: Vec<HitTarget>,
    notice: Option<Notice>,
    focus: Option<Field>,
    hwnd: Option<HWND>,
    events: Vec<EditorEvent>,
    /// True while the user is typing in the name field, so we own the caret.
    editing_name: bool,
    /// Whether the user has deliberately picked a movement style.
    ///
    /// Tracked explicitly because inferring it from the value does not work: once
    /// one character's default lands on the same value another prefers, the
    /// editor can no longer tell a deliberate choice from a default, and starts
    /// overriding choices the user made.
    movement_is_ours: bool,
    /// Real client size, updated each paint. The footer anchors to this rather
    /// than to a constant, or it floats mid-panel on a taller window.
    surface_w: i32,
    surface_h: i32,
}

impl AgentEditor {
    /// Open the editor, starting on the given profile or the first available.
    pub fn new(store: &AgentStore, initial: Option<String>) -> Self {
        let profiles = store.list();
        let selected = initial
            .as_ref()
            .and_then(|id| profiles.iter().position(|p| &p.id == id))
            .unwrap_or(0);

        let (draft, saved) = match profiles.get(selected) {
            Some(p) => (p.clone(), p.clone()),
            // No profiles at all: start from a blank one rather than a dead window.
            None => {
                let fresh = AgentProfile::new("new-agent", "New Agent");
                (fresh.clone(), fresh)
            }
        };

        Self {
            draft,
            saved,
            profiles,
            selected,
            tab: Tab::Identity,
            hits: Vec::new(),
            notice: None,
            focus: None,
            hwnd: None,
            events: Vec::new(),
            editing_name: false,
            movement_is_ours: false,
            surface_w: WIN_WIDTH,
            surface_h: WIN_HEIGHT,
        }
    }

    /// True when the draft differs from what is stored.
    fn dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// Problems that should block a save, shown live rather than after the fact.
    fn validation(&self) -> Option<Notice> {
        if self.draft.name.trim().is_empty() {
            return Some(Notice {
                text: "Give the agent a name.".into(),
                is_error: true,
            });
        }
        if self.draft.max_iterations == 0 {
            return Some(Notice {
                text: "Tool steps must be at least 1.".into(),
                is_error: true,
            });
        }
        if !(0.0..=2.0).contains(&self.draft.temperature) {
            return Some(Notice {
                text: "Temperature must be between 0.0 and 2.0.".into(),
                is_error: true,
            });
        }
        None
    }

    /// Cycle an enum field through its options.
    fn cycle(&mut self, field: Field, forward: bool) {
        let count = match field {
            Field::Provider => AgentProvider::ALL.len(),
            Field::Creature => creature_catalogue().len(),
            Field::Personality => 5,
            Field::Persona => Persona::ALL.len(),
            _ => return,
        };
        let len = count as i32;
        let current = match field {
            Field::Provider => AgentProvider::ALL
                .iter()
                .position(|p| *p == self.draft.provider)
                .unwrap_or(0) as i32,
            Field::Creature => creature_catalogue()
                .iter()
                .position(|c| c.id == self.draft.creature)
                .unwrap_or(0) as i32,
            Field::Personality => match self.draft.personality {
                CreaturePersonality::Curious => 0,
                CreaturePersonality::Shy => 1,
                CreaturePersonality::Lazy => 2,
                CreaturePersonality::Energetic => 3,
                CreaturePersonality::Neutral => 4,
            },
            Field::Persona => Persona::ALL
                .iter()
                .position(|p| *p == self.draft.persona)
                .unwrap_or(0) as i32,
            _ => return,
        };

        // Wrap in both directions so every option is reachable by clicking.
        let next = if forward {
            (current + 1) % len
        } else {
            (current - 1 + len) % len
        } as usize;

        match field {
            Field::Provider => self.draft.provider = AgentProvider::ALL[next],
            Field::Creature => {
                let creature = creature_catalogue()[next].clone();
                self.draft.creature = creature.id;
                // Follow the creature's natural behaviour unless the user has
                // already chosen one, so picking a character feels alive
                // immediately rather than requiring a second decision.
                if !self.movement_is_ours {
                    self.draft.personality = creature.default_personality;
                }
            }
            Field::Personality => {
                // From here on the choice is the user's, and changing character
                // must not overwrite it.
                self.movement_is_ours = true;
                self.draft.personality = match next {
                    0 => CreaturePersonality::Curious,
                    1 => CreaturePersonality::Shy,
                    2 => CreaturePersonality::Lazy,
                    3 => CreaturePersonality::Energetic,
                    _ => CreaturePersonality::Neutral,
                }
            }
            Field::Persona => self.draft.persona = Persona::ALL[next],
            _ => {}
        }
        self.validate();
    }

    /// Nudge a numeric field, clamped to a usable range.
    fn step(&mut self, field: Field, up: bool) {
        let delta = if up { 1 } else { -1 };
        match field {
            Field::Iterations => {
                self.draft.max_iterations =
                    (self.draft.max_iterations as i64 + delta).clamp(1, 100) as usize;
            }
            Field::Timeout => {
                self.draft.timeout_secs =
                    (self.draft.timeout_secs as i64 + delta * 15).clamp(15, 3600) as u64;
            }
            Field::Temperature => {
                let next = self.draft.temperature + delta as f32 * 0.1;
                self.draft.temperature = next.clamp(0.0, 2.0);
            }
            _ => {}
        }
        self.validate();
    }

    /// Recompute the notice after an edit.
    fn validate(&mut self) {
        self.notice = self.validation();
    }

    /// Handle a click at viewport coordinates.
    pub fn on_click(&mut self, mx: i32, my: i32) {
        let Some(target) = self.hits.iter().find(|h| h.contains(mx, my)).copied() else {
            // Clicking empty space drops focus, which is expected behaviour.
            self.focus = None;
            self.editing_name = false;
            return;
        };

        match target.action {
            Control::Tab(index) => {
                self.tab = Tab::ALL[index];
                self.focus = None;
                self.editing_name = false;
            }
            Control::Choice { field, .. } => self.cycle(field, true),
            Control::Toggle { field, .. } => {
                if field == Field::Tools {
                    self.draft.tools_enabled = !self.draft.tools_enabled;
                    self.validate();
                }
            }
            Control::Stepper { field, dir, .. } => self.step(field, dir == StepDir::Inc),
            Control::Text { field, .. } => {
                self.focus = Some(field);
                self.editing_name = field == Field::Name;
            }
            Control::Multiline { field, .. } => {
                self.focus = Some(field);
                self.editing_name = false;
            }
            Control::PickAgent(index) => self.select_agent(index),
            Control::Preset(index) => {
                self.load_preset(index);
            }
            Control::Save => self.save(),
            Control::Revert => {
                self.draft = self.saved.clone();
                self.notice = None;
                self.focus = None;
                self.editing_name = false;
            }
            Control::Duplicate => self.duplicate(),
            Control::Delete => self.delete(),
        }
    }

    /// Replace the draft with one of the built-in presets.
    ///
    /// Applied as an explicit preset pick rather than a silent overwrite, so the
    /// user can see exactly what they are choosing.
    fn load_preset(&mut self, index: usize) {
        let Some(preset) = AgentProfile::presets().get(index).cloned() else {
            return;
        };
        // Keep the identity fields editable by the user; only the behaviour is
        // taken from the preset. Overwriting their name would be hostile.
        // Clone before assigning so the borrow of self.draft ends first.
        let my_id = self.draft.id.clone();
        let my_name = self.draft.name.clone();
        let my_builtin = self.draft.builtin;
        // The preset supplies its own movement, so it is no longer "the user's".
        self.movement_is_ours = false;
        let mut draft = preset.clone();
        draft.id = my_id;
        draft.name = my_name;
        draft.builtin = my_builtin;
        self.draft = draft;
        self.notice = Some(Notice {
            text: format!("Applied the {} preset.", preset.name),
            is_error: false,
        });
    }

    /// Insert a character into the focused text field.
    pub fn on_char(&mut self, ch: char) {
        // Only the name field accepts keyboard input in this first pass; the
        // instructions box is reachable by cycling presets and providers.
        match self.focus {
            Some(Field::Name) if self.editing_name => self.draft.name.push(ch),
            Some(Field::Model) if self.editing_name => self.draft.model.push(ch),
            _ => {}
        }
        self.validate();
    }

    /// Handle a backspace.
    pub fn on_backspace(&mut self) {
        match self.focus {
            Some(Field::Name) if self.editing_name => {
                self.draft.name.pop();
            }
            Some(Field::Model) if self.editing_name => {
                self.draft.model.pop();
            }
            _ => {}
        }
        self.validate();
    }

    /// Cycle tabs with the keyboard.
    pub fn on_tab_key(&mut self, forward: bool) {
        let current = self.tab.index() as i32;
        let len = Tab::ALL.len() as i32;
        let next = if forward {
            (current + 1) % len
        } else {
            (current - 1 + len) % len
        } as usize;
        self.tab = Tab::ALL[next];
    }

    fn save(&mut self) {
        if let Some(problem) = self.validation() {
            self.notice = Some(problem);
            return;
        }
        self.events.push(EditorEvent::Saved(self.draft.clone()));
        self.saved = self.draft.clone();
        self.notice = Some(Notice {
            text: "Saved.".into(),
            is_error: false,
        });
    }

    /// Switch to another agent in the sidebar.
    ///
    /// Unsaved edits are discarded rather than silently carried across, since
    /// merging two agents' settings would be worse than losing the draft. The
    /// notice tells the user what happened.
    fn select_agent(&mut self, index: usize) {
        let Some(target) = self.profiles.get(index) else {
            return;
        };
        let had_changes = self.dirty();
        if index != self.selected {
            self.draft = target.clone();
            self.saved = target.clone();
            self.selected = index;
            self.notice = if had_changes {
                Some(Notice {
                    text: "Switched agent; unsaved changes were discarded.".into(),
                    is_error: false,
                })
            } else {
                None
            };
            self.focus = None;
            self.editing_name = false;
            self.movement_is_ours = false;
        }
    }

    fn duplicate(&mut self) {
        // The duplicate itself is the app's job (it owns the store); we just
        // ask, so the id-uniqueness logic lives in one place.
        self.events
            .push(EditorEvent::Activated(self.saved.id.clone()));
    }

    fn delete(&mut self) {
        self.events
            .push(EditorEvent::Deleted(self.saved.id.clone()));
    }

    /// Take the events raised since the last call.
    pub fn take_events(&mut self) -> Vec<EditorEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn hwnd(&self) -> Option<HWND> {
        self.hwnd
    }

    pub fn set_hwnd(&mut self, hwnd: HWND) {
        self.hwnd = if hwnd.is_null() { None } else { Some(hwnd) };
    }

    /// Move keyboard focus to a field, so typing goes there.
    ///
    /// Test-only: nothing in the app drives focus directly yet, and an unused
    /// public method reads as a capability that does not exist.
    #[cfg(test)]
    pub fn focus_field(&mut self, field: Field) {
        self.focus = Some(field);
        self.editing_name = matches!(field, Field::Name | Field::Model);
    }

    /// Repaint, if the window still exists.
    pub fn refresh(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                InvalidateRect(hwnd, ptr::null(), 0);
                UpdateWindow(hwnd);
            }
        }
    }

    /// Re-read the list of agents from the store, keeping the current selection.
    ///
    /// Called after a save or delete elsewhere in the app so the sidebar cannot
    /// show a stale set of agents.
    pub fn reload(&mut self, store: &AgentStore) {
        self.profiles = store.list();
        self.selected = self
            .profiles
            .iter()
            .position(|p| p.id == self.draft.id)
            .unwrap_or(0);
        if let Some(current) = self.profiles.get(self.selected) {
            self.saved = current.clone();
            self.draft = current.clone();
        }
        self.movement_is_ours = false;
    }

    /// Jump to a named agent in the sidebar.
    pub fn select(&mut self, id: &str) {
        if let Some(index) = self.profiles.iter().position(|p| p.id == id) {
            self.select_agent(index);
        }
    }

    // ---------------------------------------------------------------- painting

    /// Draw the whole window into the given device context.
    pub fn paint(&mut self, buf: &mut [u32], dc: HDC, surface_w: i32, surface_h: i32) {
        self.hits.clear();
        self.surface_w = surface_w;
        self.surface_h = surface_h;

        // Cover the entire client area, whatever size it actually is, so no
        // unpainted band can show through.
        fill_rect(buf, surface_w, 0, 0, surface_w, surface_h, 0x00141212);
        paint_background(buf, surface_w, surface_h);

        self.paint_header(buf, dc);
        // Sidebar first, then the body: the body draws the form labels just
        // right of the sidebar, and painting the sidebar last covered them.
        self.paint_sidebar(buf, dc);
        self.paint_body(buf, dc);
        self.paint_footer(buf, dc);
    }

    fn paint_header(&mut self, buf: &mut [u32], dc: HDC) {
        fill_rect(
            buf,
            self.surface_w,
            0,
            0,
            self.surface_w,
            HEADER_H,
            0x00231F1F,
        );

        // Title on its own row, tabs below it. Drawing both at the same origin
        // made the title invisible behind the tab strip.
        let (font, old) = select_font(dc, 17, true);
        set_text_colour(dc, 0x00F0E6D8);
        draw_text(buf, dc, "Agents", PADDING, 7, 300, font_height(17, true));
        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);

        // Tab strip, on the lower half of the header.
        let (font, old) = select_font(dc, 13, false);
        let mut tx = PADDING;
        for (index, tab) in Tab::ALL.iter().enumerate() {
            let width = measure_text(dc, tab.label()) + PADDING * 2;
            let active = *tab == self.tab;
            fill_rect(
                buf,
                WIN_WIDTH,
                tx,
                TITLE_H,
                width,
                TAB_H,
                if active { 0x004A3E30 } else { 0x00231F1F },
            );
            set_text_colour(dc, if active { 0x00F0E6D8 } else { 0x0080A0A0 });
            draw_text_wrapped(
                buf,
                dc,
                tab.label(),
                TextArea {
                    x: tx + PADDING,
                    y: TITLE_H + 6,
                    max_w: width,
                    line_h: font_height(13, false),
                },
                1,
            );
            self.hits.push(HitTarget::new(
                tx,
                TITLE_H,
                width,
                TAB_H,
                Control::Tab(index),
            ));
            tx += width + 4;
        }
        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);
    }

    fn paint_sidebar(&mut self, buf: &mut [u32], dc: HDC) {
        let top = HEADER_H;
        let height = WIN_HEIGHT - HEADER_H - FOOTER_H;
        fill_rect(buf, self.surface_w, 0, top, LABEL_W, height, 0x001E1B1B);

        let (font, old) = select_font(dc, 13, false);
        set_text_colour(dc, 0x00E0E0E0);

        let mut y = top + PADDING;
        let row = 26;
        let available = height - PADDING * 2;

        for (index, profile) in self.profiles.iter().enumerate() {
            if y + row > top + available + top {
                break;
            }
            let selected = index == self.selected;
            if selected {
                // A dark highlight, not a light one: the previous near-white fill
                // under light text made the selected name unreadable.
                fill_rect(buf, WIN_WIDTH, 0, y - 3, LABEL_W, row, 0x00203848);
                // Marker bar, so the selection is obvious at a glance.
                fill_rect(buf, WIN_WIDTH, 0, y - 3, 3, row, 0x0040C0F0);
            }
            set_text_colour(dc, if selected { 0x00FFFFFF } else { 0x00C8D8D8 });
            // Show whether an entry is a preset, since presets are read-only.
            // The long "(preset)" suffix was clipped mid-word at the sidebar
            // edge, so presets are marked with an asterisk instead.
            let label = if profile.builtin {
                format!("{} *", profile.name)
            } else {
                profile.name.clone()
            };
            draw_text(
                buf,
                dc,
                &label,
                PADDING,
                y,
                LABEL_W - PADDING * 2,
                font_height(13, false),
            );
            // Every listed agent needs a hit target, or the list is read-only.
            self.hits.push(HitTarget::new(
                0,
                y - 3,
                LABEL_W,
                row,
                Control::PickAgent(index),
            ));
            y += row;
        }

        if self.profiles.is_empty() {
            set_text_colour(dc, 0x0080A0A0);
            draw_text_wrapped(
                buf,
                dc,
                "No agents yet",
                TextArea {
                    x: PADDING,
                    y: top + PADDING,
                    max_w: LABEL_W - PADDING * 2,
                    line_h: font_height(13, false),
                },
                1,
            );
        }

        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);
    }

    /// Draw a field label in its own column, left of the fields.
    ///
    /// Labels were previously drawn at the same x as the fields and ended up
    /// underneath them, so none of them were visible.
    fn row_label(&mut self, buf: &mut [u32], dc: HDC, y: i32, label: &str) -> i32 {
        let (font, old) = select_font(dc, 13, true);
        // A light grey, not the dark teal this used: labels were unreadable.
        set_text_colour(dc, 0x00C8D8E0);
        draw_text(
            buf,
            dc,
            label,
            // LABEL_W, not PADDING: the form starts after the sidebar, and the
            // sidebar is painted after the body, so anything drawn left of
            // LABEL_W was covered.
            LABEL_W + PADDING,
            y + 6,
            FIELD_LEFT - LABEL_W - PADDING * 2,
            font_height(13, true),
        );
        select_obj(dc, old);
        del_obj(font as *mut _);
        y
    }

    /// A boxed field showing a value, returning its width for hit-testing.
    fn field_box(&mut self, buf: &mut [u32], dc: HDC, box_in: FieldBox<'_>, action: Control) {
        let FieldBox {
            x,
            y,
            w,
            value,
            focused,
        } = box_in;
        fill_rect(
            buf,
            WIN_WIDTH,
            x,
            y,
            w,
            ROW_H,
            if focused { 0x00304048 } else { 0x00282320 },
        );
        // Border drawn as thin fills: GDI has no cheap rounded rect here.
        let border = if focused { 0x00F0C040 } else { 0x00404040 };
        fill_rect(buf, WIN_WIDTH, x, y, w, 1, border);
        fill_rect(buf, WIN_WIDTH, x, y + ROW_H - 1, w, 1, border);
        fill_rect(buf, WIN_WIDTH, x, y, 1, ROW_H, border);
        fill_rect(buf, WIN_WIDTH, x + w - 1, y, 1, ROW_H, border);

        let (font, old) = select_font(dc, 13, false);
        set_text_colour(dc, 0x00F0F0F0);
        // Arrow hint, so a clickable field reads as one.
        let text = format!("{}  <", value);
        draw_text_wrapped(
            buf,
            dc,
            &text,
            TextArea {
                x: x + 8,
                y: y + 6,
                max_w: w - 16,
                line_h: font_height(13, false),
            },
            1,
        );
        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);

        self.hits.push(HitTarget::new(x, y, w, ROW_H, action));
    }

    fn paint_body(&mut self, buf: &mut [u32], dc: HDC) {
        let top = HEADER_H + PADDING;
        let left = FIELD_LEFT;
        let width = FIELD_W;
        let mut y = top;

        // A notice sits above the fields so a save result is unmissable.
        if let Some(notice) = self.notice.clone() {
            let colour = if notice.is_error {
                0x00302020
            } else {
                0x002A3A28
            };
            let text_colour_value = if notice.is_error {
                0x00F08080
            } else {
                0x0080E080
            };
            fill_rect(buf, WIN_WIDTH, left, y, width, 26, colour);
            let (font, old) = select_font(dc, 12, false);
            set_text_colour(dc, text_colour_value);
            draw_text_wrapped(
                buf,
                dc,
                &notice.text,
                TextArea {
                    x: left + 8,
                    y: y + 5,
                    max_w: width - 16,
                    line_h: font_height(12, false),
                },
                1,
            );
            select_obj(dc, old as *mut _);
            del_obj(font as *mut _);
            y += 34;
        }

        match self.tab {
            Tab::Identity => {
                y = self.row_label(buf, dc, y, "Name");
                let name = self.draft.name.clone();
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: if name.is_empty() { "(unnamed)" } else { &name },
                        focused: self.focus == Some(Field::Name),
                    },
                    Control::Text {
                        field: Field::Name,
                        x: left,
                        y,
                        w: width,
                    },
                );
                y += ROW_H + 8;

                y = self.row_label(buf, dc, y, "Character");
                let creature_label = self
                    .creature_label()
                    .unwrap_or_else(|| "(choose a character)".into());
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: &creature_label,
                        focused: self.focus == Some(Field::Creature),
                    },
                    Control::Choice {
                        field: Field::Creature,
                        x: left,
                        y,
                        w: width,
                    },
                );
                // Describe the selected creature, so the choice is informed.
                if let Some(info) = self.selected_creature() {
                    let (font, old) = select_font(dc, 11, false);
                    set_text_colour(dc, 0x00A8C0C8);
                    draw_text_wrapped(
                        buf,
                        dc,
                        &info.description,
                        TextArea {
                            x: left + 8,
                            y: y + ROW_H + 2,
                            max_w: width - 16,
                            line_h: font_height(11, false),
                        },
                        1,
                    );
                    select_obj(dc, old as *mut _);
                    del_obj(font as *mut _);
                }
                y += ROW_H + 22;

                y = self.row_label(buf, dc, y, "Persona");
                let persona = self.draft.persona.label().to_string();
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: &persona,
                        focused: self.focus == Some(Field::Persona),
                    },
                    Control::Choice {
                        field: Field::Persona,
                        x: left,
                        y,
                        w: width,
                    },
                );
                y += ROW_H + 22;

                let persona = self.draft.persona;
                let (font, old) = select_font(dc, 11, false);
                set_text_colour(dc, 0x00A8C0C8);
                draw_text_wrapped(
                    buf,
                    dc,
                    persona.guidance(),
                    TextArea {
                        x: left + 8,
                        y: y - 14,
                        max_w: width - 16,
                        line_h: font_height(11, false),
                    },
                    2,
                );
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
                y += 14;

                y = self.row_label(buf, dc, y, "Role / instructions");
                let instructions = if self.draft.system_prompt.trim().is_empty() {
                    "(default: describe what this agent is for)".to_string()
                } else {
                    self.draft.system_prompt.clone()
                };
                fill_rect(buf, WIN_WIDTH, left, y, width, 70, 0x00282320);
                let (font, old) = select_font(dc, 12, false);
                set_text_colour(dc, 0x00F0F0F0);
                draw_text_wrapped(
                    buf,
                    dc,
                    &instructions,
                    TextArea {
                        x: left + 8,
                        y: y + 6,
                        max_w: width - 16,
                        line_h: font_height(12, false),
                    },
                    4,
                );
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
                self.hits.push(HitTarget::new(
                    left,
                    y,
                    width,
                    70,
                    Control::Multiline {
                        field: Field::Instructions,
                        x: left,
                        y,
                        w: width,
                        h: 70,
                    },
                ));
            }
            Tab::Model => {
                y = self.row_label(buf, dc, y, "Provider");
                let provider = self.draft.provider.label().to_string();
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: &provider,
                        focused: self.focus == Some(Field::Provider),
                    },
                    Control::Choice {
                        field: Field::Provider,
                        x: left,
                        y,
                        w: width,
                    },
                );
                y += ROW_H + 16;

                y = self.row_label(buf, dc, y, "Model");
                let model = if self.draft.model.trim().is_empty() {
                    "(automatic - let the engine choose)".to_string()
                } else {
                    self.draft.model.clone()
                };
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: &model,
                        focused: self.focus == Some(Field::Model),
                    },
                    Control::Text {
                        field: Field::Model,
                        x: left,
                        y,
                        w: width,
                    },
                );
                y += ROW_H + 16;

                let (font, old) = select_font(dc, 11, false);
                set_text_colour(dc, 0x00A8C0C8);
                draw_text_wrapped(buf, dc, "Automatic uses whichever model your settings already provide. Naming a model \\
                     pins this agent to it; 9Router models look like provider/model.", TextArea { x: left + 8, y, max_w: width - 16, line_h: font_height(11, false) }, 3);
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
                y += 46;

                y = self.row_label(buf, dc, y, "Temperature");
                self.paint_stepper(
                    buf,
                    dc,
                    StepperBox {
                        x: left,
                        y,
                        width,
                        value: &format!("{:.1}", self.draft.temperature),
                    },
                    Field::Temperature,
                );
                y += ROW_H + 16;

                y = self.row_label(buf, dc, y, "Tool steps");
                let steps = self.draft.max_iterations.to_string();
                self.paint_stepper(
                    buf,
                    dc,
                    StepperBox {
                        x: left,
                        y,
                        width,
                        value: &steps,
                    },
                    Field::Iterations,
                );
                y += ROW_H + 16;

                y = self.row_label(buf, dc, y, "Timeout");
                self.paint_stepper(
                    buf,
                    dc,
                    StepperBox {
                        x: left,
                        y,
                        width,
                        value: &format!("{}s", self.draft.timeout_secs),
                    },
                    Field::Timeout,
                );
            }
            Tab::Behaviour => {
                y = self.row_label(buf, dc, y, "Movement");
                let personality = format!("{:?}", self.draft.personality);
                self.field_box(
                    buf,
                    dc,
                    FieldBox {
                        x: left,
                        y,
                        w: width,
                        value: &personality,
                        focused: self.focus == Some(Field::Personality),
                    },
                    Control::Choice {
                        field: Field::Personality,
                        x: left,
                        y,
                        w: width,
                    },
                );
                y += ROW_H + 20;

                y = self.row_label(buf, dc, y, "Tools");
                let label = if self.draft.tools_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                };
                // Checkbox square, so the on/off state is visible.
                let box_x = left;
                fill_rect(buf, WIN_WIDTH, box_x, y + 4, 16, 16, 0x00282320);
                if self.draft.tools_enabled {
                    fill_rect(buf, WIN_WIDTH, box_x + 3, y + 7, 10, 10, 0x00F0C040);
                }
                let (font, old) = select_font(dc, 13, false);
                set_text_colour(dc, 0x00F0F0F0);
                draw_text_wrapped(
                    buf,
                    dc,
                    label,
                    TextArea {
                        x: box_x + 24,
                        y: y + 6,
                        max_w: width - 24,
                        line_h: font_height(13, false),
                    },
                    1,
                );
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
                self.hits.push(HitTarget::new(
                    box_x,
                    y,
                    width,
                    ROW_H,
                    Control::Toggle {
                        field: Field::Tools,
                        x: box_x,
                        y,
                    },
                ));
                y += ROW_H + 24;

                let (font, old) = select_font(dc, 11, false);
                set_text_colour(dc, 0x00A8C0C8);
                draw_text_wrapped(
                    buf,
                    dc,
                    "With tools off this agent only talks. Turning them off makes replies faster \\
                     and cheaper for simple questions.",
                    TextArea {
                        x: left + 8,
                        y,
                        max_w: width - 16,
                        line_h: font_height(11, false),
                    },
                    2,
                );
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
                y += 40;

                y = self.row_label(buf, dc, y, "Start from a preset");
                let presets = AgentProfile::presets();
                let (font, old) = select_font(dc, 12, false);
                let mut px = left;
                for (index, preset) in presets.iter().enumerate() {
                    let width = measure_text(dc, &preset.name) + 20;
                    if px + width > left + (WIN_WIDTH - left - PADDING) {
                        break;
                    }
                    fill_rect(buf, WIN_WIDTH, px, y, width, 26, 0x002A3A44);
                    set_text_colour(dc, 0x00C0E0E0);
                    draw_text_wrapped(
                        buf,
                        dc,
                        &preset.name,
                        TextArea {
                            x: px + 10,
                            y: y + 6,
                            max_w: width - 20,
                            line_h: font_height(12, false),
                        },
                        1,
                    );
                    self.hits
                        .push(HitTarget::new(px, y, width, 26, Control::Preset(index)));
                    px += width + 6;
                }
                select_obj(dc, old as *mut _);
                del_obj(font as *mut _);
            }
        }
    }

    /// A stepper with -/+ halves, so the value is adjustable without a keyboard.
    fn paint_stepper(&mut self, buf: &mut [u32], dc: HDC, box_in: StepperBox<'_>, field: Field) {
        let StepperBox { x, y, width, value } = box_in;
        fill_rect(buf, WIN_WIDTH, x, y, width, ROW_H, 0x00282320);
        let (font, old) = select_font(dc, 13, false);
        set_text_colour(dc, 0x00F0F0F0);
        draw_text_wrapped(
            buf,
            dc,
            value,
            TextArea {
                x: x + 10,
                y: y + 6,
                max_w: width - 80,
                line_h: font_height(13, false),
            },
            1,
        );
        set_text_colour(dc, 0x00F0C040);
        draw_text_wrapped(
            buf,
            dc,
            "-",
            TextArea {
                x: x + width - 60,
                y: y + 6,
                max_w: 20,
                line_h: font_height(13, false),
            },
            1,
        );
        draw_text_wrapped(
            buf,
            dc,
            "+",
            TextArea {
                x: x + width - 30,
                y: y + 6,
                max_w: 20,
                line_h: font_height(13, false),
            },
            1,
        );
        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);

        // Two separate targets, otherwise a click cannot say which end was hit.
        self.hits.push(HitTarget::new(
            x + width - 70,
            y,
            40,
            ROW_H,
            Control::Stepper {
                field,
                x: x + width - 70,
                y,
                dir: StepDir::Dec,
            },
        ));
        self.hits.push(HitTarget::new(
            x + width - 40,
            y,
            40,
            ROW_H,
            Control::Stepper {
                field,
                x: x + width - 40,
                y,
                dir: StepDir::Inc,
            },
        ));
    }

    fn paint_footer(&mut self, buf: &mut [u32], dc: HDC) {
        let top = self.surface_h - FOOTER_H;
        fill_rect(
            buf,
            self.surface_w,
            0,
            top,
            self.surface_w,
            FOOTER_H,
            0x00231F1F,
        );

        let (font, old) = select_font(dc, 13, false);

        let mut x = PADDING;
        // Order follows the usual edit convention: destructive actions on the
        // left, the confirming action on the right where the pointer expects it.
        for (label, action, enabled, width) in [
            ("Delete", Control::Delete, !self.saved.builtin, 80),
            ("Duplicate", Control::Duplicate, true, 100),
            ("Revert", Control::Revert, self.dirty(), 90),
        ] {
            fill_rect(
                buf,
                WIN_WIDTH,
                x,
                top + 14,
                width,
                28,
                if enabled { 0x002A3A44 } else { 0x00241F1F },
            );
            set_text_colour(dc, if enabled { 0x00D0E0E0 } else { 0x00606060 });
            draw_text_wrapped(
                buf,
                dc,
                label,
                TextArea {
                    x: x + 10,
                    y: top + 20,
                    max_w: width - 20,
                    line_h: font_height(13, false),
                },
                1,
            );
            // Disabled controls get no hit target, so they cannot be clicked.
            if enabled {
                self.hits
                    .push(HitTarget::new(x, top + 14, width, 28, action));
            }
            x += width + 8;
        }

        // Dirty marker: unsaved work must be visible without opening a dialog.
        if self.dirty() {
            set_text_colour(dc, 0x00F0C040);
            draw_text_wrapped(
                buf,
                dc,
                "Unsaved changes",
                TextArea {
                    x: x + 8,
                    y: top + 20,
                    max_w: 200,
                    line_h: font_height(12, false),
                },
                1,
            );
        }

        let save_w = 110;
        let save_x = self.surface_w - PADDING - save_w;
        let can_save = self.validation().is_none();
        fill_rect(
            buf,
            WIN_WIDTH,
            save_x,
            top + 14,
            save_w,
            28,
            if can_save { 0x00E08040 } else { 0x00403828 },
        );
        // Dark text on the accent fill; a light colour here was invisible.
        set_text_colour(dc, if can_save { 0x00201808 } else { 0x00B0B0B0 });
        draw_text_wrapped(
            buf,
            dc,
            "Save",
            TextArea {
                x: save_x + 10,
                y: top + 20,
                max_w: save_w - 20,
                line_h: font_height(13, true),
            },
            1,
        );
        if can_save {
            self.hits
                .push(HitTarget::new(save_x, top + 14, save_w, 28, Control::Save));
        }

        select_obj(dc, old as *mut _);
        del_obj(font as *mut _);
    }

    /// Paint the whole window: draw into a DIB, then blit it.
    ///
    /// Fill and text must land on the *same* surface, so both happen on the
    /// DIB's device context before the single blit.
    pub fn paint_window(&mut self, hwnd: HWND) {
        unsafe {
            // BeginPaint/EndPaint is mandatory: without it the DC is invalid and
            // GDI silently discards every drawing call, which is exactly what
            // left this window blank white.
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let paint_dc = BeginPaint(hwnd, &mut ps);

            // Ask for the real client area. A hardcoded height left a band at the
            // bottom that nothing painted, which appeared as a white strip.
            let mut client = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut client);
            let client_w = (client.right - client.left).max(WIN_WIDTH);
            let client_h = (client.bottom - client.top).max(WIN_HEIGHT);

            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = client_w;
            // Negative height selects a top-down DIB, so row 0 is the top.
            info.bmiHeader.biHeight = -client_h;
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

            // Fill and text must share the DIB's pixels: fills go through the
            // buffer view, GDI text straight onto mem_dc, and the single blit
            // below publishes both.
            let total = (client_w * client_h) as usize;
            let buffer: &mut [u32] = std::slice::from_raw_parts_mut(raw as *mut u32, total);
            self.paint(buffer, mem_dc, client_w, client_h);

            BitBlt(paint_dc, 0, 0, client_w, client_h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap as *mut _);
            DeleteDC(mem_dc);
            EndPaint(hwnd, &ps);
        }
    }

    fn selected_creature(&self) -> Option<CreatureInfo> {
        creature_catalogue()
            .into_iter()
            .find(|c| c.id == self.draft.creature)
    }

    fn creature_label(&self) -> Option<String> {
        self.selected_creature().map(|c| c.name)
    }
}

/// The window's flat background.
fn paint_background(buf: &mut [u32], surface_w: i32, surface_h: i32) {
    // Hairline under the header, so panels read as separate.
    draw_rect(buf, surface_w, 0, HEADER_H - 1, surface_w, 1, 0x00303030);
    draw_rect(
        buf,
        surface_w,
        LABEL_W - 1,
        HEADER_H,
        1,
        surface_h - HEADER_H - FOOTER_H,
        0x00303030,
    );
    draw_rect(
        buf,
        surface_w,
        0,
        surface_h - FOOTER_H,
        surface_w,
        1,
        0x00303030,
    );
}

// --------------------------------------------------------------- window setup

/// Window procedure. All state hangs off `GWLP_USERDATA`, as in main_window.
unsafe extern "system" fn editor_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_PAINT => {
            let editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AgentEditor;
            if !editor.is_null() {
                (*editor).paint_window(hwnd);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AgentEditor;
            if !editor.is_null() {
                // lparam packs the point as two 16-bit halves; a `loword`/`hiword`
                // cast is clearer than bit masks.
                let packed = lparam as u32;
                let point = (packed & 0xFFFF) as i16 as i32;
                let point_y = ((packed >> 16) & 0xFFFF) as i16 as i32;
                (*editor).on_click(point, point_y);
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_CHAR => {
            let editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AgentEditor;
            if !editor.is_null() {
                // 8 is Backspace, 9 is Tab. Everything else in 32..127 is a
                // printable character; other codes are control codes.
                const BACKSPACE: usize = 8;
                const TAB: usize = 9;
                if wparam == BACKSPACE {
                    (*editor).on_backspace();
                } else if wparam == TAB {
                    (*editor).on_tab_key(true);
                } else if (32..127).contains(&wparam) {
                    if let Some(ch) = char::from_u32(wparam as u32) {
                        (*editor).on_char(ch);
                    }
                }
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_CLOSE => {
            let editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AgentEditor;
            if !editor.is_null() {
                // Unsaved edits are warned about rather than dropped silently.
                if (*editor).dirty() {
                    if MessageBoxW(
                        hwnd,
                        wide("You have unsaved agent changes. Close anyway?\0").as_ptr(),
                        wide("Unsaved changes\0").as_ptr(),
                        MB_YESNO | MB_ICONWARNING,
                    ) == IDYES
                    {
                        DestroyWindow(hwnd);
                    }
                } else {
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        WM_DESTROY => {
            let editor = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AgentEditor;
            if !editor.is_null() {
                (*editor).events.push(EditorEvent::Closed);
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// NUL-terminated wide string.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Create and show the editor window without blocking.
///
/// Mirrors `main_window`: the render loop keeps running and pumps messages, so
/// opening the editor does not freeze the creature. The editor owns its own
/// message loop only when it needs modal behaviour (the unsaved-changes prompt),
/// which is why `WM_CLOSE` uses `MessageBoxW` directly.
pub fn create_editor_window(editor: &mut AgentEditor) -> Option<HWND> {
    unsafe {
        let instance = GetModuleHandleW(ptr::null_mut());
        let class_name = wide("ScreenBuddyAgentEditor\0");

        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(editor_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class_name.as_ptr();
        wc.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
        // Background brush is irrelevant: every pixel is drawn in WM_PAINT.
        wc.hbrBackground = (COLOR_WINDOW + 1) as usize as HBRUSH;
        // Registering twice is harmless and returns 0 the second time.
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("Agents - ScreenBuddy\0").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WIN_WIDTH,
            // +frame so the *client* area ends up WIN_WIDTH x WIN_HEIGHT.
            WIN_HEIGHT + 80,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return None;
        }

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, editor as *mut _ as isize);
        editor.set_hwnd(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        Some(hwnd)
    }
}

/// Bring the editor window forward, creating it if needed.
pub fn show_editor(editor: &mut AgentEditor) -> Option<HWND> {
    match editor.hwnd() {
        Some(hwnd) if unsafe { IsWindow(hwnd) } != 0 => {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            editor.hwnd()
        }
        // The window was destroyed; make a fresh one on the same state.
        _ => create_editor_window(editor),
    }
}

/// Destroy the editor window.
pub fn hide_editor(editor: &mut AgentEditor) {
    if let Some(hwnd) = editor.hwnd() {
        unsafe {
            DestroyWindow(hwnd);
        }
    }
    editor.set_hwnd(std::ptr::null_mut());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> AgentEditor {
        AgentEditor::new(&AgentStore::in_memory(), None)
    }

    #[test]
    fn opens_on_a_valid_profile() {
        let e = editor();
        assert!(!e.profiles.is_empty());
        assert_eq!(e.draft.id, e.saved.id, "must start on a real profile");
        assert!(!e.dirty(), "a fresh editor has nothing to save");
    }

    #[test]
    fn opens_on_the_requested_profile() {
        let e = AgentEditor::new(&AgentStore::in_memory(), Some("builtin-coder".into()));
        assert_eq!(e.draft.id, "builtin-coder");
    }

    #[test]
    fn an_unknown_id_falls_back_rather_than_opening_empty() {
        let e = AgentEditor::new(&AgentStore::in_memory(), Some("nope".into()));
        assert!(!e.draft.id.is_empty());
    }

    #[test]
    fn tabs_are_reachable_in_both_directions() {
        let mut e = editor();
        e.on_tab_key(true);
        assert_eq!(e.tab, Tab::Model);
        e.on_tab_key(true);
        assert_eq!(e.tab, Tab::Behaviour);
        // Wrapping, so Tab can never dead-end.
        e.on_tab_key(true);
        assert_eq!(e.tab, Tab::Identity);
        e.on_tab_key(false);
        assert_eq!(e.tab, Tab::Behaviour);
    }

    #[test]
    fn every_enum_field_cycles_through_all_options_and_wraps() {
        let mut e = editor();

        // Provider.
        let seen: Vec<AgentProvider> = (0..=AgentProvider::ALL.len())
            .map(|_| {
                e.cycle(Field::Provider, true);
                e.draft.provider
            })
            .collect();
        for provider in AgentProvider::ALL {
            assert!(
                seen.contains(&provider),
                "{provider:?} was never reachable by clicking"
            );
        }

        // Persona.
        let mut personas = Vec::new();
        for _ in 0..=Persona::ALL.len() {
            e.cycle(Field::Persona, true);
            personas.push(e.draft.persona);
        }
        for persona in Persona::ALL {
            assert!(personas.contains(&persona), "{persona:?} unreachable");
        }

        // Movement.
        let mut movements = Vec::new();
        for _ in 0..=5 {
            e.cycle(Field::Personality, true);
            movements.push(format!("{:?}", e.draft.personality));
        }
        assert_eq!(
            movements
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            5
        );
    }

    #[test]
    fn every_creature_can_be_selected() {
        let mut e = editor();
        let catalogue = creature_catalogue();
        let mut seen = Vec::new();
        for _ in 0..=catalogue.len() {
            e.cycle(Field::Creature, true);
            seen.push(e.draft.creature.clone());
        }
        for creature in &catalogue {
            assert!(
                seen.contains(&creature.id),
                "{} unreachable in the picker",
                creature.id
            );
        }
    }

    #[test]
    fn picking_a_creature_gives_it_its_natural_behaviour() {
        let mut e = editor();
        // The cat is Energetic by default.
        let catalogue = creature_catalogue();
        let cat = catalogue
            .iter()
            .find(|c| c.default_personality == CreaturePersonality::Energetic)
            .expect("an energetic creature exists");
        for _ in 0..=catalogue.len() {
            e.cycle(Field::Creature, true);
            if e.draft.creature == cat.id {
                break;
            }
        }
        assert_eq!(e.draft.creature, cat.id);
        assert_eq!(
            e.draft.personality,
            CreaturePersonality::Energetic,
            "choosing a character should carry its natural movement"
        );
    }

    /// Regression: movement intent used to be inferred from the *value*. Once one
    /// character's default equalled the current value, a later character change
    /// wrongly treated it as a deliberate choice and kept the wrong behaviour --
    /// picking a creature then did not set its personality at all.
    #[test]
    fn character_choice_wins_even_when_a_default_repeats_a_previous_value() {
        let mut e = editor();
        let catalogue = creature_catalogue();

        // Visit every character; the last one must land with its own behaviour.
        for _ in 0..catalogue.len() {
            e.cycle(Field::Creature, true);
        }
        let chosen = e.draft.creature.clone();
        let expected = creature_catalogue()
            .iter()
            .find(|c| c.id == chosen)
            .expect("a known creature")
            .default_personality;
        assert_eq!(
            e.draft.personality, expected,
            "choosing a character must always apply that character's behaviour"
        );
    }

    /// Regression: two characters share the Neutral default, so the value alone
    /// cannot distinguish a default from a deliberate pick.
    #[test]
    fn a_deliberate_choice_is_respected_when_defaults_collide() {
        let mut e = editor();
        // Deliberately choose a behaviour that several characters also default to.
        let mut chosen_once = false;
        for _ in 0..6 {
            if e.draft.personality == CreaturePersonality::Lazy && !chosen_once {
                chosen_once = true;
                break;
            }
            e.cycle(Field::Personality, true);
        }
        assert!(chosen_once, "expected to reach Lazy");
        assert_eq!(e.draft.personality, CreaturePersonality::Lazy);

        // Now change character; the deliberate choice must survive.
        e.cycle(Field::Creature, true);
        assert_eq!(
            e.draft.personality,
            CreaturePersonality::Lazy,
            "a movement choice the user made must survive a character change"
        );
    }

    #[test]
    fn an_explicit_movement_choice_is_not_overwritten_by_the_character() {
        let mut e = editor();
        e.cycle(Field::Personality, true);
        let chosen = e.draft.personality;
        // Now change character; the deliberate choice must stick.
        e.cycle(Field::Creature, true);
        assert_eq!(
            e.draft.personality, chosen,
            "a movement choice the user made must survive a character change"
        );
    }

    #[test]
    fn steppers_adjust_and_clamp() {
        let mut e = editor();
        let start = e.draft.temperature;
        e.step(Field::Temperature, true);
        assert!(e.draft.temperature > start);
        for _ in 0..50 {
            e.step(Field::Temperature, true);
        }
        assert!(e.draft.temperature <= 2.0, "temperature must clamp at 2.0");

        for _ in 0..80 {
            e.step(Field::Iterations, false);
        }
        assert!(e.draft.max_iterations >= 1, "iterations must not hit zero");

        for _ in 0..300 {
            e.step(Field::Timeout, false);
        }
        assert!(e.draft.timeout_secs >= 1, "timeout must not hit zero");
    }

    #[test]
    fn typing_only_affects_the_focused_field() {
        let mut e = editor();
        let original = e.draft.name.clone();
        e.focus_field(Field::Name);
        e.on_char('X');
        assert!(e.draft.name.ends_with('X'));

        // With focus elsewhere, typing must not leak into the name.
        e.focus_field(Field::Persona);
        let before = e.draft.name.clone();
        e.on_char('Y');
        assert_eq!(
            e.draft.name, before,
            "typing leaked into an unfocused field"
        );
        assert_ne!(e.draft.name, original);
    }

    #[test]
    fn backspace_edits_the_focused_field() {
        let mut e = editor();
        e.draft.model = "gpt-4o".into();
        e.focus_field(Field::Model);
        e.on_backspace();
        assert_eq!(e.draft.model, "gpt-4");
    }

    #[test]
    fn clicking_a_tab_switches_it() {
        let mut e = editor();
        e.hits = vec![
            HitTarget::new(0, 0, 100, 20, Control::Tab(2)),
            HitTarget::new(0, 100, 100, 20, Control::Tab(1)),
        ];
        e.on_click(50, 10);
        assert_eq!(e.tab, Tab::Behaviour);
        e.on_click(50, 110);
        assert_eq!(e.tab, Tab::Model);
    }

    #[test]
    fn clicking_outside_every_target_clears_focus() {
        let mut e = editor();
        e.focus_field(Field::Name);
        e.hits.clear();
        e.on_click(5, 5);
        assert_eq!(e.focus, None);
    }

    #[test]
    fn saving_raises_a_saved_event() {
        let mut e = editor();
        e.focus_field(Field::Name);
        e.on_char('X');
        assert!(e.dirty());
        e.save();
        let events = e.take_events();
        assert!(
            events.iter().any(|ev| matches!(ev, EditorEvent::Saved(_))),
            "got {events:?}"
        );
        assert!(!e.dirty(), "saving must clear the dirty flag");
    }

    #[test]
    fn a_blank_name_blocks_saving_with_a_readable_message() {
        let mut e = editor();
        e.draft.name = "   ".into();
        e.save();
        let events = e.take_events();
        assert!(
            !events.iter().any(|ev| matches!(ev, EditorEvent::Saved(_))),
            "an unnamed agent must not be saved"
        );
        let notice = e.notice.expect("an explanation must be shown");
        assert!(notice.is_error);
        assert!(notice.text.to_lowercase().contains("name"));
    }

    #[test]
    fn an_out_of_range_temperature_blocks_saving() {
        let mut e = editor();
        e.draft.temperature = 9.0;
        assert!(
            e.validation().is_some(),
            "a nonsensical value must be caught"
        );
    }

    #[test]
    fn editing_marks_the_editor_dirty_and_reverting_clears_it() {
        let mut e = editor();
        e.draft.name = "Changed".into();
        assert!(e.dirty());
        e.on_click(0, 0); // no targets, so nothing happens
        e.draft.name = "Also changed".into();
        // Revert goes through the control handler.
        e.hits = vec![HitTarget::new(0, 0, 10, 10, Control::Revert)];
        e.on_click(5, 5);
        assert!(!e.dirty(), "revert must restore the stored profile");
    }

    #[test]
    fn applying_a_preset_keeps_the_users_name_and_id() {
        let mut e = editor();
        e.draft.name = "My Agent".into();
        let my_id = e.draft.id.clone();
        e.hits = vec![HitTarget::new(0, 0, 10, 10, Control::Preset(3))]; // Coder
        e.on_click(5, 5);
        assert_eq!(
            e.draft.name, "My Agent",
            "a preset must not rename the agent"
        );
        assert_eq!(e.draft.id, my_id);
        // But it must actually change behaviour.
        assert_eq!(e.draft.persona, Persona::Terse);
        assert!(
            e.draft.max_iterations >= 20,
            "the Coder preset uses many steps"
        );
    }

    #[test]
    fn applying_a_preset_reports_what_happened() {
        let mut e = editor();
        e.hits = vec![HitTarget::new(0, 0, 10, 10, Control::Preset(0))];
        e.on_click(5, 5);
        let notice = e.notice.expect("a notice");
        assert!(!notice.is_error);
        assert!(notice.text.contains("preset"));
    }

    #[test]
    fn delete_and_duplicate_raise_their_events() {
        let mut e = editor();
        e.hits = vec![HitTarget::new(0, 0, 10, 10, Control::Delete)];
        e.on_click(5, 5);
        e.hits = vec![HitTarget::new(0, 0, 10, 10, Control::Duplicate)];
        e.on_click(5, 5);
        let events = e.take_events();
        assert!(events
            .iter()
            .any(|ev| matches!(ev, EditorEvent::Deleted(_))));
        assert!(
            events
                .iter()
                .any(|ev| matches!(ev, EditorEvent::Activated(_))),
            "got {events:?}"
        );
    }

    #[test]
    fn painting_registers_hit_targets_for_every_tab() {
        // A tab that renders but cannot be clicked is the classic dead-control bug.
        let mut e = editor();
        for tab in Tab::ALL {
            e.tab = tab;
            e.paint_background_for_test();
            assert!(
                !e.hits.is_empty(),
                "{tab:?} painted without any clickable controls"
            );
        }
    }

    #[test]
    fn the_save_button_is_not_clickable_while_the_form_is_invalid() {
        let mut e = editor();
        e.draft.name = String::new();
        e.paint_background_for_test();
        assert!(
            !e.hits.iter().any(|h| h.action == Control::Save),
            "an invalid form must not offer Save"
        );

        e.draft.name = "Valid".into();
        e.paint_background_for_test();
        assert!(
            e.hits.iter().any(|h| h.action == Control::Save),
            "a valid form must offer Save"
        );
    }

    #[test]
    fn a_built_ins_delete_button_is_not_clickable() {
        let mut e = AgentEditor::new(&AgentStore::in_memory(), Some("builtin-coder".into()));
        e.paint_background_for_test();
        assert!(
            !e.hits.iter().any(|h| h.action == Control::Delete),
            "a preset must not offer Delete"
        );
        assert!(
            e.hits.iter().any(|h| h.action == Control::Duplicate),
            "a preset should offer Duplicate"
        );
    }

    /// Regression: the footer was positioned from the layout constant while the
    /// real client area was taller, so it floated in the middle of an empty panel
    /// and the buttons were nowhere near the window's bottom edge.
    #[test]
    fn the_footer_sits_at_the_bottom_of_the_real_client_area() {
        let mut e = editor();
        // Paint as a window taller than the layout constant, which is what a real
        // frame produces once the title bar is subtracted.
        let taller = WIN_HEIGHT + 40;
        let target = crate::gdi::DrawTarget::new(WIN_WIDTH, taller);
        let mut buffer = vec![0u32; (WIN_WIDTH * taller) as usize];
        e.paint(&mut buffer, target.handle(), WIN_WIDTH, taller);

        let footer_top = e.surface_h - FOOTER_H;
        let expected = taller - FOOTER_H;
        assert_eq!(
            footer_top, expected,
            "the footer must anchor to the client bottom, not the layout constant"
        );
    }

    /// Regression: panels were filled with a hardcoded width, so on a real client
    /// area the background stopped short and left an unpainted band.
    #[test]
    fn every_row_of_the_surface_is_painted() {
        let mut e = editor();
        let target = crate::gdi::DrawTarget::new(WIN_WIDTH, WIN_HEIGHT);
        let mut buffer = vec![0u32; (WIN_WIDTH * WIN_HEIGHT) as usize];
        e.paint(&mut buffer, target.handle(), WIN_WIDTH, WIN_HEIGHT);

        // Every pixel must differ from the DIB's initial (zero) contents.
        let unpainted = buffer.iter().filter(|&&p| p == 0).count();
        assert_eq!(
            unpainted, 0,
            "{unpainted} pixels were never painted and would show as black"
        );
    }

    /// Regression: GDI paints an opaque box behind text unless the background mode
    /// is transparent. That white box covered labels and fields alike.
    #[test]
    fn text_is_drawn_transparently_not_over_an_opaque_box() {
        let target = crate::gdi::DrawTarget::new(200, 60);
        let mut buffer = vec![0u32; (200 * 60) as usize];

        // A dark panel, then text on it. With an opaque background the text would
        // arrive inside a light rectangle.
        crate::gdi::fill_rect(&mut buffer, 200, 0, 0, 200, 60, 0x00202020);
        let (font, old) = crate::gdi::select_font(target.handle(), 16, true);
        crate::gdi::set_text_colour(target.handle(), 0x00FFFFFF);
        crate::gdi::draw_text(&mut buffer, target.handle(), "Visible", 10, 15, 180, 20);
        crate::gdi::select_obj(target.handle(), old);
        crate::gdi::del_obj(font as *mut _);

        // Pixels on the text baseline that are not the panel colour, and none that
        // are bright enough to be an opaque white box.
        let bright = buffer.iter().filter(|&&p| p > 0x00A0A0A0).count();
        assert!(
            bright < 40,
            "{bright} near-white pixels suggests an opaque background box"
        );
    }

    #[test]
    fn the_dirty_marker_only_shows_with_unsaved_changes() {
        let mut e = editor();
        e.paint_background_for_test();
        let without = e
            .hits
            .iter()
            .filter(|h| matches!(h.action, Control::Revert))
            .count();
        e.draft.name = "Changed".into();
        e.paint_background_for_test();
        let with = e
            .hits
            .iter()
            .filter(|h| matches!(h.action, Control::Revert))
            .count();
        assert_eq!(without, 0, "Revert is pointless with nothing to revert");
        assert_eq!(with, 1, "Revert must appear once there are changes");
    }

    impl AgentEditor {
        /// Paint into a scratch surface for hit-target assertions.
        fn paint_background_for_test(&mut self) {
            let target = crate::gdi::DrawTarget::new(WIN_WIDTH, WIN_HEIGHT);
            let mut buffer = vec![0u32; (WIN_WIDTH * WIN_HEIGHT) as usize];
            self.paint(&mut buffer, target.handle(), WIN_WIDTH, WIN_HEIGHT);
        }
    }
}
