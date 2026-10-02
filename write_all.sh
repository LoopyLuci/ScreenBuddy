#!/bin/bash
# Write all ScreenBuddy files at once

cd /G/Projects/ScreenBuddy

# Remove stray directory
rm -rf screenbuddy-core

# Create directory structure
mkdir -p crates/screenbuddy-core/src
mkdir -p .github/workflows
mkdir -p .githooks

# === error.rs ===
cat > crates/screenbuddy-core/src/error.rs << 'EOF'
use thiserror::Error;

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
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Other: {0}")]
    Other(String),
}
EOF

echo "Written error.rs"

# === ai.rs ===
cat > crates/screenbuddy-core/src/ai.rs << 'EOF'
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
    pub text: String, pub model_tier: ModelTier, pub backend: Backend,
    pub tokens_used: u32, pub finish_reason: FinishReason,
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
struct ModelInfo { tier: ModelTier, backend: Backend, name: String, max_context: u32, cost: f64 }

pub struct AiEngine {
    config: AiConfig, models: Vec<ModelInfo>, client: reqwest::Client,
}

impl AiEngine {
    pub fn new(config: AiConfig) -> Self {
        let models = Self::build_model_list(&config);
        let client = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().expect("HTTP client");
        Self { config, models, client }
    }

    pub async fn generate(&self, req: AiRequest) -> Result<AiResponse> {
        let tier = req.force_tier.unwrap_or_else(|| self.auto_select_tier(&req));
        let model = self.select_model(tier)?;
        match model.backend {
            Backend::Candle => self.gen_candle(&req, model).await,
            Backend::LlamaCpp => self.gen_llama(&req, model).await,
            Backend::OpenAi => self.gen_openai(&req, model).await,
            Backend::Anthropic => self.gen_anthropic(&req, model).await,
            Backend::Custom => self.gen_custom(&req, model).await,
        }
    }

    fn auto_select_tier(&self, req: &AiRequest) -> ModelTier {
        let c = self.estimate_complexity(&req.prompt);
        if self.config.offline_only {
            if c < 0.3 { ModelTier::LocalTiny } else if c < 0.7 { ModelTier::LocalMedium } else { ModelTier::LocalLarge }
        } else {
            if c < 0.2 && self.has_local() { ModelTier::LocalTiny }
            else if c < 0.5 && self.has_local() { ModelTier::LocalMedium }
            else if c < 0.8 { ModelTier::CloudBudget } else { ModelTier::CloudPremium }
        }
    }

    fn estimate_complexity(&self, p: &str) -> f64 {
        let base = (p.len() as f64 / 1000.0).min(1.0);
        if ["function","class","implement","debug","code"].iter().any(|k| p.to_lowercase().contains(k)) {
            (base + 0.3).min(1.0)
        } else { base }
    }

    fn has_local(&self) -> bool { self.models.iter().any(|m| matches!(m.backend, Backend::Candle | Backend::LlamaCpp)) }

    fn select_model(&self, tier: ModelTier) -> Result<&ModelInfo> {
        self.models.iter().find(|m| m.tier == tier).or_else(|| self.models.first())
            .ok_or_else(|| crate::error::Error::InvalidConfig("No models".into()))
    }

