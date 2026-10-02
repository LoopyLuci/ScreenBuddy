use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatOverlayEvent {
    UserMessage(String),
    AiResponse(String),
    Toggle,
    MoveTo(i32, i32),
    Show,
    Hide,
    Shutdown,
}

pub struct ChatOverlay {
    event_tx: Sender<ChatOverlayEvent>,
    event_rx: Arc<Mutex<Receiver<ChatOverlayEvent>>>,
    is_visible: bool,
    position: (i32, i32),
    size: (i32, i32),
    messages: Vec<ChatMessage>,
    ai_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

impl Default for ChatOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatOverlay {
    pub fn new() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            is_visible: false,
            position: (100, 100),
            size: (380, 500),
            messages: Vec::new(),
            ai_enabled: true,
        }
    }

    pub fn start(&self) -> Result<(), String> {
        let rx = self.event_rx.clone();
        thread::spawn(move || {
            loop {
                let event = {
                    let lock = rx.lock().unwrap();
                    lock.recv()
                };
                match event {
                    Ok(ChatOverlayEvent::Shutdown) => break,
                    Ok(ChatOverlayEvent::UserMessage(text)) => {
                        // In a full implementation, this would send to AI
                        println!("[Chat] User: {}", text);
                    }
                    Ok(ChatOverlayEvent::AiResponse(text)) => {
                        println!("[Chat] AI: {}", text);
                    }
                    _ => {}
                }
            }
        });
        Ok(())
    }

    pub fn send_event(&self, event: ChatOverlayEvent) {
        let _ = self.event_tx.send(event);
    }

    pub fn event_sender(&self) -> Sender<ChatOverlayEvent> {
        self.event_tx.clone()
    }

    pub fn receiver(&self) -> Arc<Mutex<Receiver<ChatOverlayEvent>>> {
        self.event_rx.clone()
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.position = (x, y);
    }
    pub fn set_size(&mut self, w: i32, h: i32) {
        self.size = (w, h);
    }
    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
    pub fn size(&self) -> (i32, i32) {
        self.size
    }
    pub fn position(&self) -> (i32, i32) {
        self.position
    }

    pub fn add_message(&mut self, role: MessageRole, content: &str) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    pub fn set_ai_enabled(&mut self, enabled: bool) {
        self.ai_enabled = enabled;
    }
    pub fn is_ai_enabled(&self) -> bool {
        self.ai_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_creation() {
        let overlay = ChatOverlay::new();
        assert!(!overlay.is_visible());
        assert_eq!(overlay.size(), (380, 500));
    }

    #[test]
    fn test_overlay_toggle() {
        let mut overlay = ChatOverlay::new();
        overlay.toggle();
        assert!(overlay.is_visible());
        overlay.toggle();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_overlay_messages() {
        let mut overlay = ChatOverlay::new();
        overlay.add_message(MessageRole::User, "Hello");
        overlay.add_message(MessageRole::Assistant, "Hi there!");
        assert_eq!(overlay.messages().len(), 2);
        assert_eq!(overlay.messages()[0].content, "Hello");
    }

    #[test]
    fn test_overlay_clear() {
        let mut overlay = ChatOverlay::new();
        overlay.add_message(MessageRole::User, "test");
        overlay.clear();
        assert!(overlay.messages().is_empty());
    }

    #[test]
    fn test_overlay_ai_enabled() {
        let mut overlay = ChatOverlay::new();
        assert!(overlay.is_ai_enabled());
        overlay.set_ai_enabled(false);
        assert!(!overlay.is_ai_enabled());
    }
}
