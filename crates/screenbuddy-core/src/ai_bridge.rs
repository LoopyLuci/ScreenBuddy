//! AI Chat Bridge for ScreenBuddy
//!
//! Bridges the AiEngine with the ChatOverlay for real-time conversation.

use std::sync::{Arc, Mutex};
use std::thread;

use crate::ai::{AiEngine, AiRequest};

/// The model the user has configured, when there is one.
///
/// Read from the engine rather than re-reading the settings store, so there is
/// one place that knows what model is in force.
fn model_override(ai: &AiEngine) -> Option<String> {
    ai.pinned_model()
}
use crate::chat_overlay::{ChatOverlay, MessageRole};

/// AI Chat Bridge - connects AiEngine to ChatOverlay
pub struct AiChatBridge {
    ai: Arc<AiEngine>,
    overlay: Arc<Mutex<ChatOverlay>>,
    running: Arc<Mutex<bool>>,
    tokio_runtime: Arc<tokio::runtime::Runtime>,
    /// Messages a consumer has already taken from the overlay.
    ///
    /// Shared, not per-clone: the render loop holds a clone, so a per-instance
    /// counter would let each clone independently replay the whole overlay.
    consumed: Arc<std::sync::atomic::AtomicUsize>,
    /// A fingerprint of the last consumed message: (role, content).
    ///
    /// Used to recognise a rewritten overlay. Watching the length cannot detect a
    /// clear, because between two drains it can go 1 -> 0 -> 1 and never be
    /// observed shorter.
    last_consumed: Arc<std::sync::Mutex<Option<(MessageRole, String)>>>,
}

impl AiChatBridge {
    /// Build a bridge owning its own engine.
    ///
    /// Prefer [Self::with_engine] where an engine already exists: this wraps its
    /// argument in a fresh Arc, so a caller that also holds the engine ends up
    /// with two of them.
    pub fn new(ai: AiEngine, overlay: ChatOverlay) -> Self {
        Self::with_engine(Arc::new(ai), overlay)
    }

    /// Build a bridge around an already-shared engine.
    ///
    /// Preferred over [Self::new], which wraps its argument in a fresh Arc and
    /// therefore produced a *second* engine: settings applied to one never
    /// reached the other, and the bridge's copy kept the built-in model
    /// catalogue while the configured model was applied to an engine that never
    /// sent a request.
    pub fn with_engine(ai: Arc<AiEngine>, overlay: ChatOverlay) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        Self {
            ai,
            overlay: Arc::new(Mutex::new(overlay)),
            running: Arc::new(Mutex::new(false)),
            tokio_runtime: Arc::new(runtime),
            consumed: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            last_consumed: Arc::new(std::sync::Mutex::new(None)),
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
                // The configured model must actually reach the engine. Passing
                // None here meant every request used a tier-selected catalogue
                // entry instead, so a user's chosen model was ignored and the
                // hardcoded names were used whether or not they existed.
                force_model: model_override(&ai),
            };

            // Call AI using tokio runtime
            let result = runtime.block_on(async { ai.generate(request).await });

