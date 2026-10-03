use crate::error::{Error, Result};
use crate::nine_router::{self, RouterModel};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Maximum transcript messages sent per request.
///
/// The newest are kept: during a tool-use loop the recent exchange is what the
/// model needs, so truncation must discard the oldest.
const MAX_HISTORY_MESSAGES: usize = 20;

/// Placeholders for a malformed tool tag, so one malformed entry cannot make a
/// whole request invalid.
const TOOL_ID_FALLBACK: &str = "call_unknown";
const TOOL_NAME_FALLBACK: &str = "tool";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ModelTier {
    LocalTiny,
    LocalMedium,
    LocalLarge,
    CloudBudget,
    CloudPremium,
    CloudMax,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Backend {
    Ollama,
    LlamaCpp,
    OpenAi,
    Anthropic,
    /// [9Router](https://github.com/decolua/9router): an OpenAI-compatible
    /// router that fronts many providers with automatic fallback.
    NineRouter,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub force_tier: Option<ModelTier>,
    pub history: Option<Vec<Message>>,
    /// Tool schemas advertised to the model for this request.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
}

/// A tool the model is allowed to call, described in JSON Schema form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A tool invocation requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub text: String,
    pub model_tier: ModelTier,
    pub backend: Backend,
    pub tokens_used: u32,
    pub finish_reason: FinishReason,
    /// Tool invocations the model requested, if any.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    Error(String),
}

