//! System Integration for ScreenBuddy
//!
//! Monitors system events and allows the companion to react.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Type of system event
#[derive(Debug, Clone, PartialEq)]
pub enum SystemEventType {
    Idle,
    Active,
    Locked,
    Unlocked,
    Notification,
    BatteryLow,
    HeadphonesConnected,
    HeadphonesDisconnected,
    ProcessStarted(String),
    ProcessStopped(String),
    WindowChanged(String),
    Custom(String),
}

/// A system event
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub event_type: SystemEventType,
    pub timestamp: Instant,
    pub metadata: HashMap<String, String>,
}

impl SystemEvent {
    pub fn new(event_type: SystemEventType) -> Self {
        Self {
            event_type,
            timestamp: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// System integration manager
pub struct SystemIntegration {
    enabled: bool,
    idle_threshold: Duration,
    last_activity: Instant,
    is_idle: bool,
    event_listeners: Arc<RwLock<Vec<Arc<dyn Fn(SystemEvent) + Send + Sync>>>>,
}

impl SystemIntegration {
    pub fn new() -> Self {
        Self {
            enabled: true,
            idle_threshold: Duration::from_secs(300), // 5 minutes
            last_activity: Instant::now(),
            is_idle: false,
            event_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn set_idle_threshold(&mut self, threshold: Duration) {
        self.idle_threshold = threshold;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_idle(&self) -> bool {
        self.is_idle
    }

    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    pub fn register_listener<F>(&mut self, listener: F)
    where
        F: Fn(SystemEvent) + Send + Sync + 'static,
    {
        self.event_listeners.write().unwrap().push(Arc::new(listener));
    }

    pub fn clear_listeners(&mut self) {
        self.event_listeners.write().unwrap().clear();
    }

    /// Update activity state (should be called regularly)
    pub fn tick(&mut self) -> Option<SystemEvent> {
        if !self.enabled {
            return None;
        }

        let now = Instant::now();
        let elapsed = now - self.last_activity;

        if elapsed > self.idle_threshold && !self.is_idle {
            self.is_idle = true;
            let event = SystemEvent::new(SystemEventType::Idle)
                .with_metadata("duration_secs", &elapsed.as_secs().to_string());
            self.fire_event(event.clone());
            Some(event)
        } else {
            None
        }
    }

    /// Call when user activity is detected
    pub fn report_activity(&mut self) {
        self.last_activity = Instant::now();
        if self.is_idle {
            self.is_idle = false;
            let event = SystemEvent::new(SystemEventType::Active);
            self.fire_event(event);
        }
    }

    /// Simulate a notification
    pub fn notify(&self, title: &str, message: &str) -> SystemEvent {
        let event = SystemEvent::new(SystemEventType::Notification)
            .with_metadata("title", title)
            .with_metadata("message", message);
        self.fire_event(event.clone());
        event
    }

    fn fire_event(&self, event: SystemEvent) {
        for listener in self.event_listeners.read().unwrap().iter() {
            listener(event.clone());
        }
    }

    /// Get system info (placeholder for actual system queries)
    pub fn get_system_info(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();
        info.insert("os".to_string(), std::env::consts::OS.to_string());
        info.insert("arch".to_string(), std::env::consts::ARCH.to_string());
        info.insert("idle".to_string(), self.is_idle.to_string());
        info
    }
}

/// Monitor system for events (runs in separate thread)
pub struct SystemMonitor {
    integration: Arc<Mutex<SystemIntegration>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl SystemMonitor {
    pub fn new(integration: Arc<Mutex<SystemIntegration>>) -> Self {
        Self {
            integration,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn start(&self) {
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Run monitoring loop (should be called from a thread)
    pub fn run(&self) {
        while self.running() {
            // In a real implementation, this would:
            // - Check for idle time
            // - Monitor active window
            // - Listen for system events
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_integration_new() {
        let si = SystemIntegration::new();
        assert!(si.enabled());
        assert!(!si.is_idle());
    }

    #[test]
    fn test_system_integration_disable() {
        let mut si = SystemIntegration::new();
        si.set_enabled(false);
        assert!(!si.enabled());
    }

    #[test]
    fn test_idle_detection() {
        let mut si = SystemIntegration::new();
        si.set_idle_threshold(Duration::from_millis(100));
        // Should not be idle immediately
        assert!(!si.is_idle());
    }

    #[test]
    fn test_activity_report() {
        let mut si = SystemIntegration::new();
        si.is_idle = true;
        si.report_activity();
        assert!(!si.is_idle());
    }

    #[test]
    fn test_notify() {
        let si = SystemIntegration::new();
        let event = si.notify("Test", "Hello");
        assert_eq!(event.event_type, SystemEventType::Notification);
    }

    #[test]
    fn test_system_info() {
        let si = SystemIntegration::new();
        let info = si.get_system_info();
        assert!(info.contains_key("os"));
        assert!(info.contains_key("arch"));
    }

    #[test]
    fn test_monitor_start_stop() {
        let si = Arc::new(Mutex::new(SystemIntegration::new()));
        let monitor = SystemMonitor::new(si);
        monitor.start();
        assert!(monitor.running());
        monitor.stop();
        assert!(!monitor.running());
    }

    #[test]
    fn test_idle_threshold() {
        let mut si = SystemIntegration::new();
        si.set_idle_threshold(Duration::from_secs(60));
        // Just check it doesn't panic
    }

    #[test]
    fn test_listener() {
        let mut si = SystemIntegration::new();
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called2 = called.clone();
        si.register_listener(move |_event| {
            called2.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        si.notify("test", "msg");
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
