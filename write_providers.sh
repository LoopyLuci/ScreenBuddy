#!/bin/bash
# Write all ScreenBuddy files - comprehensive script
cd /G/Projects/ScreenBuddy

# Remove any stray directory at root
rm -rf screenbuddy-core 2>/dev/null

# Create directory structure
mkdir -p crates/screenbuddy-core/src
mkdir -p .github/workflows
mkdir -p .githooks

echo "=== Writing core library files ==="

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
echo "  error.rs"

# === providers.rs ===
cat > crates/screenbuddy-core/src/providers.rs << 'EOF'
/// AI Provider Ecosystem for ScreenBuddy
///
/// Supports 20+ AI providers and local hosting solutions.
/// Cloud APIs: OpenAI, Anthropic, Google, Grok, OpenRouter, Together, etc.
/// Local: Ollama, LLM Studio, LM Studio, vLLM, Unsloth, llama.cpp

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi, Anthropic, Google, Grok, OpenRouter, Together,
    Deepseek, Mistral, Cohere, Perplexity, Replicate, Huggingface,
    Ollama, LlmStudio, LmStudio, Vllm, Unsloth, LlamaCpp,
    TextGenerationWebui, Koboldcpp, Custom,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::OpenAi => write!(f, "OpenAI"),
            Provider::Anthropic => write!(f, "Anthropic"),
            Provider::Google => write!(f, "Google"),
            Provider::Grok => write!(f, "Grok"),
            Provider::OpenRouter => write!(f, "OpenRouter"),
            Provider::Together => write!(f, "Together AI"),
            Provider::Deepseek => write!(f, "DeepSeek"),
            Provider::Mistral => write!(f, "Mistral"),
            Provider::Cohere => write!(f, "Cohere"),
            Provider::Perplexity => write!(f, "Perplexity"),
            Provider::Replicate => write!(f, "Replicate"),
            Provider::Huggingface => write!(f, "Hugging Face"),
            Provider::Ollama => write!(f, "Ollama"),
            Provider::LlmStudio => write!(f, "LLM Studio"),
            Provider::LmStudio => write!(f, "LM Studio"),
            Provider::Vllm => write!(f, "vLLM"),
            Provider::Unsloth => write!(f, "Unsloth"),
            Provider::LlamaCpp => write!(f, "llama.cpp"),
            Provider::TextGenerationWebui => write!(f, "Text Generation WebUI"),
            Provider::Koboldcpp => write!(f, "Koboldcpp"),
            Provider::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
    pub enabled: bool,
    pub priority: u32,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            provider: Provider::OpenAi,
            api_key: None,
            base_url: None,
            model: None,
            max_tokens: 2048,
            temperature: 0.7,
            timeout_secs: 120,
            enabled: true,
            priority: 100,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub supports_functions: bool,
    pub supports_chat: bool,
    pub max_context_window: u32,
    pub cost_per_1k_input_tokens: f64,
    pub cost_per_1k_output_tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRegistry {
    pub providers: HashMap<Provider, ProviderConfig>,
    pub default_provider: Provider,
    pub fallback_enabled: bool,
    pub auto_detect_local: bool,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        let mut providers = HashMap::new();
        
        // Cloud providers
        providers.insert(Provider::OpenAi, ProviderConfig {
            provider: Provider::OpenAi,
            model: Some("gpt-4o".to_string()),
            max_tokens: 4096,
            priority: 10,
            ..Default::default()
        });
        providers.insert(Provider::Anthropic, ProviderConfig {
            provider: Provider::Anthropic,
            model: Some("claude-3-5-sonnet-20241022".to_string()),
            max_tokens: 4096,
            priority: 11,
            ..Default::default()
        });
        providers.insert(Provider::Google, ProviderConfig {
            provider: Provider::Google,
            model: Some("gemini-1.5-pro".to_string()),
            max_tokens: 4096,
            priority: 12,
            ..Default::default()
        });
        providers.insert(Provider::Grok, ProviderConfig {
            provider: Provider::Grok,
            model: Some("grok-beta".to_string()),
            max_tokens: 4096,
            priority: 13,
            ..Default::default()
        });
        providers.insert(Provider::OpenRouter, ProviderConfig {
            provider: Provider::OpenRouter,
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            model: Some("auto".to_string()),
            max_tokens: 4096,
            priority: 20,
            ..Default::default()
        });
        providers.insert(Provider::Together, ProviderConfig {
            provider: Provider::Together,
            base_url: Some("https://api.together.xyz/v1".to_string()),
            model: Some("meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string()),
            max_tokens: 4096,
            priority: 21,
            ..Default::default()
        });
        providers.insert(Provider::Deepseek, ProviderConfig {
            provider: Provider::Deepseek,
            base_url: Some("https://api.deepseek.com/v1".to_string()),
            model: Some("deepseek-chat".to_string()),
            max_tokens: 4096,
            priority: 22,
            ..Default::default()
        });
        providers.insert(Provider::Mistral, ProviderConfig {
            provider: Provider::Mistral,
            base_url: Some("https://api.mistral.ai/v1".to_string()),
            model: Some("mistral-large-latest".to_string()),
            max_tokens: 4096,
            priority: 23,
            ..Default::default()
        });
        providers.insert(Provider::Cohere, ProviderConfig {
            provider: Provider::Cohere,
            base_url: Some("https://api.cohere.com/v1".to_string()),
            model: Some("command-r-plus".to_string()),
            max_tokens: 4096,
            priority: 24,
            ..Default::default()
        });
        providers.insert(Provider::Perplexity, ProviderConfig {
            provider: Provider::Perplexity,
            base_url: Some("https://api.perplexity.ai".to_string()),
            model: Some("llama-3.1-sonar-large-128k-online".to_string()),
            max_tokens: 4096,
            priority: 25,
            ..Default::default()
        });
        providers.insert(Provider::Replicate, ProviderConfig {
            provider: Provider::Replicate,
            base_url: Some("https://api.replicate.com/v1".to_string()),
            model: Some("meta/meta-llama-3.1-405b-instruct".to_string()),
            max_tokens: 4096,
            priority: 26,
            ..Default::default()
        });
        providers.insert(Provider::Huggingface, ProviderConfig {
            provider: Provider::Huggingface,
            base_url: Some("https://api-inference.huggingface.co".to_string()),
            model: Some("meta-llama/Llama-3.3-70B-Instruct".to_string()),
            max_tokens: 4096,
            priority: 27,
            ..Default::default()
        });
        
        // Local providers
        providers.insert(Provider::Ollama, ProviderConfig {
            provider: Provider::Ollama,
            base_url: Some("http://localhost:11434/v1".to_string()),
            model: Some("llama3.2".to_string()),
            max_tokens: 4096,
            priority: 50,
            ..Default::default()
        });
        providers.insert(Provider::LlmStudio, ProviderConfig {
            provider: Provider::LlmStudio,
            base_url: Some("http://localhost:1234/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 51,
            ..Default::default()
        });
        providers.insert(Provider::LmStudio, ProviderConfig {
            provider: Provider::LmStudio,
            base_url: Some("http://localhost:1234/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 52,
            ..Default::default()
        });
        providers.insert(Provider::Vllm, ProviderConfig {
            provider: Provider::Vllm,
            base_url: Some("http://localhost:8000/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 53,
            ..Default::default()
        });
        providers.insert(Provider::Unsloth, ProviderConfig {
            provider: Provider::Unsloth,
            base_url: Some("http://localhost:8000/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 54,
            ..Default::default()
        });
        providers.insert(Provider::LlamaCpp, ProviderConfig {
            provider: Provider::LlamaCpp,
            base_url: Some("http://localhost:8080/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 55,
            ..Default::default()
        });
        providers.insert(Provider::TextGenerationWebui, ProviderConfig {
            provider: Provider::TextGenerationWebui,
            base_url: Some("http://localhost:5000/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 56,
            ..Default::default()
        });
        providers.insert(Provider::Koboldcpp, ProviderConfig {
            provider: Provider::Koboldcpp,
            base_url: Some("http://localhost:5001/v1".to_string()),
            model: Some("local-model".to_string()),
            max_tokens: 4096,
            priority: 57,
            ..Default::default()
        });
        
        Self {
            providers,
            default_provider: Provider::OpenAi,
            fallback_enabled: true,
            auto_detect_local: true,
        }
    }
}

impl ProviderRegistry {
    pub fn get_best_provider(&self) -> Option<&ProviderConfig> {
        let mut providers: Vec<&ProviderConfig> = self.providers.values().filter(|p| p.enabled).collect();
        providers.sort_by_key(|p| p.priority);
        providers.first().copied()
    }

    pub fn get(&self, provider: &Provider) -> Option<&ProviderConfig> { self.providers.get(provider) }
    pub fn get_mut(&mut self, provider: &Provider) -> Option<&mut ProviderConfig> { self.providers.get_mut(provider) }

    pub fn set_api_key(&mut self, provider: Provider, key: String) {
        if let Some(config) = self.providers.get_mut(&provider) { config.api_key = Some(key); config.enabled = true; }
    }

    pub fn enable(&mut self, provider: Provider) {
        if let Some(config) = self.providers.get_mut(&provider) { config.enabled = true; }
    }

    pub fn disable(&mut self, provider: Provider) {
        if let Some(config) = self.providers.get_mut(&provider) { config.enabled = false; }
    }

    pub fn set_default(&mut self, provider: Provider) { self.default_provider = provider; }

    pub fn enabled_providers(&self) -> Vec<&ProviderConfig> {
        self.providers.values().filter(|p| p.enabled).collect()
    }
}

pub struct LocalProviderDetector;

impl LocalProviderDetector {
    pub async fn detect_running() -> Vec<Provider> {
        let mut running = Vec::new();
        let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build().unwrap_or_default();
        
        if client.get("http://localhost:11434/api/tags").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::Ollama);
        }
        if client.get("http://localhost:1234/v1/models").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::LlmStudio);
            running.push(Provider::LmStudio);
        }
        if client.get("http://localhost:8000/v1/models").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::Vllm);
            running.push(Provider::Unsloth);
        }
        if client.get("http://localhost:8080/health").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::LlamaCpp);
        }
        if client.get("http://localhost:5000/v1/models").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::TextGenerationWebui);
        }
        if client.get("http://localhost:5001/api/v1/model").send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            running.push(Provider::Koboldcpp);
        }
        running
    }
}

