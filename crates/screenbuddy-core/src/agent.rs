//! Agentic Runtime for ScreenBuddy
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;

use crate::ai::{AiEngine, AiRequest, Message, MessageRole, ToolSpec};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    Thinking(String),
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        name: String,
        output: String,
    },
    Message(String),
    Done,
    Error(String),
}

pub type ToolHandler = Arc<
    dyn Fn(
            serde_json::Value,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>
        + Send
        + Sync,
>;

pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: ToolHandler,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_iterations: usize,
    pub timeout_secs: u64,
    pub system_prompt: String,
    /// A model name to pin, or empty to let the engine choose by tier.
    pub model: String,
    /// Sampling temperature for this agent.
    pub temperature: f32,
    /// Whether this agent may use tools.
    #[serde(default = "default_true")]
    pub tools_enabled: bool,
}

/// Serialisation default for `tools_enabled`, so an older config file without
/// the field means "tools on" rather than "tools off".
fn default_true() -> bool {
    true
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout_secs: 120,
            system_prompt: "You are ScreenBuddy, a helpful desktop AI companion.".to_string(),
            model: String::new(),
            temperature: 0.7,
            tools_enabled: true,
        }
    }
}

pub struct AgentRuntime {
    ai: Arc<AiEngine>,
    tools: Arc<Mutex<HashMap<String, Tool>>>,
    /// Shared so applying an agent profile takes effect on the next request,
    /// rather than only at construction. Without this an agent's persona, tool
    /// budget and timeout were inert.
    config: Arc<RwLock<AgentConfig>>,
    /// The agent currently in effect, for status and diagnostics.
    active_agent: Arc<Mutex<Option<String>>>,
    tx: broadcast::Sender<AgentEvent>,
}

