//! Settings UI for ScreenBuddy
//!
//! Provides a configuration panel for runtime settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Settings categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingsCategory {
    General,
    Audio,
    Animation,
    AI,
    Creatures,
    Performance,
}

impl SettingsCategory {
    /// A short display name for the setting description.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Audio => "Audio",
            Self::Animation => "Animation",
            Self::AI => "AI",
            Self::Creatures => "Creatures",
            Self::Performance => "Performance",
        }
    }
}

/// A single setting value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettingValue {
    Bool(f64),
    Int(i64),
    Float(f64),
    String(String),
    Enum(String, Vec<String>),
}

/// Setting definition with metadata
#[derive(Debug, Clone)]
pub struct SettingDef {
    pub name: String,
    pub description: String,
    pub category: SettingsCategory,
    pub default: SettingValue,
    pub min: Option<SettingValue>,
    pub max: Option<SettingValue>,
}

/// Settings manager - handles persistence and change notification
#[derive(Debug, Clone)]
pub struct SettingsUI {
    settings: Arc<Mutex<HashMap<String, SettingValue>>>,
    definitions: Vec<SettingDef>,
    changed: bool,
    /// Where settings are written. Empty means "in memory only", which is what
    /// tests use so they never touch the real file.
    path: std::path::PathBuf,
    /// Applied changes, oldest first, for undo.
    undo_stack: Vec<Change>,
    /// Undone changes, most recently undone last, for redo.
    redo_stack: Vec<Change>,
}

/// One setting change, kept so it can be reversed.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub name: String,
    pub before: SettingValue,
    pub after: SettingValue,
}

/// How many changes to keep. A settings session is short; an unbounded history
/// would only grow for the life of the process.
const MAX_HISTORY: usize = 100;

impl Default for SettingsUI {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsUI {
    pub fn new() -> Self {
        let mut s = Self {
            settings: Arc::new(Mutex::new(HashMap::new())),
            definitions: Vec::new(),
            changed: false,
            path: crate::paths::data_file("settings.json"),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        s.register_defaults();
        // Restore what the user last chose; the defaults above remain the fallback.
        s.load();
        s
    }

    /// A store that never touches disk, for tests.
    pub fn in_memory() -> Self {
        let mut s = Self {
            settings: Arc::new(Mutex::new(HashMap::new())),
            definitions: Vec::new(),
            changed: false,
            path: std::path::PathBuf::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        s.register_defaults();
        s
    }

    /// A shareable read-only handle.
    ///
    /// The value map is already behind an `Arc<Mutex<_>>`, so this is a cheap
    /// clone that can be handed to the IPC layer while the render loop keeps
    /// mutating the owner.
    pub fn reader(&self) -> SettingsReader {
        SettingsReader {
            settings: Arc::clone(&self.settings),
        }
    }

    /// Write the current settings to disk.
    ///
    /// Best-effort by design: a settings file that cannot be written must not stop
    /// the app from running.
    pub fn save(&self) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let snapshot: HashMap<String, SettingValue> = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .clone();
        let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    /// Write the current settings to an explicit path.
    ///
    /// Separate from `save`, which writes the app's own file: this is for a user
    /// keeping a named profile to move between machines. Returns how many
    /// settings were written so the caller can confirm the file is not empty.
    pub fn export_to(&self, path: &std::path::Path) -> Result<usize, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let snapshot: HashMap<String, SettingValue> = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .clone();
        let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(snapshot.len())
    }

    /// Clamp a value to the definition's declared bounds.
    ///
    /// Only used by import. `set()` deliberately rejects an out-of-range value,
    /// which is right for a single control edit but wrong for a profile: a file
    /// written elsewhere may carry a value this build's definition refuses, and
    /// clamping turns that into a working setting rather than a silently
    /// dropped one. Enum and String have no numeric bounds and pass through.
    fn clamp_to_definition(&self, name: &str, value: SettingValue) -> SettingValue {
        let Some(def) = self.definitions.iter().find(|d| d.name == name) else {
            return value;
        };
        match (value, &def.min, &def.max) {
            (
                SettingValue::Float(v),
                Some(SettingValue::Float(lo)),
                Some(SettingValue::Float(hi)),
            ) => SettingValue::Float(v.clamp(*lo, *hi)),
            (SettingValue::Int(v), Some(SettingValue::Int(lo)), Some(SettingValue::Int(hi))) => {
                SettingValue::Int(v.clamp(*lo, *hi))
            }
            (other, _, _) => other,
        }
    }

    /// Read settings from an explicit path and apply the ones this build knows.
    ///
    /// Unknown keys are counted and skipped rather than rejected, so a profile
    /// written by a newer build still loads on an older one. Returns
    /// `(applied, skipped)`.
    pub fn import_from(&mut self, path: &std::path::Path) -> Result<(usize, Vec<String>), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let stored = serde_json::from_str::<HashMap<String, SettingValue>>(&text)
            .map_err(|e| e.to_string())?;
        let known: std::collections::HashSet<String> =
            self.definitions.iter().map(|d| d.name.clone()).collect();
        let mut applied = 0usize;
        let mut skipped = Vec::new();
        for (key, value) in stored {
            if !known.contains(&key) {
                skipped.push(key);
                continue;
            }
            // Clamp to the definition's bounds first: set() rejects an
            // out-of-range value, which would drop the key entirely.
            let value = self.clamp_to_definition(&key, value);
            // Then go through set() so the change is recorded for undo and the
            // rest of the validation still applies.
            if self.set(&key, value).is_ok() {
                applied += 1;
            }
        }
        Ok((applied, skipped))
    }

