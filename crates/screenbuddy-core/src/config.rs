//! Configuration Management for ScreenBuddy
use crate::ai::AiConfig;
use crate::audio::AudioConfig;
use crate::providers::ProviderRegistry;
use crate::state::AppConfig;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    Auto,
}

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

impl Default for ScreenBuddyConfig {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            active_creature: "companion-bird-01".to_string(),
            window_position: None,
            window_size: None,
            ai: AiConfig::default(),
            audio: AudioConfig::default(),
            app: crate::state::AppConfig::default(),
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
    path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let path = Self::get_config_path();
        let config = if path.exists() {
            Self::load_from(&path).unwrap_or_default()
        } else {
            ScreenBuddyConfig::default()
        };
        Self {
            config: RwLock::new(config),
            path,
        }
    }

    fn get_config_path() -> PathBuf {
        // Use proper app data directory
        if let Some(dirs) = directories::ProjectDirs::from("com", "screenbuddy", "ScreenBuddy") {
            let mut p = dirs.config_dir().to_path_buf();
            std::fs::create_dir_all(&p).ok();
            p.push("screenbuddy.toml");
            p
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join("screenbuddy.toml")
        }
    }

    fn load_from(path: &PathBuf) -> Result<ScreenBuddyConfig, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        let content = toml::to_string_pretty(&*self.config.read()).map_err(|e| e.to_string())?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&self.path, content).map_err(|e| e.to_string())
    }

    pub fn get(&self) -> ScreenBuddyConfig {
        self.config.read().clone()
    }
    pub fn set_creature(&self, id: String) {
        self.config.write().active_creature = id;
    }
    pub fn set_position(&self, x: i32, y: i32) {
        self.config.write().window_position = Some((x, y));
    }
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ScreenBuddyConfig::default();
        assert_eq!(config.active_creature, "companion-bird-01");
        assert!(config.audio.enabled);
    }

    #[test]
    fn test_config_manager() {
        let manager = ConfigManager::new();
        assert_eq!(manager.get().active_creature, "companion-bird-01");
    }

    #[test]
    fn test_set_creature() {
        let manager = ConfigManager::new();
        manager.set_creature("robo-cat".to_string());
        assert_eq!(manager.get().active_creature, "robo-cat");
    }
}
