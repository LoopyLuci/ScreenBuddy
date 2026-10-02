#!/usr/bin/env python3
"""Write all ScreenBuddy implementation files at once."""
import os

W = r"G:\Projects\ScreenBuddy\screenbuddy-core\src"

# === error.rs ===
with open(os.path.join(W, "error.rs"), "w") as f:
    f.write('''use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Creature not found: {0}")]
    CreatureNotFound(String),
    #[error("Invalid definition: {0}")]
    InvalidDefinition(String),
    #[error("Validation failed: {0:?}")]
    Validation(Vec<String>),
    #[error("Render error: {0}")]
    Render(String),
    #[error("Platform error: {0}")]
    Platform(String),
    #[error("Window error: {0}")]
    Window(String),
    #[error("Animation error: {0}")]
    Animation(String),
    #[error("State error: {0}")]
    State(String),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Other: {0}")]
    Other(String),
}
''')

# === ai.rs (with force_tier and history fields) ===
with open(os.path.join(W, "ai.rs"), "w") as f:
    f.write('''//! AI Engine for ScreenBuddy
use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ModelTier { LocalTiny, LocalMedium, LocalLarge, CloudBudget, CloudPremium, CloudMax }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Backend { Candle, LlamaCpp, OpenAi, Anthropic, Custom }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub force_tier: Option<ModelTier>,
    pub history: Option<Vec<Message>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message { pub role: MessageRole, pub content: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole { System, User, Assistant }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub text: String,
    pub model_tier: ModelTier,
    pub backend: Backend,
    pub tokens_used: u32,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason { Stop, Length, Error(String) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub custom_endpoint: Option<String>,
    pub custom_api_key: Option<String>,
    pub local_model_path: Option<PathBuf>,
    pub local_model_type: LocalModelType,
    pub default_tier: ModelTier,
    pub max_tokens_default: u32,
    pub offline_only: bool,
    pub cloud_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalModelType { Candle, LlamaCpp }

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openai_api_key: None, anthropic_api_key: None,
            custom_endpoint: None, custom_api_key: None,
            local_model_path: None, local_model_type: LocalModelType::LlamaCpp,
            default_tier: ModelTier::LocalTiny, max_tokens_default: 2048,
            offline_only: true, cloud_fallback: false,
        }
    }
}

#[derive(Debug, Clone)]
struct ModelInfo {
    tier: ModelTier, backend: Backend, name: String,
    max_context: u32, cost_per_1k_tokens: f64,
}

pub struct AiEngine {
    config: AiConfig,
    available_models: Vec<ModelInfo>,
    client: reqwest::Client,
}

impl AiEngine {
    pub fn new(config: AiConfig) -> Self {
        let available_models = Self::build_model_list(&config);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");
        Self { config, available_models, client }
    }

    pub async fn generate(&self, req: AiRequest) -> Result<AiResponse> {
        let tier = req.force_tier.unwrap_or_else(|| self.auto_select_tier(&req));
        let model = self.select_model(tier)?;
        match model.backend {
            Backend::Candle => self.generate_candle(&req, model).await,
            Backend::LlamaCpp => self.generate_llamacpp(&req, model).await,
            Backend::OpenAi => self.generate_openai(&req, model).await,
            Backend::Anthropic => self.generate_anthropic(&req, model).await,
            Backend::Custom => self.generate_custom(&req, model).await,
        }
    }

    fn auto_select_tier(&self, req: &AiRequest) -> ModelTier {
        let complexity = self.estimate_complexity(&req.prompt);
        if self.config.offline_only {
            if complexity < 0.3 { ModelTier::LocalTiny }
            else if complexity < 0.7 { ModelTier::LocalMedium }
            else { ModelTier::LocalLarge }
        } else {
            if complexity < 0.2 && self.has_local_model() { ModelTier::LocalTiny }
            else if complexity < 0.5 && self.has_local_model() { ModelTier::LocalMedium }
            else if complexity < 0.8 { ModelTier::CloudBudget }
            else { ModelTier::CloudPremium }
        }
    }

    fn estimate_complexity(&self, prompt: &str) -> f64 {
        let len = prompt.len() as f64;
        let base = (len / 1000.0).min(1.0);
        let code_kw = ["function", "class", "implement", "debug", "algorithm", "code"];
        if code_kw.iter().any(|k| prompt.to_lowercase().contains(k)) {
            (base + 0.3).min(1.0)
        } else { base }
    }

    fn has_local_model(&self) -> bool {
        self.available_models.iter().any(|m| matches!(m.backend, Backend::Candle | Backend::LlamaCpp))
    }

    fn select_model(&self, tier: ModelTier) -> Result<&ModelInfo> {
        self.available_models.iter().find(|m| m.tier == tier)
            .or_else(|| self.available_models.first())
            .ok_or_else(|| crate::error::Error::InvalidConfig("No models available".to_string()))
    }

    fn build_model_list(config: &AiConfig) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        if config.local_model_path.is_some() {
            match config.local_model_type {
                LocalModelType::LlamaCpp => models.push(ModelInfo {
                    tier: config.default_tier, backend: Backend::LlamaCpp,
                    name: "local-llama".into(), max_context: 4096, cost_per_1k_tokens: 0.0,
                }),
                LocalModelType::Candle => models.push(ModelInfo {
                    tier: config.default_tier, backend: Backend::Candle,
                    name: "local-candle".into(), max_context: 2048, cost_per_1k_tokens: 0.0,
                }),
            }
        }
        if !config.offline_only {
            if config.openai_api_key.is_some() {
                models.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::OpenAi, name: "gpt-3.5-turbo".into(), max_context: 16384, cost_per_1k_tokens: 0.0005 });
                models.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::OpenAi, name: "gpt-4o".into(), max_context: 128000, cost_per_1k_tokens: 0.005 });
            }
            if config.anthropic_api_key.is_some() {
                models.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::Anthropic, name: "claude-3-haiku-20240307".into(), max_context: 200000, cost_per_1k_tokens: 0.00025 });
                models.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::Anthropic, name: "claude-3-sonnet-20240229".into(), max_context: 200000, cost_per_1k_tokens: 0.003 });
            }
        }
        models
    }

    fn build_messages(&self, req: &AiRequest) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();
        if let Some(system) = &req.system_prompt {
            messages.push(serde_json::json!({ "role": "system", "content": system }));
        }
        if let Some(history) = &req.history {
            for msg in history.iter().take(20) {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                };
                messages.push(serde_json::json!({ "role": role, "content": &msg.content }));
            }
        }
        messages.push(serde_json::json!({ "role": "user", "content": &req.prompt }));
        messages
    }

    async fn generate_candle(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        Ok(AiResponse {
            text: format!("[Candle/{}] {}", model.name, self.simple_response(&req.prompt)),
            model_tier: model.tier, backend: model.backend,
            tokens_used: (req.prompt.len() / 4) as u32,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn generate_llamacpp(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        Ok(AiResponse {
            text: format!("[llama.cpp/{}] {}", model.name, self.simple_response(&req.prompt)),
            model_tier: model.tier, backend: model.backend,
            tokens_used: (req.prompt.len() / 4) as u32,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn generate_openai(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let api_key = self.config.openai_api_key.as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("OpenAI API key not set".to_string()))?;
        let messages = self.build_messages(req);
        let body = serde_json::json!({
            "model": model.name, "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
        });
        let response = self.client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = response.json().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            text: json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string(),
            model_tier: model.tier, backend: model.backend,
            tokens_used: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn generate_anthropic(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let api_key = self.config.anthropic_api_key.as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("Anthropic API key not set".to_string()))?;
        let messages = self.build_messages(req);
        let system = req.system_prompt.clone().unwrap_or_default();
        let body = serde_json::json!({
            "model": model.name, "messages": messages, "system": system,
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
        });
        let response = self.client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = response.json().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            text: json["content"][0]["text"].as_str().unwrap_or("").to_string(),
            model_tier: model.tier, backend: model.backend,
            tokens_used: json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                + json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            finish_reason: FinishReason::Stop,
        })
    }

    async fn generate_custom(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let endpoint = self.config.custom_endpoint.as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("Custom endpoint not set".to_string()))?;
        let messages = self.build_messages(req);
        let body = serde_json::json!({ "messages": messages });
        let mut request = self.client.post(endpoint).json(&body);
        if let Some(key) = &self.config.custom_api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }
        let response = request.send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = response.json().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            text: json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string(),
            model_tier: model.tier, backend: model.backend,
            tokens_used: 0, finish_reason: FinishReason::Stop,
        })
    }

    fn simple_response(&self, prompt: &str) -> String {
        let p = prompt.to_lowercase();
        if p.contains("hello") || p.contains("hi") {
            "Hello! I'm your ScreenBuddy companion. How can I help you today?".to_string()
        } else if p.contains("help") {
            "I can help you with:\n- Taking notes and reminders\n- Searching the web\n- Answering questions\n- Controlling your desktop buddy\nJust ask!".to_string()
        } else if p.contains("joke") {
            "Why do programmers prefer dark mode? Because light attracts bugs! 🐛".to_string()
        } else {
            format!("I heard: '{}'. I'm a demo response - connect a real AI model for full functionality!", prompt)
        }
    }
}

impl Clone for AiEngine {
    fn clone(&self) -> Self {
        Self { config: self.config.clone(), available_models: self.available_models.clone(), client: self.client.clone() }
    }
}
''')