    /// Read settings back, ignoring anything unrecognised.
    ///
    /// Only keys this build defines are accepted, so a stale or hand-edited file
    /// cannot inject settings the app would then honour.
    pub fn load(&mut self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return;
        };
        let Ok(stored) = serde_json::from_str::<HashMap<String, SettingValue>>(&text) else {
            // A corrupt file should not stop the app; defaults remain in force.
            return;
        };
        let known: std::collections::HashSet<String> =
            self.definitions.iter().map(|d| d.name.clone()).collect();
        let Ok(mut settings) = self.settings.lock() else {
            return;
        };
        for (key, value) in stored {
            if known.contains(&key) {
                settings.insert(key, value);
            }
        }
    }

    fn register_defaults(&mut self) {
        self.definitions = vec![
            SettingDef {
                name: "animation_speed".to_string(),
                description: "Animation playback speed multiplier".to_string(),
                category: SettingsCategory::Animation,
                default: SettingValue::Float(1.0),
                min: Some(SettingValue::Float(0.1)),
                max: Some(SettingValue::Float(5.0)),
            },
            SettingDef {
                name: "fps_target".to_string(),
                description: "Target frames per second".to_string(),
                category: SettingsCategory::Animation,
                default: SettingValue::Int(30),
                min: Some(SettingValue::Int(15)),
                max: Some(SettingValue::Int(120)),
            },
            SettingDef {
                name: "creature_count".to_string(),
                description: "Number of creatures on screen".to_string(),
                category: SettingsCategory::Creatures,
                default: SettingValue::Int(6),
                min: Some(SettingValue::Int(1)),
                max: Some(SettingValue::Int(20)),
            },
            SettingDef {
                name: "collision_avoidance".to_string(),
                description: "Enable creature collision avoidance".to_string(),
                category: SettingsCategory::Creatures,
                default: SettingValue::Bool(1.0),
                min: None,
                max: None,
            },
            SettingDef {
                name: "cursor_interaction".to_string(),
                description: "Creatures react to cursor movement".to_string(),
                category: SettingsCategory::Creatures,
                default: SettingValue::Bool(1.0),
                min: None,
                max: None,
            },
            SettingDef {
                name: "volume_master".to_string(),
                description: "Master volume (0.0 to 1.0)".to_string(),
                category: SettingsCategory::Audio,
                default: SettingValue::Float(0.7),
                min: Some(SettingValue::Float(0.0)),
                max: Some(SettingValue::Float(1.0)),
            },
            SettingDef {
                name: "volume_effects".to_string(),
                description: "Sound effects volume".to_string(),
                category: SettingsCategory::Audio,
                default: SettingValue::Float(0.8),
                min: Some(SettingValue::Float(0.0)),
                max: Some(SettingValue::Float(1.0)),
            },
            SettingDef {
                name: "volume_music".to_string(),
                description: "Background music volume".to_string(),
                category: SettingsCategory::Audio,
                default: SettingValue::Float(0.5),
                min: Some(SettingValue::Float(0.0)),
                max: Some(SettingValue::Float(1.0)),
            },
            SettingDef {
                name: "log_level".to_string(),
                description: "Log verbosity: off, error, warn, info, debug, trace".to_string(),
                category: SettingsCategory::General,
                // Consumed by logging::init at startup. An unknown value falls
                // back to info rather than off, so a typo cannot silence logs.
                default: SettingValue::Enum(
                    "info".to_string(),
                    vec![
                        "off".to_string(),
                        "error".to_string(),
                        "warn".to_string(),
                        "info".to_string(),
                        "debug".to_string(),
                        "trace".to_string(),
                    ],
                ),
                min: None,
                max: None,
            },
            SettingDef {
                name: "theme".to_string(),
                description: "Colour theme: dark, light, or high_contrast".to_string(),
                category: SettingsCategory::General,
                // Consumed by Theme::from_name via main_ui.set_theme, which the
                // render loop applies when this changes.
                default: SettingValue::Enum(
                    "dark".to_string(),
                    vec![
                        "dark".to_string(),
                        "light".to_string(),
                        "high_contrast".to_string(),
                    ],
                ),
                min: None,
                max: None,
            },
            SettingDef {
                name: "ai_provider".to_string(),
                description: "AI provider to use".to_string(),
                category: SettingsCategory::AI,
                // Kept in step with AiEngine::parse_backend, which is what
                // actually consumes this: a provider missing here is a provider
                // the user cannot select.
                default: SettingValue::Enum(
                    "ollama".to_string(),
                    vec![
                        "ollama".to_string(),
                        "openai".to_string(),
                        "anthropic".to_string(),
                        "9router".to_string(),
                        "llamacpp".to_string(),
                        "custom".to_string(),
                    ],
                ),
                min: None,
                max: None,
            },
            SettingDef {
                name: "ai_model".to_string(),
                description: "AI model name".to_string(),
                category: SettingsCategory::AI,
                // A real catalogue entry. The previous default, "llama3", matches
                // no model the engine knows about, so every request 404'd and the
                // user was shown a canned fallback rather than an error.
                default: SettingValue::String("llama3.2".to_string()),
                min: None,
                max: None,
            },
            SettingDef {
                name: "rag_enabled".to_string(),
                description: "Enable RAG memory".to_string(),
                category: SettingsCategory::AI,
                default: SettingValue::Bool(1.0),
                min: None,
                max: None,
            },
            SettingDef {
                name: "thread_pool_size".to_string(),
                description: "Worker thread count (0 = auto)".to_string(),
                category: SettingsCategory::Performance,
                default: SettingValue::Int(0),
                min: Some(SettingValue::Int(0)),
                max: Some(SettingValue::Int(32)),
            },
            SettingDef {
                name: "multi_gpu".to_string(),
                description: "Use all available GPUs".to_string(),
                category: SettingsCategory::Performance,
                default: SettingValue::Bool(1.0),
                min: None,
                max: None,
            },
            SettingDef {
                name: "window_transparency".to_string(),
                description: "Window transparency level".to_string(),
                category: SettingsCategory::General,
                default: SettingValue::Float(1.0),
                min: Some(SettingValue::Float(0.3)),
                max: Some(SettingValue::Float(1.0)),
            },
            SettingDef {
                name: "always_on_top".to_string(),
                description: "Keep window always on top".to_string(),
                category: SettingsCategory::General,
                default: SettingValue::Bool(1.0),
                min: None,
                max: None,
            },
        ];

        // Apply defaults
        let mut settings = self.settings.lock().unwrap();
        for def in &self.definitions {
            settings.insert(def.name.clone(), def.default.clone());
        }
    }

