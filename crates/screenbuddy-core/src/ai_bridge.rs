//! AI Chat Bridge for ScreenBuddy
//!
//! Bridges the AiEngine with the ChatOverlay for real-time conversation.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::chat_overlay::{ChatOverlay, ChatOverlayEvent, MessageRole};
use crate::ai::{AiEngine, AiRequest};

/// AI Chat Bridge - connects AiEngine to ChatOverlay
pub struct AiChatBridge {
    ai: Arc<AiEngine>,
    overlay: Arc<Mutex<ChatOverlay>>,
    running: Arc<Mutex<bool>>,
    tokio_runtime: Arc<tokio::runtime::Runtime>,
}

impl AiChatBridge {
    pub fn new(ai: AiEngine, overlay: ChatOverlay) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        Self {
            ai: Arc::new(ai),
            overlay: Arc::new(Mutex::new(overlay)),
            running: Arc::new(Mutex::new(false)),
            tokio_runtime: Arc::new(runtime),
        }
    }

    pub fn start(&self) -> Result<(), String> {
        *self.running.lock().unwrap() = true;
        println!("[AI Chat] Bridge started");
        Ok(())
    }

    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    pub fn send_message(&self, text: &str) {
        let ai = self.ai.clone();
        let overlay = self.overlay.clone();
        let runtime = self.tokio_runtime.clone();
        let prompt = text.to_string();

        thread::spawn(move || {
            // Add user message
            {
                let mut ov = overlay.lock().unwrap();
                ov.add_message(MessageRole::User, &prompt);
            }

            // Build request
            let request = AiRequest {
                prompt: prompt.clone(),
                system_prompt: Some(format!(
                    "You are ScreenBuddy, a friendly AI desktop companion. \
                     Keep responses brief (1-3 sentences), helpful, and playful. \
                     Use emoji occasionally. Current time: {}",
                    chrono::Local::now().format("%H:%M")
                )),
                max_tokens: Some(512),
                temperature: Some(0.8),
                stream: false,
                force_tier: None,
                history: None,
                tools: Vec::new(),
            };

            // Call AI using tokio runtime
            let result = runtime.block_on(async {
                ai.generate(request).await
            });

            // Process response
            let response_text = match result {
                Ok(resp) => {
                    println!("[AI] Response ({} tokens, tier: {:?}): {}", resp.tokens_used, resp.model_tier, resp.text);
                    resp.text
                }
                Err(e) => {
                    eprintln!("[AI] Error: {}", e);
                    // Fallback to simple response
                    ai.simple(&prompt)
                }
            };

            // Add AI response
            {
                let mut ov = overlay.lock().unwrap();
                ov.add_message(MessageRole::Assistant, &response_text);
            }

            println!("[AI Chat] AI: {}", response_text);
        });
    }

    pub fn overlay(&self) -> &Arc<Mutex<ChatOverlay>> {
        &self.overlay
    }

    pub fn ai(&self) -> &Arc<AiEngine> {
        &self.ai
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

impl Clone for AiChatBridge {
    fn clone(&self) -> Self {
        Self {
            ai: self.ai.clone(),
            overlay: self.overlay.clone(),
            running: self.running.clone(),
            tokio_runtime: self.tokio_runtime.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_bridge_creation() {
        let config = crate::ai::AiConfig::default();
        let expected = AiEngine::new(config.clone()).available_models().len();
        let ai = AiEngine::new(config);
        let overlay = ChatOverlay::new();
        let bridge = AiChatBridge::new(ai, overlay);
        // Default config has local endpoints configured, so at least one model
        // must be present. (Previously `>= 0`, which is vacuously true for usize.)
        assert!(expected > 0, "default config should expose local models");
        assert_eq!(bridge.ai().available_models().len(), expected);
    }

    #[test]
    fn test_ai_bridge_start_stop() {
        let ai = AiEngine::new(crate::ai::AiConfig::default());
        let overlay = ChatOverlay::new();
        let bridge = AiChatBridge::new(ai, overlay);
        bridge.start().unwrap();
        assert!(bridge.is_running());
        bridge.stop();
        assert!(!bridge.is_running());
    }
}