    fn build_model_list(config: &AiConfig) -> Vec<ModelInfo> {
        let mut m = Vec::new();
        if config.local_model_path.is_some() {
            let (name, ctx, backend) = match config.local_model_type {
                LocalModelType::LlamaCpp => ("local-llama", 4096, Backend::LlamaCpp),
                LocalModelType::Candle => ("local-candle", 2048, Backend::Candle),
            };
            m.push(ModelInfo { tier: config.default_tier, backend, name: name.into(), max_context: ctx, cost: 0.0 });
        }
        if !config.offline_only {
            if config.openai_api_key.is_some() {
                m.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::OpenAi, name: "gpt-3.5-turbo".into(), max_context: 16384, cost: 0.0005 });
                m.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::OpenAi, name: "gpt-4o".into(), max_context: 128000, cost: 0.005 });
            }
            if config.anthropic_api_key.is_some() {
                m.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::Anthropic, name: "claude-3-haiku-20240307".into(), max_context: 200000, cost: 0.00025 });
                m.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::Anthropic, name: "claude-3-sonnet-20240229".into(), max_context: 200000, cost: 0.003 });
            }
        }
        m
    }

    fn build_messages(&self, req: &AiRequest) -> Vec<serde_json::Value> {
        let mut msgs = Vec::new();
        if let Some(sys) = &req.system_prompt { msgs.push(serde_json::json!({"role":"system","content":sys})); }
        if let Some(hist) = &req.history {
            for msg in hist.iter().take(20) {
                let role = match msg.role { MessageRole::System => "system", MessageRole::User => "user", MessageRole::Assistant => "assistant" };
                msgs.push(serde_json::json!({"role":role,"content":&msg.content}));
            }
        }
        msgs.push(serde_json::json!({"role":"user","content":&req.prompt}));
        msgs
    }

    async fn gen_candle(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        Ok(AiResponse { text: format!("[Candle/{}] {}", model.name, self.simple(&req.prompt)), model_tier: model.tier, backend: model.backend, tokens_used: 0, finish_reason: FinishReason::Stop })
    }

    async fn gen_llama(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        Ok(AiResponse { text: format!("[llama.cpp/{}] {}", model.name, self.simple(&req.prompt)), model_tier: model.tier, backend: model.backend, tokens_used: 0, finish_reason: FinishReason::Stop })
    }

    async fn gen_openai(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let key = self.config.openai_api_key.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No OpenAI key".into()))?;
        let body = serde_json::json!({ "model": model.name, "messages": self.build_messages(req), "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default), "temperature": req.temperature.unwrap_or(0.7) });
        let resp = self.client.post("https://api.openai.com/v1/chat/completions").header("Authorization", format!("Bearer {}", key)).json(&body).send().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse { text: json["choices"][0]["message"]["content"].as_str().unwrap_or("").into(), model_tier: model.tier, backend: model.backend, tokens_used: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32, finish_reason: FinishReason::Stop })
    }

    async fn gen_anthropic(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let key = self.config.anthropic_api_key.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No Anthropic key".into()))?;
        let body = serde_json::json!({ "model": model.name, "messages": self.build_messages(req), "system": req.system_prompt.clone().unwrap_or_default(), "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default) });
        let resp = self.client.post("https://api.anthropic.com/v1/messages").header("x-api-key", key).header("anthropic-version", "2023-06-01").json(&body).send().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse { text: json["content"][0]["text"].as_str().unwrap_or("").into(), model_tier: model.tier, backend: model.backend, tokens_used: 0, finish_reason: FinishReason::Stop })
    }

    async fn gen_custom(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let ep = self.config.custom_endpoint.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No endpoint".into()))?;
        let body = serde_json::json!({ "messages": self.build_messages(req) });
        let mut r = self.client.post(ep).json(&body);
        if let Some(key) = &self.config.custom_api_key { r = r.header("Authorization", format!("Bearer {}", key)); }
        let resp = r.send().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse { text: json["choices"][0]["message"]["content"].as_str().unwrap_or("").into(), model_tier: model.tier, backend: model.backend, tokens_used: 0, finish_reason: FinishReason::Stop })
    }

    fn simple(&self, prompt: &str) -> String {
        let p = prompt.to_lowercase();
        if p.contains("hello") || p.contains("hi") { "Hello! I'm your ScreenBuddy companion. How can I help?".into() }
        else if p.contains("help") { "I can: take notes, search web, answer questions, control your buddy.".into() }
        else if p.contains("joke") { "Why do programmers prefer dark mode? Because light attracts bugs! 🐛".into() }
        else { format!("I heard: '{}'. Connect a real AI model for full responses!", prompt) }
    }
}

impl Clone for AiEngine {
    fn clone(&self) -> Self { Self { config: self.config.clone(), models: self.models.clone(), client: self.client.clone() } }
}
EOF

echo "Written ai.rs"

# === lib.rs ===
cat > crates/screenbuddy-core/src/lib.rs << 'EOF'
pub mod agent;
pub mod ai;
pub mod creature;
pub mod error;
pub mod ipc;
pub mod rag;
pub mod state;

pub use error::{Error, Result};
pub use creature::{Creature, CreatureId};
pub use state::{AppState, CreatureState, State};
pub use ai::{AiEngine, AiConfig, AiRequest, AiResponse, ModelTier, Backend, Message};
pub use rag::{RagPipeline, InMemoryVectorStore, TfIdfEmbedding, VectorStore, Chunk};
pub use agent::{AgentRuntime, AgentEvent, Tool};
pub use ipc::{IpcServer, IpcClient, GodotCommand, Response, DEFAULT_PORT};
EOF

echo "Written lib.rs"
echo "All files written successfully"
ls -la crates/screenbuddy-core/src/