# === agent.rs (fixed AiRequest fields) ===
with open(os.path.join(W, "agent.rs"), "w") as f:
    f.write('''//! Agentic Runtime for ScreenBuddy
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};

use crate::ai::{AiEngine, AiRequest, Message, MessageRole};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    Thinking(String),
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { name: String, output: String },
    Message(String),
    Done,
    Error(String),
}

pub type ToolHandler = Arc<dyn Fn(serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>> + Send + Sync>;

pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: ToolHandler,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_iterations: usize,
    pub timeout_secs: u64,
    pub system_prompt: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self { max_iterations: 10, timeout_secs: 120, system_prompt: "You are ScreenBuddy, a helpful desktop AI companion.".to_string() }
    }
}

pub struct AgentRuntime {
    ai: Arc<AiEngine>,
    tools: Arc<Mutex<HashMap<String, Tool>>>,
    config: AgentConfig,
    tx: broadcast::Sender<AgentEvent>,
}

impl AgentRuntime {
    pub fn new(ai: Arc<AiEngine>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { ai, tools: Arc::new(Mutex::new(HashMap::new())), config: AgentConfig::default(), tx }
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self { self.config = config; self }

    pub fn register_tool(&self, name: &str, tool: Tool) {
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let mut tools = self.tools.blocking_lock();
            tools.insert(name.to_string(), tool);
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> { self.tx.subscribe() }

    pub async fn run(&self, user_input: String) -> Result<String> {
        let _ = self.tx.send(AgentEvent::Thinking("Starting...".to_string()));
        let mut response = String::new();
        let mut history = Vec::new();

        for _ in 0..self.config.max_iterations {
            let _ = self.tx.send(AgentEvent::Thinking("Thinking...".to_string()));

            let req = AiRequest {
                prompt: user_input.clone(),
                system_prompt: Some(self.config.system_prompt.clone()),
                max_tokens: Some(1024),
                temperature: Some(0.7),
                stream: false,
                force_tier: None,
                history: if history.is_empty() { None } else { Some(history.clone()) },
            };

            match tokio::time::timeout(Duration::from_secs(self.config.timeout_secs), self.ai.generate(req)).await {
                Ok(Ok(ai_resp)) => {
                    if ai_resp.text.contains("TOOL:") {
                        let tool_text = ai_resp.text.split("TOOL:").nth(1).unwrap_or("").trim();
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(tool_text) {
                            if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                                if let Some(args) = val.get("arguments") {
                                    let _ = self.tx.send(AgentEvent::ToolUse { name: name.to_string(), input: args.clone() });
                                    let output = self.execute_tool(name, args.clone()).await;
                                    let _ = self.tx.send(AgentEvent::ToolResult { name: name.to_string(), output: output.clone() });
                                    history.push(Message { role: MessageRole::User, content: user_input.clone() });
                                    history.push(Message { role: MessageRole::Assistant, content: output });
                                    continue;
                                }
                            }
                        }
                    }
                    response = ai_resp.text.clone();
                    let _ = self.tx.send(AgentEvent::Message(response.clone()));
                    let _ = self.tx.send(AgentEvent::Done);
                    break;
                }
                Ok(Err(e)) => { let _ = self.tx.send(AgentEvent::Error(e.to_string())); break; }
                Err(_) => { let _ = self.tx.send(AgentEvent::Error("Timeout".to_string())); break; }
            }
        }

        Ok(response)
    }

    async fn execute_tool(&self, name: &str, input: serde_json::Value) -> String {
        let tools = self.tools.read().await;
        if let Some(tool) = tools.get(name) {
            match (tool.handler)(input).await {
                Ok(output) => output,
                Err(e) => format!("Error: {}", e),
            }
        } else {
            format!("Unknown tool: {}", name)
        }
    }

    pub fn tool_count(&self) -> usize {
        match tokio::runtime::Handle::try_current() {
            Ok(_) => self.tools.blocking_lock().len(),
            Err(_) => 0,
        }
    }
}

pub fn default_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "echo".to_string(),
            description: "Echo back the input".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
            handler: Arc::new(|input| Box::pin(async move {
                Ok(input.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string())
            })),
        },
        Tool {
            name: "time".to_string(),
            description: "Get current time".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            handler: Arc::new(|_| Box::pin(async move {
                Ok(chrono::Local::now().to_rfc3339())
            })),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiConfig;

    #[test]
    fn test_agent() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);
        assert_eq!(agent.tool_count(), 0);
    }

    #[test]
    fn test_default_tools() {
        let tools = default_tools();
        assert_eq!(tools.len(), 2);
    }
}
''')

