use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ModelTier { LocalTiny, LocalMedium, LocalLarge, CloudBudget, CloudPremium, CloudMax }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Backend { Ollama, LlamaCpp, OpenAi, Anthropic, Custom }

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for AiRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            stream: false,
            force_tier: None,
            history: None,
            tools: Vec::new(),
        }
    }
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
pub struct Message { pub role: MessageRole, pub content: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole { System, User, Assistant }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub text: String, pub model_tier: ModelTier, pub backend: Backend,
    pub tokens_used: u32, pub finish_reason: FinishReason,
    /// Tool invocations the model requested, if any.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason { Stop, Length, Error(String) }

impl AiResponse {
    /// Convenience constructor for backend responses that carry no tool calls.
    fn plain(
        text: String,
        model_tier: ModelTier,
        backend: Backend,
        tokens_used: u32,
        finish_reason: FinishReason,
    ) -> Self {
        Self { text, model_tier, backend, tokens_used, finish_reason, tool_calls: Vec::new() }
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
    pub local_model_path: Option<PathBuf>,
    pub local_model_type: LocalModelType,
    pub default_tier: ModelTier,
    pub max_tokens_default: u32,
    pub offline_only: bool,
    pub cloud_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalModelType { Ollama, LlamaCpp }

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            ollama_endpoint: Some("http://localhost:11434".into()),
            llamacpp_endpoint: Some("http://localhost:8080".into()),
            custom_endpoint: None, custom_api_key: None,
            local_model_path: None, local_model_type: LocalModelType::Ollama,
            default_tier: ModelTier::LocalTiny, max_tokens_default: 2048,
            offline_only: true, cloud_fallback: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelInfo { pub tier: ModelTier, pub backend: Backend, pub name: String, pub max_context: u32, pub cost: f64 }

#[derive(Clone)]
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
            Backend::Ollama => self.gen_ollama(&req, model).await,
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

    fn has_local(&self) -> bool { self.models.iter().any(|m| matches!(m.backend, Backend::Ollama | Backend::LlamaCpp)) }

    fn select_model(&self, tier: ModelTier) -> Result<&ModelInfo> {
        self.models.iter().find(|m| m.tier == tier).or_else(|| self.models.first())
            .ok_or_else(|| crate::error::Error::InvalidConfig("No models".into()))
    }

    fn build_model_list(config: &AiConfig) -> Vec<ModelInfo> {
        let mut m = Vec::new();
        // Ollama — always available if endpoint configured
        if config.ollama_endpoint.is_some() {
            m.push(ModelInfo { tier: ModelTier::LocalTiny, backend: Backend::Ollama, name: "llama3.2:1b".into(), max_context: 8192, cost: 0.0 });
            m.push(ModelInfo { tier: ModelTier::LocalMedium, backend: Backend::Ollama, name: "llama3.2".into(), max_context: 8192, cost: 0.0 });
            m.push(ModelInfo { tier: ModelTier::LocalLarge, backend: Backend::Ollama, name: "llama3.1:70b".into(), max_context: 128000, cost: 0.0 });
        }
        // llama.cpp server
        if config.llamacpp_endpoint.is_some() {
            m.push(ModelInfo { tier: ModelTier::LocalMedium, backend: Backend::LlamaCpp, name: "local-llama".into(), max_context: 4096, cost: 0.0 });
        }
        if !config.offline_only {
            if config.openai_api_key.is_some() {
                m.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::OpenAi, name: "gpt-4o-mini".into(), max_context: 128000, cost: 0.00015 });
                m.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::OpenAi, name: "gpt-4o".into(), max_context: 128000, cost: 0.005 });
            }
            if config.anthropic_api_key.is_some() {
                m.push(ModelInfo { tier: ModelTier::CloudBudget, backend: Backend::Anthropic, name: "claude-3-5-haiku-20241022".into(), max_context: 200000, cost: 0.001 });
                m.push(ModelInfo { tier: ModelTier::CloudPremium, backend: Backend::Anthropic, name: "claude-3-5-sonnet-20241022".into(), max_context: 200000, cost: 0.015 });
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
                ToolCall { id, name, arguments }
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
        let resp = self.client.post(&url).json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(crate::error::Error::Network(format!("Ollama error: {}", resp.status())));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["message"]["content"].as_str().unwrap_or("").into(),
                model.tier,
                model.backend,
                json["eval_count"].as_u64().unwrap_or(0) as u32,
                if json["done"].as_bool().unwrap_or(false) { FinishReason::Stop } else { FinishReason::Length },
            )
        })
    }

    // ============ LLAMA.CPP ============
    async fn gen_llama(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let ep = self.config.llamacpp_endpoint.as_ref().unwrap();
        let url = format!("{}/completion", ep);
        let prompt = if let Some(sys) = &req.system_prompt {
            format!("{}

{}", sys, req.prompt)
        } else { req.prompt.clone() };
        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
            "stop": ["</s>", "user:", "assistant:"],
        });
        let resp = self.client.post(&url).json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(crate::error::Error::Network(format!("llama.cpp error: {}", resp.status())));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
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
        let key = self.config.openai_api_key.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No OpenAI key".into()))?;
        let mut body = serde_json::json!({
            "model": model.name,
            "messages": self.build_messages(req),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }
        let resp = self.client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", key))
            .json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!("OpenAI error: {}", err_text)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["choices"][0]["message"]["content"].as_str().unwrap_or("").into(),
                model.tier,
                model.backend,
                json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
                FinishReason::Stop,
            )
        })
    }

    // ============ ANTHROPIC ============
    async fn gen_anthropic(&self, req: &AiRequest, model: &ModelInfo) -> Result<AiResponse> {
        let key = self.config.anthropic_api_key.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No Anthropic key".into()))?;
        let mut messages = Vec::new();
        if let Some(hist) = &req.history {
            for msg in hist.iter().take(20) {
                let role = match msg.role { MessageRole::User => "user", MessageRole::Assistant => "assistant", MessageRole::System => continue };
                messages.push(serde_json::json!({"role": role, "content": &msg.content}));
            }
        }
        messages.push(serde_json::json!({"role":"user","content":&req.prompt}));
        let mut body = serde_json::json!({
            "model": model.name,
            "system": req.system_prompt.clone().unwrap_or_default(),
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::anthropic_tools(&req.tools));
        }
        let resp = self.client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body).send().await
            .map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!("Anthropic error: {}", err_text)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
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
        let ep = self.config.custom_endpoint.as_ref().ok_or_else(|| crate::error::Error::InvalidConfig("No endpoint".into()))?;
        let mut body = serde_json::json!({
            "model": model.name,
            "messages": self.build_messages(req),
            "max_tokens": req.max_tokens.unwrap_or(self.config.max_tokens_default),
            "temperature": req.temperature.unwrap_or(0.7),
        });
        if !req.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(Self::openai_tools(&req.tools));
        }
        let mut r = self.client.post(ep).header("content-type", "application/json").json(&body);
        if let Some(key) = &self.config.custom_api_key { r = r.header("Authorization", format!("Bearer {}", key)); }
        let resp = r.send().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(crate::error::Error::Network(format!("Custom API error: {}", err_text)));
        }
        let json: serde_json::Value = resp.json().await.map_err(|e| crate::error::Error::Network(e.to_string()))?;
        // OpenAI-compatible response format
        Ok(AiResponse {
            tool_calls: Self::parse_openai_tool_calls(&json),
            ..AiResponse::plain(
                json["choices"][0]["message"]["content"].as_str().unwrap_or("").into(),
                model.tier,
                model.backend,
                json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
                FinishReason::Stop,
            )
        })
    }

    pub fn simple(&self, prompt: &str) -> String {
        let p = prompt.to_lowercase();
        if p.contains("hello") || p.contains("hi ") { "Hello! I'm your ScreenBuddy companion. How can I help?".into() }
        else if p.contains("help") { "I can: chat, answer questions, control your buddy, and more! Try clicking the tray icon.".into() }
        else if p.contains("joke") { "Why do programmers prefer dark mode? Because light attracts bugs! 🐛".into() }
        else if p.contains("name") { "I'm ScreenBuddy, your AI desktop companion! 🐦".into() }
        else { format!("I heard: '{}'. Connect a real AI model (Ollama, OpenAI, etc.) for full responses!", prompt) }
    }

    pub fn config(&self) -> &AiConfig { &self.config }
    pub fn available_models(&self) -> &[ModelInfo] { &self.models }
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
        assert_eq!(out[0]["function"]["parameters"]["type"].as_str(), Some("object"));
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
}

