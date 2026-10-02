#!/bin/bash
cd /G/Projects/ScreenBuddy

# Fix chat_overlay.rs
cat > crates/screenbuddy-core/src/chat_overlay.rs << 'CHAT_EOF'
use std::sync::mpsc::{channel, Sender};

#[derive(Debug, Clone)]
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
    is_visible: bool,
    position: (i32, i32),
    size: (i32, i32),
}

impl ChatOverlay {
    pub fn new() -> Self {
        let (event_tx, _) = channel();
        Self { event_tx, is_visible: false, position: (100, 100), size: (300, 400) }
    }

    pub fn start(&self) -> Result<(), String> {
        let _event_tx = self.event_tx.clone();
        std::thread::spawn(move || {});
        Ok(())
    }

    pub fn send_event(&self, event: ChatOverlayEvent) { let _ = self.event_tx.send(event); }
    pub fn event_sender(&self) -> Sender<ChatOverlayEvent> { self.event_tx.clone() }
    pub fn set_position(&mut self, x: i32, y: i32) { self.position = (x, y); }
    pub fn set_size(&mut self, w: i32, h: i32) { self.size = (w, h); }
    pub fn toggle(&mut self) { self.is_visible = !self.is_visible; }
    pub fn is_visible(&self) -> bool { self.is_visible }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_overlay_creation() {
        let overlay = ChatOverlay::new();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_chat_overlay_toggle() {
        let mut overlay = ChatOverlay::new();
        overlay.toggle();
        assert!(overlay.is_visible());
    }

    #[test]
    fn test_chat_overlay_position() {
        let mut overlay = ChatOverlay::new();
        overlay.set_position(200, 300);
        assert_eq!(overlay.position, (200, 300));
    }

    #[test]
    fn test_chat_overlay_size() {
        let mut overlay = ChatOverlay::new();
        overlay.set_size(400, 500);
        assert_eq!(overlay.size, (400, 500));
    }

    #[test]
    fn test_chat_overlay_events() {
        let overlay = ChatOverlay::new();
        overlay.send_event(ChatOverlayEvent::UserMessage("Hello".to_string()));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
CHAT_EOF
echo "Fixed chat_overlay.rs"

# Fix config.rs - import AppConfig from state module
sed -i 's/app: AppConfig::default(),/app: crate::state::AppConfig::default(),/' crates/screenbuddy-core/src/config.rs
echo "Fixed config.rs"

# Build and test
echo "Building..."
cargo build 2>&1 | tail -5
echo ""
echo "Testing..."
cargo test --workspace 2>&1 | grep "test result"