    /// Record an applied change for undo, and drop the redo history because a
    /// new change makes any undone branch unreachable.
    fn record_change(&mut self, name: &str, before: SettingValue, after: SettingValue) {
        self.undo_stack.push(Change {
            name: name.to_string(),
            before,
            after,
        });
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Whether there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Reverse the most recent change. Returns what was undone.
    pub fn undo(&mut self) -> Option<Change> {
        let change = self.undo_stack.pop()?;
        // Write directly rather than through set(), which would record the undo
        // itself as a new change and make the history grow on every reversal.
        if let Ok(mut s) = self.settings.lock() {
            s.insert(change.name.clone(), change.before.clone());
        }
        let _ = self.save();
        self.redo_stack.push(change.clone());
        Some(change)
    }

    /// Reapply the most recently undone change.
    pub fn redo(&mut self) -> Option<Change> {
        let change = self.redo_stack.pop()?;
        if let Ok(mut s) = self.settings.lock() {
            s.insert(change.name.clone(), change.after.clone());
        }
        let _ = self.save();
        self.undo_stack.push(change.clone());
        Some(change)
    }

    /// Forget the history, e.g. after a bulk reset that is not meant to be
    /// reversible step by step.
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn get(&self, name: &str) -> Option<SettingValue> {
        self.settings.lock().unwrap().get(name).cloned()
    }

    pub fn set(&mut self, name: &str, value: SettingValue) -> Result<(), String> {
        let mut settings = self.settings.lock().unwrap();
        if let Some(def) = self.definitions.iter().find(|d| d.name == name) {
            // Validate range
            match (&value, &def.min, &def.max) {
                (
                    SettingValue::Float(v),
                    Some(SettingValue::Float(min)),
                    Some(SettingValue::Float(max)),
                ) => {
                    if *v < *min || *v > *max {
                        return Err(format!("Value {} not in range [{}, {}]", v, min, max));
                    }
                }
                (
                    SettingValue::Int(v),
                    Some(SettingValue::Int(min)),
                    Some(SettingValue::Int(max)),
                ) if (*v < *min || *v > *max) => {
                    return Err(format!("Value {} not in range [{}, {}]", v, min, max));
                }
                _ => {}
            }
            // Record the previous value for undo, but only when it actually
            // changes: a no-op set must not fill the history with nothing.
            let previous = settings.get(name).cloned();
            if previous.as_ref() == Some(&value) {
                return Ok(());
            }
            settings.insert(name.to_string(), value.clone());
            drop(settings);
            if let Some(prev) = previous {
                self.record_change(name, prev, value);
            }
            self.changed = true;
            // Persist immediately. The user will not press "save", and a setting
            // that only lives in memory is indistinguishable from one that does
            // not work.
            let _ = self.save();
            Ok(())
        } else {
            Err(format!("Unknown setting: {}", name))
        }
    }

    /// Read a setting as a number, whatever its stored representation.
    ///
    /// `SettingValue` is a tagged enum and a "bool" holds a float, so callers that
    /// only want the magnitude should not repeat that match.
    pub fn number(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            SettingValue::Bool(v) | SettingValue::Float(v) => Some(v),
            SettingValue::Int(v) => Some(v as f64),
            _ => None,
        }
    }

