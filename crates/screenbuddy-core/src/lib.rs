pub mod agent;
pub mod ai;
pub mod ai_bridge;
pub mod animation;
pub mod audio;
pub mod chat_overlay;
pub mod chat_ui;
pub mod config;
pub mod creature;
pub mod error;
pub mod hardware;
pub mod ipc;
pub mod per_pet_window;
pub mod platform;
pub mod providers;
pub mod rag;
pub mod render;
pub mod scheduler;
pub mod screen_physics;
pub mod settings_ui;
pub mod state;
pub mod system_integration;
pub mod system_tray;
pub mod tts;

pub use agent::{default_tools, AgentConfig, AgentEvent, AgentRuntime, Tool};
pub use ai::{AiConfig, AiEngine, AiRequest, AiResponse, Message, MessageRole};
pub use ai_bridge::AiChatBridge;
pub use audio::{AudioConfig, AudioEvent, AudioSystem, SoundCategory};
pub use chat_overlay::{ChatMessage, ChatOverlay, ChatOverlayEvent, MessageRole as ChatMsgRole};
pub use config::{ConfigManager, ScreenBuddyConfig, Theme};
pub use creature::{load_creature_from_file, Creature};
pub use error::{Error, Result};
pub use hardware::{hardware_profile, CpuInfo, GpuInfo, HardwareProfile};
pub use ipc::{
    AnimStateCtl, ControlRequest, GodotCommand, IpcClient, IpcConfig, IpcServer, Response,
    RuntimeStatus, DEFAULT_PORT, DEFAULT_SEARCH_LIMIT, MAX_FRAME_BYTES, MAX_SEARCH_LIMIT,
};
pub use per_pet_window::{PerPetWindowManager, PetWindowConfig, PetWindowEvent, PetWindowId};
pub use rag::RagPipeline;
pub use render::renderer::Renderer;
pub use scheduler::{TaskPriority, ThreadPool};
pub use screen_physics::{CreaturePersonality, CreaturePhysics, Monitor, ScreenManager};
pub use settings_ui::{SettingValue, SettingsCategory, SettingsUI};
pub use state::{AppConfig, AppState, CreatureStateMachine, State};
pub use system_integration::{SystemEvent, SystemEventType, SystemIntegration, SystemMonitor};
pub use system_tray::{SystemTray, TrayEvent, TrayHwnd};
pub use tts::{TtsConfig, TtsEngine, TtsSystem};