impl AiResponse {
    /// Convenience constructor for backend responses that carry no tool calls.
    fn plain(
        text: String,
        model_tier: ModelTier,
        backend: Backend,
        tokens_used: u32,
        finish_reason: FinishReason,
    ) -> Self {
        Self {
            text,
            model_tier,
            backend,
            tokens_used,
            finish_reason,
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub ollama_endpoint: Option<String>,
    pub llamacpp_endpoint: Option<String>,
    pub custom_endpoint: Option<String>,
    pub custom_api_key: Option<String>,
    /// Base URL for a [9Router](https://github.com/decolua/9router) instance.
    /// Defaults to its documented local port when unset.
    pub nine_router_endpoint: Option<String>,
    /// Optional bearer token. 9Router accepts requests without one when it runs
    /// without auth, so this is only sent when set.
    pub nine_router_api_key: Option<String>,
    pub local_model_path: Option<PathBuf>,
    pub local_model_type: LocalModelType,
    pub default_tier: ModelTier,
    pub max_tokens_default: u32,
    pub offline_only: bool,
    pub cloud_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalModelType {
    Ollama,
    LlamaCpp,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            ollama_endpoint: Some("http://localhost:11434".into()),
            llamacpp_endpoint: Some("http://localhost:8080".into()),
            custom_endpoint: None,
            custom_api_key: None,
            nine_router_endpoint: None,
            nine_router_api_key: None,
            local_model_path: None,
            local_model_type: LocalModelType::Ollama,
            default_tier: ModelTier::LocalTiny,
            max_tokens_default: 2048,
            offline_only: true,
            cloud_fallback: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub tier: ModelTier,
    pub backend: Backend,
    pub name: String,
    pub max_context: u32,
    pub cost: f64,
}

#[derive(Clone)]
pub struct AiEngine {
    config: AiConfig,
    models: Vec<ModelInfo>,
    client: reqwest::Client,
}

impl AiEngine {
    pub fn new(config: AiConfig) -> Self {
        let models = Self::build_model_list(&config);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("HTTP client");
        Self {
            config,
            models,
            client,
        }
    }

    pub async fn generate(&self, req: AiRequest) -> Result<AiResponse> {
        let tier = req
            .force_tier
            .unwrap_or_else(|| self.auto_select_tier(&req));
        let model = self.select_model(tier)?;
        match model.backend {
            Backend::Ollama => self.gen_ollama(&req, model).await,
            Backend::LlamaCpp => self.gen_llama(&req, model).await,
            Backend::OpenAi => self.gen_openai(&req, model).await,
            Backend::Anthropic => self.gen_anthropic(&req, model).await,
            Backend::NineRouter => self.gen_nine_router(&req, model).await,
            Backend::Custom => self.gen_custom(&req, model).await,
        }
    }

    fn auto_select_tier(&self, req: &AiRequest) -> ModelTier {
        let c = self.estimate_complexity(&req.prompt);
        if self.config.offline_only {
            if c < 0.3 {
                ModelTier::LocalTiny
            } else if c < 0.7 {
                ModelTier::LocalMedium
            } else {
                ModelTier::LocalLarge
            }
        } else {
            if c < 0.2 && self.has_local() {
                ModelTier::LocalTiny
            } else if c < 0.5 && self.has_local() {
                ModelTier::LocalMedium
            } else if c < 0.8 {
                ModelTier::CloudBudget
            } else {
                ModelTier::CloudPremium
            }
        }
    }

    fn estimate_complexity(&self, p: &str) -> f64 {
        let base = (p.len() as f64 / 1000.0).min(1.0);
        if ["function", "class", "implement", "debug", "code"]
            .iter()
            .any(|k| p.to_lowercase().contains(k))
        {
            (base + 0.3).min(1.0)
        } else {
            base
        }
    }

    fn has_local(&self) -> bool {
        self.models
            .iter()
            .any(|m| matches!(m.backend, Backend::Ollama | Backend::LlamaCpp))
    }

    fn select_model(&self, tier: ModelTier) -> Result<&ModelInfo> {
        self.models
            .iter()
            .find(|m| m.tier == tier)
            .or_else(|| self.models.first())
            .ok_or_else(|| crate::error::Error::InvalidConfig("No models".into()))
    }

    fn build_model_list(config: &AiConfig) -> Vec<ModelInfo> {
        let mut m = Vec::new();
        // Ollama — always available if endpoint configured
        if config.ollama_endpoint.is_some() {
            m.push(ModelInfo {
                tier: ModelTier::LocalTiny,
                backend: Backend::Ollama,
                name: "llama3.2:1b".into(),
                max_context: 8192,
                cost: 0.0,
            });
            m.push(ModelInfo {
                tier: ModelTier::LocalMedium,
                backend: Backend::Ollama,
                name: "llama3.2".into(),
                max_context: 8192,
                cost: 0.0,
            });
            m.push(ModelInfo {
                tier: ModelTier::LocalLarge,
                backend: Backend::Ollama,
                name: "llama3.1:70b".into(),
                max_context: 128000,
                cost: 0.0,
            });
        }
        // llama.cpp server
        if config.llamacpp_endpoint.is_some() {
            m.push(ModelInfo {
                tier: ModelTier::LocalMedium,
                backend: Backend::LlamaCpp,
                name: "local-llama".into(),
                max_context: 4096,
                cost: 0.0,
            });
        }
        if !config.offline_only {
            if config.openai_api_key.is_some() {
                m.push(ModelInfo {
                    tier: ModelTier::CloudBudget,
                    backend: Backend::OpenAi,
                    name: "gpt-4o-mini".into(),
                    max_context: 128000,
                    cost: 0.00015,
                });
                m.push(ModelInfo {
                    tier: ModelTier::CloudPremium,
                    backend: Backend::OpenAi,
                    name: "gpt-4o".into(),
                    max_context: 128000,
                    cost: 0.005,
                });
            }
            if config.anthropic_api_key.is_some() {
                m.push(ModelInfo {
                    tier: ModelTier::CloudBudget,
                    backend: Backend::Anthropic,
                    name: "claude-3-5-haiku-20241022".into(),
                    max_context: 200000,
                    cost: 0.001,
                });
                m.push(ModelInfo {
                    tier: ModelTier::CloudPremium,
                    backend: Backend::Anthropic,
                    name: "claude-3-5-sonnet-20241022".into(),
                    max_context: 200000,
                    cost: 0.015,
                });
            }
        }
        m
    }

    /// Tool schemas in OpenAI function-calling format.
    fn openai_tools(tools: &[ToolSpec]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect()
    }

    /// Tool schemas in Anthropic format.
    fn anthropic_tools(tools: &[ToolSpec]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect()
    }

    /// Parse `choices[0].message.tool_calls` (OpenAI, Ollama, OpenAI-compatible).
    ///
    /// Ollama reports tool calls at `message.tool_calls`, OpenAI at
    /// `choices[0].message.tool_calls`, so both are probed. A missing or
    /// malformed call list yields an empty vec rather than an error: text-only
    /// responses must keep working.
    fn parse_openai_tool_calls(json: &serde_json::Value) -> Vec<ToolCall> {
        let raw = json["choices"][0]["message"]["tool_calls"]
            .as_array()
            .or_else(|| json["message"]["tool_calls"].as_array());
        let Some(raw) = raw else { return Vec::new() };
        raw.iter()
            .enumerate()
            .map(|(i, c)| {
                let name = c["function"]["name"]
                    .as_str()
                    .or_else(|| c["name"].as_str())
                    .unwrap_or_default()
                    .to_string();
                // Arguments arrive as a JSON *string* on OpenAI and as an
                // already-parsed object on Ollama; accept both.
                let arguments = match &c["function"]["arguments"] {
                    serde_json::Value::String(s) => {
                        serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
                    }
                    other => other.clone(),
                };
                let id = c["id"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("call_{i}"));
                ToolCall {
                    id,
                    name,
                    arguments,
                }
            })
            .filter(|c| !c.name.is_empty())
            .collect()
    }

    /// Parse Anthropic `content` blocks of `type == "tool_use"`.
    fn parse_anthropic_tool_calls(json: &serde_json::Value) -> Vec<ToolCall> {
        json["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"].as_str() == Some("tool_use"))
                    .enumerate()
                    .map(|(i, b)| ToolCall {
                        id: b["id"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("call_{i}")),
                        name: b["name"].as_str().unwrap_or_default().to_string(),
                        arguments: b["input"].clone(),
                    })
                    .filter(|c| !c.name.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn build_messages(&self, req: &AiRequest) -> Vec<serde_json::Value> {
        let mut msgs = Vec::new();
        if let Some(sys) = &req.system_prompt {
            msgs.push(serde_json::json!({"role":"system","content":sys}));
        }
        if let Some(hist) = &req.history {
            // Keep the *newest* messages: on a long tool-use loop the relevant
            // context is the most recent exchange, and truncating from the front
            // would discard exactly the messages the model needs.
            for msg in Self::recent_messages(hist, MAX_HISTORY_MESSAGES) {
                msgs.push(Self::message_json(msg));
            }
        }
        // Only append the prompt when it carries content. On continuation turns
        // the transcript already ends with the tool results, and an empty user
        // turn would be rejected by some backends.
        if !req.prompt.is_empty() {
            msgs.push(serde_json::json!({"role":"user","content":&req.prompt}));
        }
        msgs
    }

    /// The last `max` messages, preserving chronological order.
    fn recent_messages(history: &[Message], max: usize) -> &[Message] {
        if history.len() <= max {
            history
        } else {
            &history[history.len() - max..]
        }
    }

    /// Render one transcript entry into an OpenAI-shaped message.
    ///
    /// Tool turns are stored with a tagged encoding written by `AgentRuntime`:
    /// `[tool_use <id> <name> <json-args>]` and `[tool_result <id> <name>] <text>`.
    /// The shared `<id>` lets a tool result reference the exact call it answers,
    /// which OpenAI requires via `tool_call_id`.
    fn message_json(msg: &Message) -> serde_json::Value {
        let content = msg.content.as_str();

        if let Some(rest) = content.strip_prefix("[tool_use ") {
            // The tag closes with ']', which belongs to the encoding rather than
            // the JSON argument object; strip it or the args fail to parse.
            let rest = rest.strip_suffix(']').unwrap_or(rest);
            let mut parts = rest.splitn(3, ' ');
            let (id, name, args) = match (parts.next(), parts.next(), parts.next()) {
                (Some(id), Some(name), Some(args)) => (id, name, args),
                _ => (TOOL_ID_FALLBACK, TOOL_NAME_FALLBACK, "{}"),
            };
            return serde_json::json!({
                "role": "assistant",
                "content": serde_json::Value::Null,
                "tool_calls": [{
                    "id": id,
                    "type": "function",
                    "function": {"name": name, "arguments": args},
                }],
            });
        }

        if let Some(rest) = content.strip_prefix("[tool_result ") {
            let (head, output) = match rest.split_once(']') {
                Some((h, o)) => (h, o.trim_start()),
                None => (rest, ""),
            };
            let mut parts = head.splitn(2, ' ');
            let id = parts.next().unwrap_or(TOOL_ID_FALLBACK);
            let name = parts.next().unwrap_or(TOOL_NAME_FALLBACK);
            return serde_json::json!({
                "role": "tool",
                "tool_call_id": id,
                "name": name,
                "content": output,
            });
        }

        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        };
        serde_json::json!({"role":role,"content":content})
    }

    // ============ OLLAMA ============
    async fn gen_ollama(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let ep = self.config.ollama_endpoint.as_ref().unwrap();
        let url = format!("{}/api/chat", ep);
        let mut body = serde_json::json!({
            "model": model.name,
            "messages": self.build_messages(req),
            "stream": false,
            "options": {
                "num_predict": req.max_tokens.unwrap_or(self.config.max_tokens_default),
                "temperature": req.temperature.unwrap_or(0.7),
            }
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(crate::error::Error::Network(format!(
                "Ollama error: {}",
                resp.status()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["message"]["content"].as_str().unwrap_or("").into(),
                model.tier,
                model.backend,
                json["eval_count"].as_u64().unwrap_or(0) as u32,
                if json["done"].as_bool().unwrap_or(false) {
                    FinishReason::Stop
                } else {
                    FinishReason::Length
                },
            )
        })
    }

    // ============ LLAMA.CPP ============
    async fn gen_llama(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let ep = self.config.llamacpp_endpoint.as_ref().unwrap();
        let url = format!("{}/completion", ep);
        let prompt = if let Some(sys) = &req.system_prompt {
            format!(
                "{}

{}",
                sys, req.prompt
            )
        } else {
            req.prompt.clone()
        };
        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
            "stop": ["</s>", "user:", "assistant:"],
        });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(crate::error::Error::Network(format!(
                "llama.cpp error: {}",
                resp.status()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        // The /completion endpoint has no native function-calling field; tool use
        // there is driven by GBNF grammar constraints, so the raw text is the answer.
        Ok(AiResponse::plain(
            json["content"].as_str().unwrap_or("").into(),
            model.tier,
            model.backend,
            json["tokens_predicted"].as_u64().unwrap_or(0) as u32,
            FinishReason::Stop,
        ))
    }

    // ============ OPENAI ============
    async fn gen_openai(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let key = self
            .config
            .openai_api_key
            .as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("No OpenAI key".into()))?;
        let mut body = serde_json::json!({
            "model": model.name,
            "messages": self.build_messages(req),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }
        let resp = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", key))
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!(
                "OpenAI error: {}",
                err_text
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
                model.tier,
                model.backend,
                json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
                FinishReason::Stop,
            )
        })
    }

    /// Render one transcript entry for Anthropic's content-block format.
    ///
    /// Anthropic requires tool results in a `user` turn containing a `tool_result`
    /// block that references the `tool_use` id, so tagged transcript entries are
    /// converted rather than passed through as text. `None` means "drop this
    /// entry" (a system message, which belongs in the top-level `system` field).
    fn anthropic_message(msg: &Message) -> Option<serde_json::Value> {
        let content = msg.content.as_str();

        if let Some(rest) = content.strip_prefix("[tool_use ") {
            // The tag closes with ']', which belongs to the encoding rather than
            // the JSON argument object; strip it or the args fail to parse.
            let rest = rest.strip_suffix(']').unwrap_or(rest);
            let mut parts = rest.splitn(3, ' ');
            let (id, name, args) = match (parts.next(), parts.next(), parts.next()) {
                (Some(id), Some(name), Some(args)) => (id, name, args),
                _ => (TOOL_ID_FALLBACK, TOOL_NAME_FALLBACK, "{}"),
            };
            let input: serde_json::Value =
                serde_json::from_str(args).unwrap_or(serde_json::Value::Object(Default::default()));
            return Some(serde_json::json!({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                }],
            }));
        }

        if let Some(rest) = content.strip_prefix("[tool_result ") {
            let (head, output) = match rest.split_once(']') {
                Some((h, o)) => (h, o.trim_start()),
                None => (rest, ""),
            };
            let id = head.split(' ').next().unwrap_or(TOOL_ID_FALLBACK);
            // Anthropic wants tool results in a user turn.
            return Some(serde_json::json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": output,
                }],
            }));
        }

        let role = match msg.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            // System text belongs in the request's top-level `system` field.
            MessageRole::System => return None,
        };
        Some(serde_json::json!({"role": role, "content": content}))
    }

    // ============ ANTHROPIC ============
    async fn gen_anthropic(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let key = self
            .config
            .anthropic_api_key
            .as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("No Anthropic key".into()))?;
        let mut messages = Vec::new();
        if let Some(hist) = &req.history {
            // Keep the newest entries, matching the OpenAI path.
            for msg in Self::recent_messages(hist, MAX_HISTORY_MESSAGES) {
                if let Some(entry) = Self::anthropic_message(msg) {
                    messages.push(entry);
                }
            }
        }
        // Skip an empty continuation prompt; the transcript already ends with
        // the tool results, and Anthropic rejects an empty final user turn.
        if !req.prompt.is_empty() {
            messages.push(serde_json::json!({"role":"user","content":&req.prompt}));
        }
        let mut body = serde_json::json!({
            "model": model.name,
            "system": req.system_prompt.clone().unwrap_or_default(),
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::anthropic_tools(&req.tools));
        }
        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!(
                "Anthropic error: {}",
                err_text
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            tool_calls: Self::parse_anthropic_tool_calls(&json),
            ..AiResponse::plain(
                json["content"][0]["text"].as_str().unwrap_or("").into(),
                model.tier,
                model.backend,
                json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                    + json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                match json["stop_reason"].as_str() {
                    Some("end_turn") => FinishReason::Stop,
                    Some("max_tokens") => FinishReason::Length,
                    Some("tool_use") => FinishReason::Stop,
                    other => FinishReason::Error(other.unwrap_or("unknown").into()),
                },
            )
        })
    }

    // ============ CUSTOM / OpenRouter / Together / etc ============
    async fn gen_custom(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let ep = self
            .config
            .custom_endpoint
            .as_ref()
            .ok_or_else(|| crate::error::Error::InvalidConfig("No endpoint".into()))?;
        let mut body = serde_json::json!({
            "model": model.name,
            "messages": self.build_messages(req),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }
        let mut r = self
            .client
            .post(ep)
            .header("content-type", "application/json")
            .json(&body);
        if let Some(key) = &self.config.custom_api_key {
            r = r.header("Authorization", format!("Bearer {}", key));
        }
        let resp = r
            .send()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!(
                "Custom API error: {}",
                err_text
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        // OpenAI-compatible response format
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
                model.tier,
                model.backend,
                json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
                FinishReason::Stop,
            )
        })
    }

    pub fn simple(&self, prompt: &str) -> String {
        let p = prompt.to_lowercase();
        if p.contains("hello") || p.contains("hi ") {
            "Hello! I'm your ScreenBuddy companion. How can I help?".into()
        } else if p.contains("help") {
            "I can: chat, answer questions, control your buddy, and more! Try clicking the tray icon.".into()
        } else if p.contains("joke") {
            "Why do programmers prefer dark mode? Because light attracts bugs! 🐛".into()
        } else if p.contains("name") {
            "I'm ScreenBuddy, your AI desktop companion! 🐦".into()
        } else {
            format!(
                "I heard: '{}'. Connect a real AI model (Ollama, OpenAI, etc.) for full responses!",
                prompt
            )
        }
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }
    pub fn available_models(&self) -> &[ModelInfo] {
        &self.models
    }
}