    /// Read a setting as text. An Enum returns its selected option; a String
    /// returns its value. Anything else is `None`.
    pub fn text(&self, name: &str) -> Option<String> {
        match self.get(name)? {
            SettingValue::String(v) => Some(v),
            SettingValue::Enum(v, _) => Some(v),
            _ => None,
        }
    }
    pub fn all_definitions(&self) -> &[SettingDef] {
        &self.definitions
    }

    pub fn get_category(&self, category: SettingsCategory) -> Vec<(&str, &SettingDef)> {
        self.definitions
            .iter()
            .filter(|d| d.category == category)
            .map(|d| (d.name.as_str(), d))
            .collect()
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn reset(&mut self) {
        {
            let mut settings = self.settings.lock().unwrap_or_else(|e| e.into_inner());
            settings.clear();
            for def in &self.definitions {
                settings.insert(def.name.clone(), def.default.clone());
            }
        }
        self.changed = true;
        // A bulk reset is not a step-by-step change, so the per-setting history
        // no longer describes the state and must not be replayable.
        self.clear_history();
        // Persist, or the next launch restores the old values and the reset
        // appears to have done nothing.
        let _ = self.save();
    }

    /// Register an extra setting.
    ///
    /// Exists so the settings window's tests can exercise each value shape. Not
    /// part of the user-facing API: the shipped set is fixed, and a caller adding
    /// one at runtime would be changing the contract.
    #[doc(hidden)]
    pub fn register_for_test(
        &mut self,
        name: &'static str,
        default: SettingValue,
        min: Option<f64>,
        max: Option<f64>,
    ) {
        // The bounds carry the *numbers*, not a copy of the default: cloning the
        // default produced a range of [default, default] and clamped every
        // change away.
        let (lo, hi) = match &default {
            SettingValue::Int(_) => (
                min.map(|v| SettingValue::Int(v as i64)),
                max.map(|v| SettingValue::Int(v as i64)),
            ),
            SettingValue::Bool(_) | SettingValue::Float(_) => {
                (min.map(SettingValue::Float), max.map(SettingValue::Float))
            }
            // Strings and enums have no numeric range to express.
            _ => (None, None),
        };
        self.definitions.push(SettingDef {
            name: name.to_string(),
            description: format!("test setting {name}"),
            category: SettingsCategory::General,
            default,
            min: lo,
            max: hi,
        });
    }

    /// Every setting key this build knows about.
    pub fn known_keys(&self) -> Vec<String> {
        self.definitions.iter().map(|d| d.name.clone()).collect()
    }

    pub fn export(&self) -> HashMap<String, SettingValue> {
        self.settings.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_new() {
        let s = SettingsUI::in_memory();
        assert!(s.get("fps_target").is_some());
    }

    #[test]
    fn undo_and_redo_reverse_changes() {
        let mut s = SettingsUI::in_memory();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        assert!(s.can_undo(), "there is something to undo");
        assert!(!s.can_redo());

        // Undo should revert to 30.
        s.undo();
        assert_eq!(
            s.get("fps_target").unwrap(),
            SettingValue::Int(30),
            "undo restored the previous value"
        );
        assert!(s.can_redo());

        s.redo();
        assert_eq!(
            s.get("fps_target").unwrap(),
            SettingValue::Int(60),
            "redo reapplied the changed value"
        );
        assert!(!s.can_redo(), "nothing more to redo");
    }

    #[test]
    fn undo_does_not_record_a_no_op() {
        let mut s = SettingsUI::in_memory();
        s.set("fps_target", SettingValue::Int(30)).unwrap(); // same as default
        assert!(!s.can_undo(), "a no-op set must not be undoable");
    }

    #[test]
    fn reset_clears_the_undo_history() {
        let mut s = SettingsUI::in_memory();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        assert!(s.can_undo());
        s.reset();
        assert!(!s.can_undo(), "reset forgets the per-change history");
        assert!(!s.can_redo());
    }

    #[test]
    fn undo_stores_the_before_value_not_the_after() {
        // A new setting, then an undo, should restore the previous value.
        let mut s = SettingsUI::in_memory();
        // The default is 30; change it.
        s.set("volume_master", SettingValue::Float(0.9)).unwrap();
        assert!(s.can_undo());
        s.undo();
        // The value after undo must be the previous one (0.7 default).
        assert_eq!(s.get("volume_master").unwrap(), SettingValue::Float(0.7));
    }

    #[test]
    fn test_settings_get() {
        let s = SettingsUI::in_memory();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 30),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_settings_set() {
        let mut s = SettingsUI::in_memory();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 60),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_settings_validation() {
        let mut s = SettingsUI::in_memory();
        let result = s.set("fps_target", SettingValue::Int(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_unknown() {
        let mut s = SettingsUI::in_memory();
        let result = s.set("nonexistent", SettingValue::Int(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_changed() {
        let mut s = SettingsUI::in_memory();
        assert!(!s.changed());
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        assert!(s.changed());
    }

    #[test]
    fn test_settings_reset() {
        let mut s = SettingsUI::in_memory();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        s.reset();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 30),
            _ => panic!("Expected default"),
        }
    }

    #[test]
    fn test_settings_category() {
        let s = SettingsUI::in_memory();
        let audio = s.get_category(SettingsCategory::Audio);
        assert_eq!(audio.len(), 3);
    }

    #[test]
    fn test_settings_export() {
        let s = SettingsUI::in_memory();
        let exported = s.export();
        assert!(!exported.is_empty());
    }

    #[test]
    fn every_offered_provider_is_one_the_engine_accepts() {
        // The option list and parse_backend drifted apart before: 9Router and
        // llama.cpp were supported but not selectable. Tying them together stops
        // that recurring.
        let store = SettingsUI::in_memory();
        let value = store.get("ai_provider").expect("ai_provider is declared");
        let SettingValue::Enum(_, options) = value else {
            panic!("ai_provider should be an enum, got {value:?}");
        };
        for option in options {
            assert!(
                crate::ai::AiEngine::parse_backend(option.as_str()).is_ok(),
                "'{option}' is offered but the engine rejects it"
            );
        }
    }

    #[test]
    fn the_engine_supports_every_backend_the_offer_names() {
        // And the other direction: a backend with no entry cannot be selected.
        for name in [
            "ollama",
            "openai",
            "anthropic",
            "9router",
            "llamacpp",
            "custom",
        ] {
            assert!(
                crate::ai::AiEngine::parse_backend(name).is_ok(),
                "'{name}' should be selectable"
            );
        }
    }

    #[test]
    fn test_float_validation() {
        let mut s = SettingsUI::in_memory();
        assert!(s.set("volume_master", SettingValue::Float(0.5)).is_ok());
        assert!(s.set("volume_master", SettingValue::Float(1.5)).is_err());
    }
}

/// A read-only view of the settings, shareable across threads.
///
/// The value map is already behind an `Arc<Mutex<_>>`, so this is a cheap clone
/// that can be handed to the IPC layer while the render loop keeps mutating the
/// owner.
#[derive(Clone)]
pub struct SettingsReader {
    settings: Arc<Mutex<HashMap<String, SettingValue>>>,
}

impl SettingsReader {
    /// The current value of a setting, or `None` when unset or unknown.
    pub fn get(&self, name: &str) -> Option<SettingValue> {
        self.settings.lock().ok().and_then(|s| s.get(name).cloned())
    }

    /// Every setting, for a UI that lists them.
    pub fn snapshot(&self) -> HashMap<String, SettingValue> {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Read a setting as a number, whatever its stored representation.
    ///
    /// `SettingValue` is a tagged enum and a "bool" holds a float, so callers that
    /// only want the magnitude should not have to repeat this match.
    pub fn number(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            SettingValue::Bool(v) | SettingValue::Float(v) => Some(v),
            SettingValue::Int(v) => Some(v as f64),
            _ => None,
        }
    }

    /// Read a setting as text, mirroring [SettingsUI::text].
    pub fn text(&self, name: &str) -> Option<String> {
        match self.get(name)? {
            SettingValue::String(v) => Some(v),
            SettingValue::Enum(v, _) => Some(v),
            _ => None,
        }
    }
}