# === ipc.rs (simplified, no anyhow, uses our Error type) ===
with open(os.path.join(W, "ipc.rs"), "w") as f:
    f.write('''//! IPC Bridge for ScreenBuddy
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, Notify};

pub const DEFAULT_PORT: u16 = 34567;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GodotCommand {
    Ping,
    ListCreatures,
    GetCreature { id: String },
    SetCreature { id: String, data: serde_json::Value },
    ReloadCreature { id: String },
    ExportCreature { data: serde_json::Value },
    SaveConfig { config: serde_json::Value },
    LoadConfig,
    Unknown { raw: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Success { data: Option<serde_json::Value> },
    Error { message: String, code: Option<u32> },
    Pong,
    CreatureList { creatures: Vec<String> },
    CreatureData { id: String, data: serde_json::Value },
    Ack,
}

#[derive(Debug, Clone)]
pub struct IpcConfig {
    pub addr: String,
    pub enable_hot_reload: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self { addr: format!("127.0.0.1:{}", DEFAULT_PORT), enable_hot_reload: true }
    }
}

pub struct IpcServer {
    config: IpcConfig,
    shutdown: Arc<Notify>,
    tx: broadcast::Sender<GodotCommand>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl IpcServer {
    pub fn new(config: IpcConfig) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { config, shutdown: Arc::new(Notify::new()), tx, creatures: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub async fn run(&self) -> Result<(), String> {
        let listener = TcpListener::bind(&self.config.addr).await.map_err(|e| format!("Bind failed: {}", e))?;
        tracing::info!("IPC server listening on {}", self.config.addr);
        loop {
            tokio::select! {
                Ok((stream, _)) = listener.accept() => {
                    let tx = self.tx.clone();
                    let creatures = self.creatures.clone();
                    let shutdown = self.shutdown.clone();
                    tokio::spawn(async move { handle_client(stream, tx, creatures, shutdown).await; });
                }
                _ = self.shutdown.notified() => break,
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) { self.shutdown.notify_waiters(); }
    pub fn subscribe(&self) -> broadcast::Receiver<GodotCommand> { self.tx.subscribe() }
    pub async fn load_creature(&self, id: String, data: serde_json::Value) {
        self.creatures.lock().await.insert(id, data);
    }
}

async fn handle_client(
    mut stream: TcpStream,
    tx: broadcast::Sender<GodotCommand>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    shutdown: Arc<Notify>,
) {
    let mut buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            Ok(n) = stream.read(&mut buf) => {
                if n == 0 { break; }
                if n < 4 { continue; }
                let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                if n < 4 + len { continue; }
                match serde_json::from_slice::<GodotCommand>(&buf[4..4+len]) {
                    Ok(cmd) => {
                        let resp = process_command(&cmd, &creatures).await;
                        let resp_bytes = serde_json::to_vec(&resp).unwrap_or_default();
                        let header = (resp_bytes.len() as u32).to_be_bytes();
                        let _ = stream.write_all(&header).await;
                        let _ = stream.write_all(&resp_bytes).await;
                        let _ = tx.send(cmd);
                    }
                    Err(e) => {
                        let resp = Response::Error { message: format!("Parse error: {}", e), code: Some(400) };
                        let resp_bytes = serde_json::to_vec(&resp).unwrap_or_default();
                        let header = (resp_bytes.len() as u32).to_be_bytes();
                        let _ = stream.write_all(&header).await;
                        let _ = stream.write_all(&resp_bytes).await;
                    }
                }
            }
            _ = shutdown.notified() => break,
        }
    }
}

async fn process_command(
    cmd: &GodotCommand,
    creatures: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
) -> Response {
    match cmd {
        GodotCommand::Ping => Response::Pong,
        GodotCommand::ListCreatures => {
            let list = creatures.lock().await.keys().cloned().collect();
            Response::CreatureList { creatures: list }
        }
        GodotCommand::GetCreature { id } => {
            match creatures.lock().await.get(id) {
                Some(data) => Response::CreatureData { id: id.clone(), data: data.clone() },
                None => Response::Error { message: format!("Creature '{}' not found", id), code: Some(404) },
            }
        }
        GodotCommand::SetCreature { id, data } => {
            creatures.lock().await.insert(id.clone(), data.clone());
            Response::Ack
        }
        GodotCommand::ReloadCreature { id: _ } => Response::Ack,
        GodotCommand::ExportCreature { data } => {
            let id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            creatures.lock().await.insert(id.to_string(), data.clone());
            Response::Success { data: Some(serde_json::json!({"exported": id})) }
        }
        GodotCommand::SaveConfig { config: _ } => Response::Ack,
        GodotCommand::LoadConfig => Response::Success { data: None },
        GodotCommand::Unknown { raw } => {
            Response::Error { message: format!("Unknown command: {}", raw), code: Some(400) }
        }
    }
}

pub struct IpcClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl IpcClient {
    pub async fn connect(addr: &str) -> Result<Self, String> {
        let stream = TcpStream::connect(addr).await.map_err(|e| format!("Connect failed: {}", e))?;
        Ok(Self { stream, buf: vec![0u8; 8192] })
    }

    pub async fn send_command(&mut self, cmd: &GodotCommand) -> Result<Response, String> {
        let cmd_bytes = serde_json::to_vec(cmd).map_err(|e| e.to_string())?;
        let header = (cmd_bytes.len() as u32).to_be_bytes();
        self.stream.write_all(&header).await.map_err(|e| e.to_string())?;
        self.stream.write_all(&cmd_bytes).await.map_err(|e| e.to_string())?;
        let n = self.stream.read(&mut self.buf).await.map_err(|e| e.to_string())?;
        if n < 4 { return Err("Short read".to_string()); }
        let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if n < 4 + len { return Err("Incomplete response".to_string()); }
        serde_json::from_slice(&self.buf[4..4+len]).map_err(|e| e.to_string())
    }

    pub async fn ping(&mut self) -> Result<(), String> {
        match self.send_command(&GodotCommand::Ping).await? {
            Response::Pong => Ok(()),
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn list_creatures(&mut self) -> Result<Vec<String>, String> {
        match self.send_command(&GodotCommand::ListCreatures).await? {
            Response::CreatureList { creatures } => Ok(creatures),
            Response::Error { message, .. } => Err(message),
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn disconnect(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

pub async fn start_server() -> Result<(), String> {
    let config = IpcConfig::default();
    let server = IpcServer::new(config);
    server.run().await
}
''')