pub struct ProviderBuilder;

impl ProviderBuilder {
    pub fn from_env() -> ProviderRegistry {
        let mut registry = ProviderRegistry::default();
        for (var, provider) in &[
            ("OPENAI_API_KEY", Provider::OpenAi),
            ("ANTHROPIC_API_KEY", Provider::Anthropic),
            ("GOOGLE_API_KEY", Provider::Google),
            ("GROK_API_KEY", Provider::Grok),
            ("OPENROUTER_API_KEY", Provider::OpenRouter),
            ("TOGETHER_API_KEY", Provider::Together),
            ("DEEPSEEK_API_KEY", Provider::Deepseek),
            ("MISTRAL_API_KEY", Provider::Mistral),
            ("COHERE_API_KEY", Provider::Cohere),
            ("PERPLEXITY_API_KEY", Provider::Perplexity),
            ("REPLICATE_API_KEY", Provider::Replicate),
            ("HUGGINGFACE_API_KEY", Provider::Huggingface),
        ] {
            if let Ok(key) = std::env::var(var) {
                registry.set_api_key(*provider, key);
            }
        }
        registry
    }

    pub fn from_config(path: &PathBuf) -> Result<ProviderRegistry> {
        let content = std::fs::read_to_string(path)?;
        let registry: ProviderRegistry = toml::from_str(&content)
            .map_err(|e| crate::error::Error::InvalidConfig(format!("Failed to parse config: {}", e)))?;
        Ok(registry)
    }
}

