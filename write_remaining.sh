#!/bin/bash
# Write all ScreenBuddy implementation files

cd /G/Projects/ScreenBuddy

# Remove stray directory
rm -rf screenbuddy-core

# Create directories
mkdir -p crates/screenbuddy-core/src
mkdir -p .github/workflows
mkdir -p .githooks

# === audio.rs ===
cat > crates/screenbuddy-core/src/audio.rs << 'EOF'
//! Audio System for ScreenBuddy
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub creature_volume: f32,
    pub notification_volume: f32,
    pub enabled: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { master_volume: 0.7, creature_volume: 0.8, notification_volume: 0.9, enabled: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoundCategory {
    Idle, Walk, Fly, Sleep, Celebrate, Click, Notification, Error, WakeUp, Sleepiness,
}

pub struct AudioEngine {
    config: AudioConfig,
    sounds: HashMap<SoundCategory, Vec<PathBuf>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self { config: AudioConfig::default(), sounds: HashMap::new() }
    }

    pub fn with_config(config: AudioConfig) -> Self {
        Self { config, sounds: HashMap::new() }
    }

    pub fn register_sound(&mut self, category: SoundCategory, path: PathBuf) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("Sound file not found: {}", path.display()));
        }
        self.sounds.entry(category).or_insert_with(Vec::new).push(path);
        Ok(())
    }

    pub fn register_directory(&mut self, dir: PathBuf) -> Result<usize, String> {
        if dir.exists() {
            let mut count = 0;
            for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if matches!(ext.to_str().unwrap_or(""), "wav" | "mp3" | "ogg" | "flac") {
                            let name = path.file_stem().unwrap_or_default().to_string_lossy();
                            let category = Self::parse_category(&name);
                            self.register_sound(category, path)?;
                            count += 1;
                        }
                    }
                }
            }
            Ok(count)
        } else {
            Ok(0)
        }
    }

    fn parse_category(name: &str) -> SoundCategory {
        match name.to_lowercase().as_str() {
            s if s.contains("idle") => SoundCategory::Idle,
            s if s.contains("walk") => SoundCategory::Walk,
            s if s.contains("fly") => SoundCategory::Fly,
            s if s.contains("sleep") => SoundCategory::Sleep,
            s if s.contains("celebrat") => SoundCategory::Celebrate,
            s if s.contains("click") => SoundCategory::Click,
            s if s.contains("notification") => SoundCategory::Notification,
            s if s.contains("error") => SoundCategory::Error,
            _ => SoundCategory::Idle,
        }
    }

    pub fn play(&self, _category: SoundCategory) -> Result<(), String> {
        Ok(())
    }

    pub fn play_file(&self, _path: &PathBuf, _volume: f32, _looped: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn stop_all(&self) {}

    pub fn set_master_volume(&mut self, volume: f32) {
        self.config.master_volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_creature_volume(&mut self, volume: f32) {
        self.config.creature_volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

impl Default for AudioEngine {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_config_default() {
        let config = AudioConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn test_parse_category() {
        assert_eq!(AudioEngine::parse_category("idle_chirp"), SoundCategory::Idle);
        assert_eq!(AudioEngine::parse_category("sleep_snore"), SoundCategory::Sleep);
    }
}
EOF

echo "Written audio.rs"

# === chat_ui.rs ===
cat > crates/screenbuddy-core/src/chat_ui.rs << 'EOF'
//! Chat UI Overlay for ScreenBuddy
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole { User, Assistant, System }

pub struct ChatHistory {
    messages: Vec<ChatMessage>,
    max_messages: usize,
}

impl ChatHistory {
    pub fn new(max_messages: usize) -> Self {
        Self { messages: Vec::new(), max_messages }
    }

    pub fn add(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            timestamp: chrono::Local::now().to_rfc3339(),
        });
        if self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    pub fn messages(&self) -> &[ChatMessage] { &self.messages }
    pub fn clear(&mut self) { self.messages.clear(); }
    pub fn len(&self) -> usize { self.messages.len() }
    pub fn is_empty(&self) -> bool { self.messages.is_empty() }
}

impl Default for ChatHistory {
    fn default() -> Self { Self::new(100) }
}

pub struct ChatUIState {
    pub history: ChatHistory,
    pub input_text: String,
    pub is_open: bool,
    pub is_dirty: bool,
}

impl ChatUIState {
    pub fn new() -> Self {
        Self { history: ChatHistory::default(), input_text: String::new(), is_open: false, is_dirty: true }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.history.add(role, content);
        self.is_dirty = true;
    }

    pub fn append_input(&mut self, text: &str) {
        self.input_text.push_str(text);
        self.is_dirty = true;
    }

    pub fn backspace(&mut self) {
        self.input_text.pop();
        self.is_dirty = true;
    }

    pub fn clear_input(&mut self) {
        self.input_text.clear();
        self.is_dirty = true;
    }

    pub fn send(&mut self) -> Option<String> {
        if self.input_text.trim().is_empty() { return None; }
        let text = self.input_text.clone();
        self.add_message(MessageRole::User, text.clone());
        self.clear_input();
        Some(text)
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        self.is_dirty = true;
    }
}

impl Default for ChatUIState {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_history() {
        let mut history = ChatHistory::new(5);
        history.add(MessageRole::User, "Hello".to_string());
        history.add(MessageRole::Assistant, "Hi!".to_string());
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_chat_ui_state() {
        let mut state = ChatUIState::new();
        assert!(!state.is_open);
        state.toggle();
        assert!(state.is_open);
        state.append_input("Hello");
        let msg = state.send();
        assert_eq!(msg.unwrap(), "Hello");
    }
}
EOF

echo "Written chat_ui.rs"

# === lib.rs ===
cat > crates/screenbuddy-core/src/lib.rs << 'EOF'
pub mod agent;
pub mod ai;
pub mod audio;
pub mod chat_ui;
pub mod creature;
pub mod error;
pub mod ipc;
pub mod rag;
pub mod state;

pub use error::{Error, Result};
pub use creature::{Creature, CreatureId, load_creature_from_file};
pub use state::{AppState, CreatureState, State};
pub use ai::{AiEngine, AiConfig, AiRequest, AiResponse, ModelTier, Backend, Message};
pub use rag::{RagPipeline, InMemoryVectorStore, TfIdfEmbedding, VectorStore, Chunk};
pub use agent::{AgentRuntime, AgentEvent, Tool};
pub use audio::{AudioEngine, AudioConfig, SoundCategory};
pub use chat_ui::{ChatHistory, ChatUIState, ChatMessage, MessageRole};
pub use ipc::{IpcServer, IpcClient, GodotCommand, Response, DEFAULT_PORT};
EOF

echo "Written lib.rs"
echo "All source files written successfully"