# === rag.rs (fixed embedding call) ===
with open(os.path.join(W, "rag.rs"), "w") as f:
    f.write('''//! RAG Pipeline for ScreenBuddy
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String, pub text: String, pub source: String, pub embedding: Option<Vec<f32>>,
}

pub trait VectorStore: Send + Sync {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String>;
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
}

pub struct InMemoryVectorStore { chunks: Vec<Chunk> }
impl InMemoryVectorStore { pub fn new() -> Self { Self { chunks: Vec::new() } } }
impl VectorStore for InMemoryVectorStore {
    fn insert(&mut self, chunk: Chunk) -> Result<(), String> { self.chunks.push(chunk); Ok(()) }
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(f32, Chunk)> {
        let mut r: Vec<(f32, Chunk)> = self.chunks.iter()
            .filter_map(|c| c.embedding.as_ref().map(|e| (cosine(query, e), c.clone())))
            .collect();
        r.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        r.truncate(top_k);
        r
    }
    fn len(&self) -> usize { self.chunks.len() }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() { return 0.0; }
    let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { d / (na * nb) }
}

pub struct TfIdfEmbedding;
impl TfIdfEmbedding {
    pub fn embed(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; 256];
        let tokens: Vec<&str> = text.split_whitespace().collect();
        if tokens.is_empty() { return v; }
        for token in &tokens { let h = simple_hash(token) % 256; v[h] += 1.0; }
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 { for x in &mut v { *x /= n; } }
        v
    }
}

fn simple_hash(s: &str) -> usize {
    let mut h = 5381usize;
    for c in s.bytes() { h = ((h << 5).wrapping_add(h)).wrapping_add(c as usize); }
    h
}

pub struct RagPipeline {
    store: Box<dyn VectorStore>,
    chunk_size: usize,
    chunk_overlap: usize,
}

impl RagPipeline {
    pub fn new() -> Self {
        Self { store: Box::new(InMemoryVectorStore::new()), chunk_size: 512, chunk_overlap: 64 }
    }

    pub fn ingest_document(&mut self, path: &Path, content: &str) -> Result<usize, String> {
        if content.is_empty() { return Ok(0); }
        let chunks = self.chunk_text(content);
        let count = chunks.len();
        for (i, chunk_text) in chunks.into_iter().enumerate() {
            let embedding = TfIdfEmbedding::embed(&chunk_text);
            let chunk = Chunk {
                id: format!("{}-{}", path.display(), i),
                text: chunk_text,
                source: path.to_string_lossy().to_string(),
                embedding: Some(embedding),
            };
            self.store.insert(chunk)?;
        }
        Ok(count)
    }

    pub fn retrieve(&self, query: &str, top_k: usize) -> Vec<Chunk> {
        let query_embedding = TfIdfEmbedding::embed(query);
        self.store.search(&query_embedding, top_k).into_iter().map(|(_, chunk)| chunk).collect()
    }

    pub fn build_context(&self, query: &str, top_k: usize) -> String {
        let chunks = self.retrieve(query, top_k);
        chunks.iter().map(|c| c.text.clone()).collect::<Vec<_>>().join("\n\n")
    }

    pub fn len(&self) -> usize { self.store.len() }

    fn chunk_text(&self, text: &str) -> Vec<String> {
        if text.len() <= self.chunk_size { return vec![text.to_string()]; }
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let end = (start + self.chunk_size).min(text.len());
            chunks.push(text[start..end].to_string());
            if end >= text.len() { break; }
            start = end;
            if start > self.chunk_overlap { start -= self.chunk_overlap; }
        }
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine(&a, &b) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rag_ingest_and_retrieve() {
        let mut rag = RagPipeline::new();
        let count = rag.ingest_document(Path::new("test.txt"), "Rust is a programming language. It is fast.").unwrap();
        assert!(count > 0);
        let results = rag.retrieve("programming language", 2);
        assert!(!results.is_empty());
    }
}
''')