pub fn get_capabilities(provider: &Provider) -> ProviderCapabilities {
    match provider {
        Provider::OpenAi => ProviderCapabilities { supports_streaming: true, supports_vision: true, supports_functions: true, supports_chat: true, max_context_window: 128_000, cost_per_1k_input_tokens: 0.005, cost_per_1k_output_tokens: 0.015 },
        Provider::Anthropic => ProviderCapabilities { supports_streaming: true, supports_vision: true, supports_functions: true, supports_chat: true, max_context_window: 200_000, cost_per_1k_input_tokens: 0.003, cost_per_1k_output_tokens: 0.015 },
        Provider::Google => ProviderCapabilities { supports_streaming: true, supports_vision: true, supports_functions: true, supports_chat: true, max_context_window: 1_000_000, cost_per_1k_input_tokens: 0.00125, cost_per_1k_output_tokens: 0.005 },
        Provider::Grok => ProviderCapabilities { supports_streaming: true, supports_vision: true, supports_functions: true, supports_chat: true, max_context_window: 128_000, cost_per_1k_input_tokens: 0.002, cost_per_1k_output_tokens: 0.01 },
        Provider::Ollama => ProviderCapabilities { supports_streaming: true, supports_vision: true, supports_functions: false, supports_chat: true, max_context_window: 128_000, cost_per_1k_input_tokens: 0.0, cost_per_1k_output_tokens: 0.0 },
        _ => ProviderCapabilities { supports_streaming: true, supports_vision: false, supports_functions: false, supports_chat: true, max_context_window: 32_000, cost_per_1k_input_tokens: 0.0, cost_per_1k_output_tokens: 0.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_display() {
        assert_eq!(format!("{}", Provider::OpenAi), "OpenAI");
        assert_eq!(format!("{}", Provider::Ollama), "Ollama");
    }

    #[test]
    fn test_registry_default() {
        let registry = ProviderRegistry::default();
        assert!(registry.get(&Provider::OpenAi).is_some());
        assert!(registry.get_best_provider().is_some());
    }

    #[test]
    fn test_registry_set_api_key() {
        let mut registry = ProviderRegistry::default();
        registry.set_api_key(Provider::OpenAi, "test-key".to_string());
        assert_eq!(registry.get(&Provider::OpenAi).unwrap().api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_enabled_providers() {
        let registry = ProviderRegistry::default();
        assert!(!registry.enabled_providers().is_empty());
    }

    #[test]
    fn test_capabilities() {
        let caps = get_capabilities(&Provider::OpenAi);
        assert!(caps.supports_streaming);
        assert!(caps.supports_vision);
    }
}
EOF
echo "  providers.rs"

echo "=== All files written ==="
ls -la crates/screenbuddy-core/src/