impl AiEngine {
    /// Chat completion through 9Router.
    ///
    /// The body is OpenAI-shaped, which is what 9Router expects; the difference
    /// from the plain OpenAI path is the base URL, the optional bearer token,
    /// and an error message that names 9Router so a failure is diagnosable.
    pub async fn gen_nine_router(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let base = nine_router::base_url(self.config.nine_router_endpoint.as_deref());
        let url = nine_router::endpoint(&base, "chat/completions");

        let mut body = serde_json::json!({
            // 9Router expects the upstream-qualified name it advertises.
            "model": model.name,
            "messages": self.build_messages(req),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
            "stream": false,
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }

        let mut request = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body);

        // 9Router may run without auth, so only send a token when configured
        // rather than sending an empty "Bearer".
        if let Some(key) = self
            .config
            .nine_router_api_key
            .as_ref()
            .filter(|k| !k.trim().is_empty())
        {
            request = request.header("Authorization", format!("Bearer {}", key.trim()));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("9Router unreachable at {url}: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "9Router returned {}: {}",
                status,
                nine_router::truncate(&detail, 400)
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("9Router sent an unreadable reply: {e}")))?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| Error::Network("9Router reply had no message content".into()))?
            .to_string();

        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                text,
                model.tier,
                model.backend,
                json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
                FinishReason::Stop,
            )
        })
    }

    /// Ask 9Router which models it can reach.
    ///
    /// This is the discovery path: the router's catalogue depends on which
    /// providers the user has configured, so it cannot be hardcoded here.
    pub async fn nine_router_models(&self) -> Result<Vec<RouterModel>> {
        self.nine_router_models_at(None, None).await
    }

    /// Model discovery against an explicit base URL and key, overriding config.
    ///
    /// [`Self::nine_router_models`] is the normal path and reads the configured
    /// endpoint; this exists for probing a second router or for tests.
    pub async fn nine_router_models_at(
        &self,
        configured: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<Vec<RouterModel>> {
        let base =
            nine_router::base_url(configured.or(self.config.nine_router_endpoint.as_deref()));
        let url = nine_router::endpoint(&base, "models");

        let mut request = self.client.get(&url);
        let key = api_key.or(self.config.nine_router_api_key.as_deref());
        if let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) {
            request = request.header("Authorization", format!("Bearer {key}"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(format!("9Router unreachable at {url}: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(Error::Network(format!(
                "9Router returned {} listing models: {}",
                status,
                nine_router::truncate(&detail, 200)
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::Network(format!("9Router model list unreadable: {e}")))?;

        // OpenAI shape: {"data": [{"id": "...", ...}]}
        let entries = json["data"]
            .as_array()
            .ok_or_else(|| Error::Network("9Router model list had no 'data' array".into()))?;

        let mut models = Vec::with_capacity(entries.len());
        for entry in entries {
            match serde_json::from_value::<RouterModel>(entry.clone()) {
                Ok(model) => models.push(model),
                // Skip entries without an id rather than failing the whole list:
                // one malformed entry should not hide every usable model.
                Err(_) => continue,
            }
        }
        // Sorted so the picker does not reshuffle between calls.
        models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_tool_calls_with_string_arguments() {
        let json = serde_json::json!({
            "choices": [{"message": {"content": null, "tool_calls": [
                {"id": "call_1", "type": "function",
                 "function": {"name": "echo", "arguments": "{\"text\":\"hi\"}"}}
            ]}}]
        });
        let calls = AiEngine::parse_openai_tool_calls(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].id, "call_1");
        // Arguments must be decoded from the JSON string into a real value.
        assert_eq!(calls[0].arguments["text"].as_str(), Some("hi"));
    }

    #[test]
    fn parses_ollama_tool_calls_with_object_arguments() {
        // Ollama nests tool_calls under `message` and passes arguments as an object.
        let json = serde_json::json!({
            "message": {"content": "", "tool_calls": [
                {"function": {"name": "time", "arguments": {}}}
            ]}
        });
        let calls = AiEngine::parse_openai_tool_calls(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "time");
        assert!(calls[0].arguments.is_object());
        // Missing id is synthesised so tool results can still be correlated.
        assert_eq!(calls[0].id, "call_0");
    }

    #[test]
    fn parses_anthropic_tool_use_blocks() {
        let json = serde_json::json!({
            "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "toolu_1", "name": "echo", "input": {"text": "hi"}}
            ]
        });
        let calls = AiEngine::parse_anthropic_tool_calls(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].id, "toolu_1");
        assert_eq!(calls[0].arguments["text"].as_str(), Some("hi"));
    }

    #[test]
    fn text_only_responses_yield_no_tool_calls() {
        let openai = serde_json::json!({
            "choices": [{"message": {"content": "hello"}}]
        });
        assert!(AiEngine::parse_openai_tool_calls(&openai).is_empty());

        let ollama = serde_json::json!({"message": {"content": "hello"}});
        assert!(AiEngine::parse_openai_tool_calls(&ollama).is_empty());

        let anthropic = serde_json::json!({"content": [{"type": "text", "text": "hello"}]});
        assert!(AiEngine::parse_anthropic_tool_calls(&anthropic).is_empty());
    }

    #[test]
    fn malformed_tool_calls_do_not_panic() {
        // Wrong types and empty names must be tolerated, not crash or error.
        let json = serde_json::json!({
            "choices": [{"message": {"tool_calls": [
                {"id": 42, "function": {"name": 7, "arguments": "not json"}},
                {"function": {"name": "", "arguments": {}}},
                "garbage"
            ]}}]
        });
        let calls = AiEngine::parse_openai_tool_calls(&json);
        // Only the entry with a non-empty name survives filtering.
        assert_eq!(calls.len(), 0);
    }

    #[test]
    fn tool_specs_serialise_to_openai_function_shape() {
        let specs = vec![ToolSpec {
            name: "echo".into(),
            description: "Echo back the input".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        let out = AiEngine::openai_tools(&specs);
        assert_eq!(out[0]["type"].as_str(), Some("function"));
        assert_eq!(out[0]["function"]["name"].as_str(), Some("echo"));
        assert_eq!(
            out[0]["function"]["parameters"]["type"].as_str(),
            Some("object")
        );
    }

    #[test]
    fn tool_specs_serialise_to_anthropic_shape() {
        let specs = vec![ToolSpec {
            name: "echo".into(),
            description: "Echo back the input".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        let out = AiEngine::anthropic_tools(&specs);
        assert_eq!(out[0]["name"].as_str(), Some("echo"));
        assert_eq!(out[0]["input_schema"]["type"].as_str(), Some("object"));
    }

    #[test]
    fn default_ai_config_offers_local_models() {
        let engine = AiEngine::new(AiConfig::default());
        assert!(!engine.available_models().is_empty());
    }

    fn msg(role: MessageRole, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
        }
    }

    #[test]
    fn history_truncation_keeps_the_newest_messages() {
        // Regression: truncation used `.take(20)`, keeping the OLDEST messages
        // and dropping the recent tool exchange the model needs to answer.
        let history: Vec<Message> = (0..50)
            .map(|i| msg(MessageRole::User, &format!("m{i}")))
            .collect();
        let engine = AiEngine::new(AiConfig::default());
        let req = AiRequest {
            prompt: "next".into(),
            history: Some(history),
            ..Default::default()
        };
        let msgs = engine.build_messages(&req);
        // Default config sets no system_prompt: 20 history + 1 prompt.
        assert_eq!(msgs.len(), 21);
        let hist_slice = &msgs[..20];
        assert_eq!(hist_slice[0]["content"].as_str(), Some("m30"));
        assert_eq!(hist_slice[19]["content"].as_str(), Some("m49"));
        assert_eq!(msgs[20]["content"].as_str(), Some("next"));
    }

    #[test]
    fn short_history_is_not_truncated() {
        let history: Vec<Message> = (0..5)
            .map(|i| msg(MessageRole::User, &format!("m{i}")))
            .collect();
        let engine = AiEngine::new(AiConfig::default());
        let req = AiRequest {
            prompt: "next".into(),
            history: Some(history),
            ..Default::default()
        };
        // 5 history + 1 prompt, no system message.
        assert_eq!(engine.build_messages(&req).len(), 6);
    }

    #[test]
    fn empty_continuation_prompt_is_omitted() {
        // A continuation turn must not send an empty user message.
        let engine = AiEngine::new(AiConfig::default());
        let req = AiRequest {
            prompt: String::new(),
            history: Some(vec![msg(MessageRole::User, "hi")]),
            ..Default::default()
        };
        let msgs = engine.build_messages(&req);
        assert!(msgs.iter().all(|m| m["content"] != serde_json::json!("")));
        assert!(!msgs
            .iter()
            .any(|m| m["role"] == "user" && m["content"] == ""));
    }

    #[test]
    fn tool_use_entry_renders_native_tool_calls() {
        let out = AiEngine::message_json(&msg(
            MessageRole::Assistant,
            r#"[tool_use call_abc123 echo {"text":"hi"}]"#,
        ));
        assert_eq!(out["role"], "assistant");
        assert_eq!(out["tool_calls"][0]["id"], "call_abc123");
        assert_eq!(out["tool_calls"][0]["type"], "function");
        assert_eq!(out["tool_calls"][0]["function"]["name"], "echo");
        assert!(out["tool_calls"][0]["function"]["arguments"].is_string());
    }

    #[test]
    fn tool_result_entry_uses_the_tool_role_with_matching_id() {
        // OpenAI requires role="tool" plus tool_call_id matching the assistant call.
        let out =
            AiEngine::message_json(&msg(MessageRole::User, "[tool_result call_abc123 echo] hi"));
        assert_eq!(out["role"], "tool");
        assert_eq!(out["tool_call_id"], "call_abc123");
        assert_eq!(out["name"], "echo");
        assert_eq!(out["content"], "hi");
    }

    #[test]
    fn malformed_tool_tags_do_not_break_rendering() {
        // Truncated tags must still produce a structurally valid message.
        let use_out = AiEngine::message_json(&msg(MessageRole::Assistant, "[tool_use call_1 echo"));
        // Only two fields present, so arguments are missing and fall back to "{}".
        assert_eq!(use_out["tool_calls"][0]["id"], TOOL_ID_FALLBACK);
        assert_eq!(
            use_out["tool_calls"][0]["function"]["name"],
            TOOL_NAME_FALLBACK
        );
        assert_eq!(use_out["tool_calls"][0]["function"]["arguments"], "{}");

        let result_out = AiEngine::message_json(&msg(MessageRole::User, "[tool_result call_2]"));
        assert_eq!(result_out["tool_call_id"], "call_2");
    }

    #[test]
    fn anthropic_tool_use_block_is_native() {
        let out = AiEngine::anthropic_message(&msg(
            MessageRole::Assistant,
            r#"[tool_use call_abc123 echo {"text":"hi"}]"#,
        ))
        .unwrap();
        assert_eq!(out["role"], "assistant");
        assert_eq!(out["content"][0]["type"], "tool_use");
        assert_eq!(out["content"][0]["id"], "call_abc123");
        assert_eq!(out["content"][0]["name"], "echo");
        // Arguments must be parsed into a real object, not left as a string.
        assert_eq!(out["content"][0]["input"]["text"].as_str(), Some("hi"));
    }

    #[test]
    fn anthropic_tool_result_block_is_native() {
        // Anthropic requires tool_result inside a user turn.
        let out = AiEngine::anthropic_message(&msg(
            MessageRole::User,
            "[tool_result call_abc123 echo] hi",
        ))
        .unwrap();
        assert_eq!(out["role"], "user");
        assert_eq!(out["content"][0]["type"], "tool_result");
        assert_eq!(out["content"][0]["tool_use_id"], "call_abc123");
        assert_eq!(out["content"][0]["content"], "hi");
    }

    #[test]
    fn anthropic_drops_system_messages_from_the_turn_list() {
        assert!(AiEngine::anthropic_message(&msg(MessageRole::System, "be nice")).is_none());
    }

    #[test]
    fn anthropic_keeps_plain_messages() {
        let out = AiEngine::anthropic_message(&msg(MessageRole::Assistant, "hello")).unwrap();
        assert_eq!(out["role"], "assistant");
        assert_eq!(out["content"], "hello");
    }
}