# === creature.rs (add AutoTransition, after_seconds, fix CreatureStateDef) ===
with open(os.path.join(W, "creature.rs"), "w") as f:
    f.write('''//! Creature definitions and validation for ScreenBuddy
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

pub type CreatureId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub name: String,
    pub category: String,
    pub rarity: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,
    pub visual: Visual,
    pub audio: Option<Audio>,
    pub behavior: Behavior,
    pub ai: Option<Ai>,
    pub permissions: Option<Permissions>,
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Visual {
    #[serde(rename = "animation_format")]
    pub animation_format: AnimationFormat,
    pub texture: String,
    pub atlas: Option<String>,
    pub skeleton: Option<String>,
    #[serde(default = "default_scale")]
    pub default_scale: f32,
    pub idle_animation: String,
    pub walk_animation: Option<String>,
    pub fly_animation: Option<String>,
    pub action_animations: Option<HashMap<String, String>>,
    pub animation_fps: Option<u32>,
    pub sprite_size: Option<[u32; 2]>,
    pub frames_per_animation: Option<HashMap<String, u32>>,
    pub particle_effects: Option<ParticleEffects>,
    pub color_palette: Option<Vec<String>>,
}

fn default_scale() -> f32 { 1.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationFormat { SpriteSheet, Spine, Live2d, Frame }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audio {
    pub idle_sound: Option<String>,
    pub click_sound: Option<String>,
    pub notification_sound: Option<String>,
    pub celebration_sound: Option<String>,
    pub error_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Behavior {
    pub script: Option<String>,
    pub default_state: String,
    #[serde(default)]
    pub autonomous_actions: bool,
    pub interaction_radius: Option<f32>,
    pub movement_speed: Option<f32>,
    pub idle_timeout: Option<f32>,
    pub personality: Option<HashMap<String, f32>>,
    pub states: Option<HashMap<String, CreatureStateDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTransition {
    pub after_seconds: Option<f32>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureStateDef {
    pub description: Option<String>,
    pub animation: Option<String>,
    pub loop_animation: Option<bool>,
    pub movement: Option<String>,
    pub speed_range: Option<Vec<f32>>,
    pub can_transition_to: Option<Vec<String>>,
    pub auto_transition: Option<AutoTransition>,
}

impl Default for CreatureStateDef {
    fn default() -> Self {
        Self {
            description: None, animation: None, loop_animation: Some(true),
            movement: None, speed_range: None, can_transition_to: None, auto_transition: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleEffects {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ai {
    pub system_prompt: Option<String>,
    pub voice_id: Option<String>,
    pub specializations: Option<Vec<String>>,
    pub response_style: Option<String>,
    pub max_response_length: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub can_read_screen: bool,
    #[serde(default)]
    pub can_read_clipboard: bool,
    #[serde(default)]
    pub can_send_notifications: bool,
    #[serde(default)]
    pub can_access_files: bool,
    #[serde(default)]
    pub can_execute_commands: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub schema_version: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

fn validate_creature(creature: &Creature) -> Result<()> {
    let mut errors = Vec::new();
    if creature.id.is_empty() { errors.push("id cannot be empty".to_string()); }
    if creature.name.is_empty() { errors.push("name cannot be empty".to_string()); }
    if creature.behavior.default_state.is_empty() { errors.push("behavior.default_state cannot be empty".to_string()); }
    if creature.visual.default_scale <= 0.0 { errors.push("visual.default_scale must be > 0".to_string()); }
    if let Some(personality) = &creature.behavior.personality {
        for (trait_name, value) in personality {
            if *value < 0.0 || *value > 1.0 {
                errors.push(format!("personality.{} must be between 0.0 and 1.0, got {}", trait_name, value));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(Error::Validation(errors)) }
}

pub fn load_creature_from_file(path: &Path) -> Result<Creature> {
    let content = std::fs::read_to_string(path)?;
    let creature: Creature = serde_json::from_str(&content)?;
    validate_creature(&creature)?;
    Ok(creature)
}

pub fn load_creatures_from_dir(dir: &Path) -> Result<Vec<Creature>> {
    let mut creatures = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match load_creature_from_file(&path) {
                    Ok(creature) => creatures.push(creature),
                    Err(e) => tracing::warn!("Failed to load creature from {}: {}", path.display(), e),
                }
            }
        }
    }
    Ok(creatures)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_creature() -> Creature {
        Creature {
            id: "test-01".to_string(), name: "Test".to_string(), category: "test".to_string(),
            rarity: None, description: None, version: None, author: None, tags: None,
            visual: Visual {
                animation_format: AnimationFormat::SpriteSheet, texture: "test.png".to_string(),
                atlas: None, skeleton: None, default_scale: 1.0, idle_animation: "idle".to_string(),
                walk_animation: None, fly_animation: None, action_animations: None,
                animation_fps: None, sprite_size: None, frames_per_animation: None,
                particle_effects: None, color_palette: None,
            },
            audio: None,
            behavior: Behavior {
                script: None, default_state: "idle".to_string(), autonomous_actions: false,
                interaction_radius: None, movement_speed: None, idle_timeout: None,
                personality: None, states: None,
            },
            ai: None, permissions: None, metadata: None,
        }
    }

    #[test]
    fn test_validate_creature_minimal() {
        let creature = test_creature();
        assert!(validate_creature(&creature).is_ok());
    }

    #[test]
    fn test_validate_creature_invalid_personality() {
        let mut personality = HashMap::new();
        personality.insert("curiosity".to_string(), 1.5);
        let creature = Creature {
            behavior: Behavior {
                script: None, default_state: "idle".to_string(), autonomous_actions: false,
                interaction_radius: None, movement_speed: None, idle_timeout: None,
                personality: Some(personality), states: None,
            },
            ..test_creature()
        };
        let result = validate_creature(&creature);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_companion_bird() {
        let path = Path::new("../../assets/generated/companion-bird-01.json");
        if path.exists() {
            let creature = load_creature_from_file(path).unwrap();
            assert_eq!(creature.id, "companion-bird-01");
        }
    }
}
''')