            // Process response
            let response_text = match result {
                Ok(resp) => {
                    println!(
                        "[AI] Response ({} tokens, tier: {:?}): {}",
                        resp.tokens_used, resp.model_tier, resp.text
                    );
                    resp.text
                }
                Err(e) => {
                    eprintln!("[AI] Error: {}", e);
                    // Report the failure rather than inventing a reply. This used
                    // to fall back to a keyword-matched canned response, so a 404
                    // surfaced as "Hello! I'm your ScreenBuddy companion. How
                    // can I help?" - the user was told a model had answered when
                    // none had, which hides a broken configuration behind a
                    // conversation that looks fine.
                    format!("I could not reach the AI model: {e}")
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

    /// How many overlay messages a consumer has already taken.
    ///
    /// Tracked here rather than inferred from a consumer's own message count:
    /// the chat window also receives messages by other paths, so using its count
    /// as the offset made the drain step over replies and lose them.
    pub fn consumed(&self) -> usize {
        self.consumed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Take the overlay messages not yet consumed, advancing the cursor.
    pub fn take_unconsumed(&self) -> Vec<(MessageRole, String)> {
        let ov = self.overlay.lock().unwrap();
        // A shrinking overlay means it was cleared. Clamping the cursor to the
        // new length is not enough: after a clear that regrows to the same
        // length, a stale cursor would sit exactly at the end and swallow every
        // new message. Restart from zero instead.
        let consumed = self.consumed.load(std::sync::atomic::Ordering::Relaxed);
        let messages = ov.messages();
        // The cursor is only trustworthy if the message it stopped after is
        // still there. If it is not, the overlay was cleared or rewritten.
        let boundary_ok = match (consumed, self.last_consumed.lock().unwrap().as_ref()) {
            (0, _) => true,
            (n, Some((role, text))) => messages
                .get(n - 1)
                .is_some_and(|m| m.role == *role && m.content == *text),
            _ => false,
        };
        let start = if boundary_ok { consumed } else { 0 };
        let messages = ov.messages();
        let fresh: Vec<(MessageRole, String)> = messages[start..]
            .iter()
            .map(|m| (m.role, m.content.clone()))
            .collect();
        self.consumed
            .store(messages.len(), std::sync::atomic::Ordering::Relaxed);
        *self.last_consumed.lock().unwrap() = messages.last().map(|m| (m.role, m.content.clone()));
        fresh
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
            consumed: self.consumed.clone(),
            last_consumed: self.last_consumed.clone(),
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn taking_unconsumed_does_not_skip_a_reply_after_a_user_turn() {
        // Regression: the drain offset came from the chat window's message
        // count, which ran ahead of the overlay because the window also receives
        // messages by other paths. The assistant reply following a user turn was
        // then skipped and never persisted, so a conversation lost every answer.
        let bridge = AiChatBridge::new(
            AiEngine::new(crate::ai::AiConfig::default()),
            ChatOverlay::new(),
        );
        {
            let mut ov = bridge.overlay().lock().unwrap();
            ov.add_message(MessageRole::User, "hello");
            ov.add_message(MessageRole::Assistant, "hi there");
        }

        let first = bridge.take_unconsumed();
        assert_eq!(first.len(), 2, "both turns should be taken the first time");

        // A second drain with nothing new must not repeat them.
        assert!(
            bridge.take_unconsumed().is_empty(),
            "already-consumed turns should not repeat"
        );

        // And a later reply must still arrive rather than being skipped past.
        {
            let mut ov = bridge.overlay().lock().unwrap();
            ov.add_message(MessageRole::User, "again");
            ov.add_message(MessageRole::Assistant, "still here");
        }
        let later = bridge.take_unconsumed();
        assert_eq!(
            later.len(),
            2,
            "a reply after a further user turn must be taken"
        );
        assert_eq!(later[1].1, "still here");
    }

    #[test]
    fn taking_unconsumed_recovers_if_the_overlay_was_cleared() {
        let bridge = AiChatBridge::new(
            AiEngine::new(crate::ai::AiConfig::default()),
            ChatOverlay::new(),
        );
        {
            let mut ov = bridge.overlay().lock().unwrap();
            ov.add_message(MessageRole::User, "one");
        }
        assert_eq!(bridge.take_unconsumed().len(), 1);

        bridge.overlay().lock().unwrap().clear();
        {
            let mut ov = bridge.overlay().lock().unwrap();
            ov.add_message(MessageRole::User, "after clear");
        }
        // The cursor pointed past a shortened overlay; it must clamp, not skip
        // the new message entirely.
        let after = bridge.take_unconsumed();
        assert_eq!(
            after.len(),
            1,
            "a cleared overlay must not swallow later messages"
        );
        assert_eq!(after[0].1, "after clear");
    }

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
