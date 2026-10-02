//! Text-to-Speech for ScreenBuddy
//!
//! Makes the creature speak responses aloud.

use std::sync::{Arc, Mutex};

/// TTS engine state
#[derive(Debug, Clone)]
pub enum TtsEngine {
    /// No TTS available
    None,
    /// Windows SAPI
    #[cfg(windows)]
    Sap,
    /// espeak-ng
    Espeak,
    /// Custom command
    Command(String),
}

/// TTS configuration
#[derive(Debug, Clone)]
pub struct TtsConfig {
    pub enabled: bool,
    pub engine: TtsEngine,
    pub volume: f32,
    pub rate: f32,
    pub voice: Option<String>,
    pub pitch: f32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: TtsEngine::None,
            volume: 0.8,
            rate: 1.0,
            voice: None,
            pitch: 1.0,
        }
    }
}

/// Text-to-Speech system
#[derive(Debug, Clone)]
pub struct TtsSystem {
    config: TtsConfig,
    is_speaking: Arc<Mutex<bool>>,
}

impl TtsSystem {
    pub fn new() -> Self {
        Self {
            config: TtsConfig::default(),
            is_speaking: Arc::new(Mutex::new(false)),
        }
    }

    pub fn with_config(mut self, config: TtsConfig) -> Self {
        self.config = config;
        self
    }

    pub fn configure(&mut self, config: TtsConfig) {
        self.config = config;
    }

    pub fn is_available(&self) -> bool {
        match self.config.engine {
            TtsEngine::None => false,
            _ => true,
        }
    }

    pub fn is_speaking(&self) -> bool {
        *self.is_speaking.lock().unwrap()
    }

    /// Speak text (non-blocking)
    pub fn speak(&self, text: &str) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        if text.is_empty() {
            return Ok(());
        }

        let text = text.to_string();
        let engine = self.config.engine.clone();
        let volume = self.config.volume;
        let rate = self.config.rate;
        let is_speaking = self.is_speaking.clone();

        // Mark as speaking
        *is_speaking.lock().unwrap() = true;

        std::thread::spawn(move || {
            match engine {
                TtsEngine::None => {}
                #[cfg(windows)]
                TtsEngine::Sap => {
                    // Use PowerShell TTS (works on Windows without extra deps)
                    let _ = std::process::Command::new("powershell")
                        .args(&[
                            "-Command",
                            &format!(
                                "Add-Type -AssemblyName System.Speech; $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; $synth.Speak('{}');",
                                text.replace("'", "''")
                            ),
                        ])
                        .output();
                }
                TtsEngine::Espeak => {
                    let _ = std::process::Command::new("espeak")
                        .args(&[&format!("--volume={}", (volume * 200.0) as u32), &text])
                        .output();
                }
                TtsEngine::Command(cmd) => {
                    let _ = std::process::Command::new(&cmd)
                        .arg(&text)
                        .output();
                }
            }
            *is_speaking.lock().unwrap() = false;
        });

        Ok(())
    }

    /// Stop speaking
    pub fn stop(&self) {
        *self.is_speaking.lock().unwrap() = false;
        // Could send kill signal to TTS process
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.config.volume = vol.clamp(0.0, 1.0);
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.config.rate = rate.clamp(0.5, 3.0);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    pub fn config(&self) -> &TtsConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_new() {
        let tts = TtsSystem::new();
        assert!(!tts.is_available());
    }

    #[test]
    fn test_tts_enable() {
        let mut tts = TtsSystem::new();
        tts.set_enabled(true);
        assert!(tts.config().enabled);
    }

    #[test]
    fn test_tts_volume() {
        let mut tts = TtsSystem::new();
        tts.set_volume(0.5);
        assert_eq!(tts.config().volume, 0.5);
    }

    #[test]
    fn test_tts_volume_clamp() {
        let mut tts = TtsSystem::new();
        tts.set_volume(2.0);
        assert_eq!(tts.config().volume, 1.0);
    }

    #[test]
    fn test_tts_rate() {
        let mut tts = TtsSystem::new();
        tts.set_rate(2.0);
        assert_eq!(tts.config().rate, 2.0);
    }

    #[test]
    fn test_tts_speak_disabled() {
        let tts = TtsSystem::new();
        assert!(tts.speak("hello").is_ok());
    }

    #[test]
    fn test_tts_speak_empty() {
        let tts = TtsSystem::new();
        assert!(tts.speak("").is_ok());
    }

    #[test]
    fn test_tts_config() {
        let tts = TtsSystem::new();
        assert!(!tts.config().enabled);
        assert_eq!(tts.config().volume, 0.8);
    }
}