# === lib.rs (updated exports) ===
with open(os.path.join(W, "lib.rs"), "w") as f:
    f.write('''pub mod agent;
pub mod ai;
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
pub use ipc::{IpcServer, IpcClient, GodotCommand, Response, DEFAULT_PORT};
''')

# === CI/CD pipeline ===
ci_dir = r"G:\Projects\ScreenBuddy\.github\workflows"
os.makedirs(ci_dir, exist_ok=True)
with open(os.path.join(ci_dir, "ci.yml"), "w") as f:
    f.write('''name: ScreenBuddy CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 6 * * 1'  # Weekly build check

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    name: Check
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Run clippy
        run: cargo clippy --all-targets -- -D warnings

  test:
    name: Test
    runs-on: windows-latest
    needs: check
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Run tests
        run: cargo test --workspace --verbose

  build:
    name: Build
    runs-on: ${{ matrix.os }}
    needs: test
    strategy:
      matrix:
        os: [windows-latest, ubuntu-latest, macos-latest]
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: screenbuddy.exe
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: screenbuddy
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: screenbuddy
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
      - name: Build release
        run: cargo build --release --verbose
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: screenbuddy-${{ matrix.target }}
          path: target/release/${{ matrix.artifact }}
''')

# === Pre-commit hooks ===
os.makedirs(r"G:\Projects\ScreenBuddy\.githooks", exist_ok=True)
with open(r"G:\Projects\ScreenBuddy\.githooks\pre-commit", "w") as f:
    f.write('''#!/bin/sh
# ScreenBuddy pre-commit hook
echo "Running pre-commit checks..."

# Check formatting
cargo fmt --all -- --check
if [ $? -ne 0 ]; then
    echo "❌ Formatting check failed. Run 'cargo fmt' to fix."
    exit 1
fi

# Run clippy
cargo clippy --all-targets -- -D warnings
if [ $? -ne 0 ]; then
    echo "❌ Clippy check failed."
    exit 1
fi

# Run tests
cargo test --workspace
if [ $? -ne 0 ]; then
    echo "❌ Tests failed."
    exit 1
fi

echo "✅ All checks passed!"
''')

