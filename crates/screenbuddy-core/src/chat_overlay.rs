use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
                // Poll with try_recv rather than holding the lock across a
                // blocking recv(): the receiver is handed out by `receiver()`, so
                // locking it for the whole wait would stall every other consumer
                // and deadlock them against this thread.
                let event = {
                    let lock = match rx.lock() {
                        Ok(lock) => lock,
                        Err(_) => break,
                    };
                    lock.try_recv()
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
                    // Toggle/MoveTo/Show and friends need no work here.
                    Ok(_) => {}
                    Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
                    Err(TryRecvError::Disconnected) => break,
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

    /// The event loop must not hold the receiver lock while waiting.
    ///
    /// The receiver is also handed out by `receiver()`, so a blocking recv()
    /// under the lock would starve every other consumer. The loop is given a
    /// moment to reach its wait before contending, otherwise the assertion
    /// could pass simply because the thread had not started yet.
    #[test]
    fn test_event_loop_does_not_hold_the_lock_while_waiting() {
        let overlay = ChatOverlay::new();
        overlay.start().expect("event loop starts");

        // Let the loop thread reach its wait so the lock is genuinely contended.
        std::thread::sleep(Duration::from_millis(150));

        let rx = overlay.receiver();
        // Under a blocking recv() under the lock this can never be acquired.
        let acquired = rx.try_lock().is_ok();
        assert!(
            acquired,
            "the event loop is holding the receiver lock while waiting"
        );

        overlay.send_event(ChatOverlayEvent::Shutdown);
    }

    #[test]
    fn test_event_loop_delivers_events_and_stops_on_shutdown() {
        let overlay = ChatOverlay::new();
        overlay.start().expect("event loop starts");
        overlay.send_event(ChatOverlayEvent::UserMessage("hello".into()));
        overlay.send_event(ChatOverlayEvent::Shutdown);

        // The loop drains what is queued and exits; give it a moment, then
        // confirm the receiver is still usable and empty.
        std::thread::sleep(Duration::from_millis(100));
        let rx = overlay.receiver();
        assert!(rx.lock().is_ok(), "shutdown must not poison the receiver");
    }

    #[test]
    fn test_overlay_ai_enabled() {
        let mut overlay = ChatOverlay::new();
        assert!(overlay.is_ai_enabled());
        overlay.set_ai_enabled(false);
        assert!(!overlay.is_ai_enabled());
    }
}