impl AgentRuntime {
    pub fn new(ai: Arc<AiEngine>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            ai,
            tools: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(RwLock::new(AgentConfig::default())),
            active_agent: Arc::new(Mutex::new(None)),
            tx,
        }
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = Arc::new(RwLock::new(config));
        self
    }

    /// Apply an agent profile's runtime settings.
    ///
    /// This is the link that makes a saved agent actually do anything: its
    /// composed system prompt, tool budget and timeout take effect on the next
    /// request.
    pub fn apply_profile(&self, profile: &crate::agent_profile::AgentProfile) {
        let config = profile.to_agent_config();
        {
            let Ok(mut current) = self.config.write() else {
                return;
            };
            *current = config;
        }
        if let Ok(mut active) = self.active_agent.lock() {
            *active = Some(profile.id.clone());
        }
    }

    /// The id of the agent currently in effect.
    pub fn active_agent(&self) -> Option<String> {
        self.active_agent
            .lock()
            .ok()
            .and_then(|a| a.clone())
            .filter(|id| !id.is_empty())
    }

    /// A snapshot of the settings in force, for `get_agent_info`.
    pub fn active_config(&self) -> AgentConfig {
        self.config
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|_| AgentConfig::default())
    }

    pub fn register_tool(&self, name: &str, tool: Tool) {
        // std::sync::Mutex: the guard is only ever held for a map insert, never
        // across an await, so it cannot deadlock the runtime.
        let mut tools = self.tools.lock().unwrap_or_else(|e| e.into_inner());
        tools.insert(name.to_string(), tool);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.tx.subscribe()
    }

    pub async fn run(&self, user_input: String) -> Result<String> {
        let _ = self
            .tx
            .send(AgentEvent::Thinking("Starting...".to_string()));
        let mut response = String::new();
        // Conversation transcript, built up across tool-use iterations. Empty
        // until the first tool call, which is what selects the first-turn shape
        // below (raw prompt) versus continuations (history).
        let mut history: Vec<Message> = Vec::new();
        // Registered tools, advertised on every turn so the model can call them.
        let tools_enabled = {
            let Ok(guard) = self.config.read() else {
                return Err(crate::error::Error::InvalidConfig(
                    "agent config lock poisoned".into(),
                ));
            };
            guard.tools_enabled
        };
        let tools: Vec<ToolSpec> = if !tools_enabled {
            // Advertising tools the agent may not use invites calls it cannot
            // answer, so an agent with tools off advertises none.
            Vec::new()
        } else {
            let map = self.tools.lock().unwrap_or_else(|e| e.into_inner());
            map.values()
                .map(|t| ToolSpec {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                })
                .collect()
        };
        // Snapshot once per run: an agent applied mid-request must not change the
        // budget part way through, and holding the read guard across an await
        // would block the writer.
        let config = self.active_config();
        let max_iterations = config.max_iterations.max(1);
        // An agent may pin a model by name; empty keeps tier selection.
        let pinned_model = if config.model.trim().is_empty() {
            None
        } else {
            Some(config.model.trim().to_string())
        };

        for iteration in 0..max_iterations {
            let _ = self.tx.send(AgentEvent::Thinking(if iteration == 0 {
                "Thinking...".to_string()
            } else {
                format!("Thinking... (step {}/{max_iterations})", iteration + 1)
            }));

            // Subsequent turns are continuations after tool results, so the
            // conversation history carries the exchange and the prompt is a
            // short nudge rather than the original user input repeated.
            let (prompt, turn_history) = if history.is_empty() {
                (user_input.clone(), None)
            } else {
                (String::new(), Some(history.clone()))
            };

            let req = AiRequest {
                prompt,
                system_prompt: Some(config.system_prompt.clone()),
                max_tokens: Some(1024),
                // The agent's own temperature, not a fixed default.
                temperature: Some(config.temperature),
                stream: false,
                force_tier: None,
                history: turn_history,
                tools: tools.clone(),
                force_model: pinned_model.clone(),
            };

            let ai_result = tokio::time::timeout(
                Duration::from_secs(config.timeout_secs),
                self.ai.generate(req),
            )
            .await;

            let ai_resp = match ai_result {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    let _ = self.tx.send(AgentEvent::Error(e.to_string()));
                    break;
                }
                Err(_) => {
                    let _ = self.tx.send(AgentEvent::Error("Timeout".to_string()));
                    break;
                }
            };

            if ai_resp.tool_calls.is_empty() {
                // No tools requested: this is the final answer.
                response = ai_resp.text.clone();
                if !response.is_empty() {
                    let _ = self.tx.send(AgentEvent::Message(response.clone()));
                }
                let _ = self.tx.send(AgentEvent::Done);
                break;
            }

            // Record the assistant turn that requested the tools. Any text it
            // produced alongside the calls is preserved as context.
            let mut assistant_content = ai_resp.text.clone();
            for call in &ai_resp.tool_calls {
                let _ = self.tx.send(AgentEvent::ToolUse {
                    name: call.name.clone(),
                    input: call.arguments.clone(),
                });
                let output = self.dispatch(&call.name, call.arguments.clone()).await;
                let _ = self.tx.send(AgentEvent::ToolResult {
                    name: call.name.clone(),
                    output: output.clone(),
                });
                if !assistant_content.is_empty() {
                    assistant_content.push('\n');
                }
                assistant_content.push_str(&format!("[called {} -> {}]", call.name, output));
                // The call id is carried in the transcript so the matching
                // tool_result can reference it via `tool_call_id` (OpenAI) or
                // `tool_use_id` (Anthropic).
                history.push(Message {
                    role: MessageRole::Assistant,
                    content: format!("[tool_use {} {} {}]", call.id, call.name, call.arguments),
                });
                history.push(Message {
                    role: MessageRole::User,
                    content: format!("[tool_result {} {}] {}", call.id, call.name, output),
                });
            }
            if !ai_resp.text.is_empty() {
                history.push(Message {
                    role: MessageRole::Assistant,
                    content: ai_resp.text,
                });
            }
            response = assistant_content;
        }

        Ok(response)
    }

    /// Invoke a registered tool by name, returning its output or an error string.
    async fn dispatch(&self, name: &str, input: serde_json::Value) -> String {
        // Clone the handler out and release the lock before awaiting it, so no
        // guard is ever held across an await point.
        let handler = {
            let map = self.tools.lock().unwrap_or_else(|e| e.into_inner());
            map.get(name).map(|t| t.handler.clone())
        };
        match handler {
            Some(h) => match h(input).await {
                Ok(out) => out,
                Err(e) => format!("error: {e}"),
            },
            None => format!("error: unknown tool '{name}'"),
        }
    }

    pub fn tool_count(&self) -> usize {
        self.tools.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

pub fn default_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "echo".to_string(),
            description: "Echo back the input".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
            handler: Arc::new(|input| {
                Box::pin(async move {
                    Ok(input
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string())
                })
            }),
        },
        Tool {
            name: "time".to_string(),
            description: "Get current time".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            handler: Arc::new(|_| Box::pin(async move { Ok(chrono::Local::now().to_rfc3339()) })),
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

    /// Regression: an agent profile's settings reached nowhere. The config was
    /// fixed at construction and nothing ever called it, so persona, tool budget,
    /// timeout, temperature and model were all inert. These assert the values
    /// the runtime will actually use.
    #[test]
    fn applying_a_profile_changes_the_settings_in_force() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);

        let mut profile = crate::agent_profile::AgentProfile::new("p", "P");
        profile.system_prompt = "ZZZ distinct role".into();
        profile.max_iterations = 3;
        profile.timeout_secs = 42;
        profile.temperature = 1.4;
        profile.tools_enabled = false;
        profile.model = "cc/claude-opus-4-6".into();
        agent.apply_profile(&profile);

        let config = agent.active_config();
        assert!(
            config.system_prompt.contains("ZZZ"),
            "prompt must be applied"
        );
        assert_eq!(config.max_iterations, 3);
        assert_eq!(config.timeout_secs, 42);
        assert!((config.temperature - 1.4).abs() < f32::EPSILON);
        assert!(!config.tools_enabled, "tools must be able to be turned off");
        assert_eq!(config.model, "cc/claude-opus-4-6");
        assert_eq!(agent.active_agent().as_deref(), Some("p"));
    }

    #[test]
    fn applying_a_second_profile_replaces_the_first() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);

        let mut first = crate::agent_profile::AgentProfile::new("a", "A");
        first.max_iterations = 5;
        agent.apply_profile(&first);

        let mut second = crate::agent_profile::AgentProfile::new("b", "B");
        second.max_iterations = 11;
        agent.apply_profile(&second);

        assert_eq!(agent.active_config().max_iterations, 11);
        assert_eq!(agent.active_agent().as_deref(), Some("b"));
    }

    #[test]
    fn a_fresh_runtime_reports_no_active_agent() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);
        assert_eq!(agent.active_agent(), None);
        // Defaults must still be sane before any profile is applied.
        assert!(agent.active_config().max_iterations >= 1);
    }

    /// The composed prompt is what the model sees, so persona must reach it.
    #[test]
    fn a_profile_persona_reaches_the_runtime_system_prompt() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);

        let mut profile = crate::agent_profile::AgentProfile::new("p", "P");
        profile.persona = crate::agent_profile::Persona::Terse;
        profile.system_prompt = "You are a test agent.".into();
        agent.apply_profile(&profile);

        let prompt = agent.active_config().system_prompt;
        assert!(
            prompt.contains("few words"),
            "persona guidance missing from the live prompt: {prompt}"
        );
        assert!(prompt.contains("You are a test agent."));
    }

    #[test]
    fn test_tool_specs_are_advertised() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);
        for tool in default_tools() {
            agent.register_tool(&tool.name.clone(), tool);
        }
        assert_eq!(agent.tool_count(), 2);
    }

    #[tokio::test]
    async fn test_dispatch_returns_tool_output() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);
        for tool in default_tools() {
            agent.register_tool(&tool.name.clone(), tool);
        }
        // echo returns its input text.
        let out = agent
            .dispatch("echo", serde_json::json!({"text":"hi"}))
            .await;
        assert_eq!(out, "hi");
    }

    #[tokio::test]
    async fn test_dispatch_unknown_tool_is_reported() {
        let ai = Arc::new(AiEngine::new(AiConfig::default()));
        let agent = AgentRuntime::new(ai);
        let out = agent.dispatch("nope", serde_json::json!({})).await;
        assert!(out.contains("unknown tool"), "got: {out}");
    }

    #[tokio::test]
    async fn test_run_emits_done_on_failure_without_panicking() {
        // No local server is running, so generate() fails; the loop must surface
        // an Error event and still terminate rather than spinning.
        let config = AiConfig {
            ollama_endpoint: Some("http://127.0.0.1:1".into()),
            llamacpp_endpoint: Some("http://127.0.0.1:1".into()),
            ..AiConfig::default()
        };
        let agent = AgentRuntime::new(Arc::new(AiEngine::new(config))).with_config(AgentConfig {
            max_iterations: 3,
            timeout_secs: 2,
            ..Default::default()
        });
        let mut events = agent.subscribe();
        let out = agent.run("hello".into()).await.unwrap();
        assert!(
            out.is_empty(),
            "no response expected on failure, got: {out}"
        );
        let mut saw_error = false;
        while let Ok(ev) = events.try_recv() {
            if matches!(ev, AgentEvent::Error(_)) {
                saw_error = true;
            }
        }
        assert!(saw_error, "expected an Error event");
    }
}
