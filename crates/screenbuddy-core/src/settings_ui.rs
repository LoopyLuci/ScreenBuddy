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

/// A single setting value
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl SettingsUI {
    pub fn new() -> Self {
        let mut s = Self {
            settings: Arc::new(Mutex::new(HashMap::new())),
            definitions: Vec::new(),
            changed: false,
        };
        s.register_defaults();
        s
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
                name: "ai_provider".to_string(),
                description: "AI provider to use".to_string(),
                category: SettingsCategory::AI,
                default: SettingValue::Enum(
                    "ollama".to_string(),
                    vec!["ollama".to_string(), "openai".to_string(), "anthropic".to_string()],
                ),
                min: None,
                max: None,
            },
            SettingDef {
                name: "ai_model".to_string(),
                description: "AI model name".to_string(),
                category: SettingsCategory::AI,
                default: SettingValue::String("llama3".to_string()),
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

    pub fn get(&self, name: &str) -> Option<SettingValue> {
        self.settings.lock().unwrap().get(name).cloned()
    }

    pub fn set(&mut self, name: &str, value: SettingValue) -> Result<(), String> {
        let mut settings = self.settings.lock().unwrap();
        if let Some(def) = self.definitions.iter().find(|d| d.name == name) {
            // Validate range
            match (&value, &def.min, &def.max) {
                (SettingValue::Float(v), Some(SettingValue::Float(min)), Some(SettingValue::Float(max))) => {
                    if *v < *min || *v > *max {
                        return Err(format!("Value {} not in range [{}, {}]", v, min, max));
                    }
                }
                (SettingValue::Int(v), Some(SettingValue::Int(min)), Some(SettingValue::Int(max))) => {
                    if *v < *min || *v > *max {
                        return Err(format!("Value {} not in range [{}, {}]", v, min, max));
                    }
                }
                _ => {}
            }
            settings.insert(name.to_string(), value);
            self.changed = true;
            Ok(())
        } else {
            Err(format!("Unknown setting: {}", name))
        }
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
        let mut settings = self.settings.lock().unwrap();
        settings.clear();
        for def in &self.definitions {
            settings.insert(def.name.clone(), def.default.clone());
        }
        self.changed = false;
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
        let s = SettingsUI::new();
        assert!(s.get("fps_target").is_some());
    }

    #[test]
    fn test_settings_get() {
        let s = SettingsUI::new();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 30),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_settings_set() {
        let mut s = SettingsUI::new();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 60),
            _ => panic!("Expected Int"),
        }
    }

    #[test]
    fn test_settings_validation() {
        let mut s = SettingsUI::new();
        let result = s.set("fps_target", SettingValue::Int(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_unknown() {
        let mut s = SettingsUI::new();
        let result = s.set("nonexistent", SettingValue::Int(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_changed() {
        let mut s = SettingsUI::new();
        assert!(!s.changed());
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        assert!(s.changed());
    }

    #[test]
    fn test_settings_reset() {
        let mut s = SettingsUI::new();
        s.set("fps_target", SettingValue::Int(60)).unwrap();
        s.reset();
        match s.get("fps_target").unwrap() {
            SettingValue::Int(v) => assert_eq!(v, 30),
            _ => panic!("Expected default"),
        }
    }

    #[test]
    fn test_settings_category() {
        let s = SettingsUI::new();
        let audio = s.get_category(SettingsCategory::Audio);
        assert_eq!(audio.len(), 3);
    }

    #[test]
    fn test_settings_export() {
        let s = SettingsUI::new();
        let exported = s.export();
        assert!(!exported.is_empty());
    }

    #[test]
    fn test_float_validation() {
        let mut s = SettingsUI::new();
        assert!(s.set("volume_master", SettingValue::Float(0.5)).is_ok());
        assert!(s.set("volume_master", SettingValue::Float(1.5)).is_err());
    }
}