# === Docker support ===
with open(r"G:\Projects\ScreenBuddy\Dockerfile", "w") as f:
    f.write('''FROM rust:1.75-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY tools/ ./tools/

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libasound2-dev libudev-dev libx11-dev libxcb1-dev \
    libxcomposite-dev libxcursor-dev libxrandr-dev libxi-dev \
    libgl1-mesa-dev && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/screenbuddy /usr/local/bin/
CMD ["screenbuddy"]
''')

# === Docker Compose ===
with open(r"G:\Projects\ScreenBuddy\docker-compose.yml", "w") as f:
    f.write('''version: '3.8'

services:
  screenbuddy:
    build: .
    volumes:
      - ./assets:/app/assets
      - ./config:/root/.config/ScreenBuddy
    environment:
      - DISPLAY=host.docker.internal:0
    network_mode: host
    restart: unless-stopped
''')

# === Makefile for local CI/CD ===
with open(r"G:\Projects\ScreenBuddy\Makefile", "w") as f:
    f.write('''.PHONY: all check test build release clean lint fmt docker

all: check test build

check:
	cargo check --workspace --all-targets

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --workspace --verbose

build:
	cargo build --workspace

release:
	cargo build --release --workspace

clean:
	cargo clean

docker:
	docker compose build

run:
	cargo run --bin screenbuddy

watch:
	cargo watch -x run
''')

# === .dockerignore ===
with open(r"G:\Projects\ScreenBuddy\.dockerignore", "w") as f:
    f.write('''target/
.git/
*.log
.env
''')

# === README CI badge ===
readme_path = r"G:\Projects\ScreenBuddy\README.md"
if os.path.exists(readme_path):
    with open(readme_path, "r") as f:
        readme = f.read()
    if "CI" not in readme:
        readme = "# ScreenBuddy 🐦\n\n![CI](https://github.com/screenbuddy/screenbuddy/workflows/ScreenBuddy%20CI/badge.svg)\n\nA next-generation desktop AI companion platform.\n\n" + readme
        with open(readme_path, "w") as f:
            f.write(readme)

print("All files written successfully!")
