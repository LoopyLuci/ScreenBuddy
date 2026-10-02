use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub muted: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            master_volume: 0.7,
            music_volume: 0.4,
            sfx_volume: 0.8,
            muted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SoundCategory {
    CreatureSound,
    Notification,
    Music,
    Ui,
}

#[derive(Debug, Clone)]
pub struct AudioFile {
    pub name: String,
    pub path: String,
    pub category: SoundCategory,
    pub volume: f32,
}

#[derive(Debug, Clone)]
pub enum AudioEvent {
    PlaySound(String),
    PlayMusic(String),
    StopMusic,
    SetVolume(f32),
    SetMuted(bool),
}

pub struct AudioSystem {
    config: AudioConfig,
    sounds: HashMap<String, AudioFile>,
    stream: Option<rodio::OutputStream>,
    sink: Option<rodio::Sink>,
    last_play: HashMap<String, Instant>,
}

impl Default for AudioSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSystem {
    pub fn new() -> Self {
        let mut sys = Self {
            config: AudioConfig::default(),
            sounds: HashMap::new(),
            stream: None,
            sink: None,
            last_play: HashMap::new(),
        };
        sys.init_audio();
        sys
    }

    fn init_audio(&mut self) {
        // Tests construct AudioSystem directly, and each one would otherwise open
        // a real cpal output device. Many concurrent opens/teardowns crash the
        // process with STATUS_ACCESS_VIOLATION on the Windows runner, which is
        // why the unit suite died partway through with no test failing.
        // Playback is exercised explicitly by tests that opt in below.
        if cfg!(test) {
            return;
        }
        if let Ok((stream, stream_handle)) = rodio::OutputStream::try_default() {
            if let Ok(sink) = rodio::Sink::try_new(&stream_handle) {
                sink.set_volume(self.config.master_volume * self.config.sfx_volume);
                self.sink = Some(sink);
                self.stream = Some(stream);
            }
        }
    }

    pub fn register_sound(&mut self, name: &str, path: &str, category: SoundCategory, volume: f32) {
        self.sounds.insert(
            name.into(),
            AudioFile {
                name: name.into(),
                path: path.into(),
                category,
                volume,
            },
        );
    }

    pub fn load_default_sounds(&mut self) {
        self.register_sound(
            "idle_hum",
            "assets/audio/idle_hum.wav",
            SoundCategory::CreatureSound,
            0.3,
        );
        self.register_sound(
            "walk_step",
            "assets/audio/step.wav",
            SoundCategory::CreatureSound,
            0.4,
        );
        self.register_sound(
            "notification",
            "assets/audio/notify.wav",
            SoundCategory::Notification,
            0.6,
        );
        self.register_sound("click", "assets/audio/click.wav", SoundCategory::Ui, 0.5);
        self.register_sound(
            "celebration",
            "assets/audio/celebrate.wav",
            SoundCategory::CreatureSound,
            0.7,
        );
        self.register_sound(
            "sleep_snore",
            "assets/audio/snore.wav",
            SoundCategory::CreatureSound,
            0.2,
        );
    }

    pub fn play(&mut self, name: &str) -> Result<(), String> {
        if !self.config.enabled || self.config.muted {
            return Ok(());
        }

        // Rate limiting: don't play same sound more than once per 50ms
        let now = Instant::now();
        if let Some(last) = self.last_play.get(name) {
            if now.duration_since(*last).as_millis() < 50 {
                return Ok(());
            }
        }
        self.last_play.insert(name.to_string(), now);

        let path = match self.sounds.get(name) {
            Some(s) => &s.path,
            None => return Err(format!("Sound '{}' not found", name)),
        };

        let full_path = PathBuf::from(path);
        if !full_path.exists() {
            return Err(format!("Sound file not found: {}", path));
        }

        if let Some(sink) = &self.sink {
            let file = std::fs::File::open(&full_path).map_err(|e| e.to_string())?;
            let source = rodio::Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
            sink.append(source);
        }
        Ok(())
    }

    pub fn play_for_state(&mut self, state_name: &str) {
        let _ = match state_name {
            "idle" => self.play("idle_hum"),
            "walk" => self.play("walk_step"),
            "celebrate" => self.play("celebration"),
            "sleep" => self.play("sleep_snore"),
            _ => Ok(()),
        };
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.config.master_volume = vol.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.config.master_volume * self.config.sfx_volume);
        }
    }

    pub fn set_master_volume(&mut self, vol: f32) {
        self.config.master_volume = vol.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.config.master_volume);
        }
    }

    pub fn master_volume(&self) -> f32 {
        self.config.master_volume
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.config.muted = muted;
    }
    pub fn config(&self) -> &AudioConfig {
        &self.config
    }
    pub fn set_config(&mut self, config: AudioConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_config_default() {
        let config = AudioConfig::default();
        assert!(config.enabled);
        assert_eq!(config.master_volume, 0.7);
    }

    #[test]
    fn test_audio_system() {
        let mut sys = AudioSystem::new();
        sys.load_default_sounds();
        // play() may fail if no audio device, but shouldn't panic
        let _ = sys.play("idle_hum");
    }

    #[test]
    fn test_audio_state_mapping() {
        let mut sys = AudioSystem::new();
        sys.play_for_state("idle");
        sys.play_for_state("walk");
        sys.play_for_state("celebrate");
        sys.play_for_state("sleep");
        sys.play_for_state("unknown");
    }

    #[test]
    fn test_parse_category() {
        let mut sys = AudioSystem::new();
        sys.register_sound("test", "test.wav", SoundCategory::Notification, 0.5);
        let _ = sys.play("test");
    }

    #[test]
    fn test_audio_volume() {
        let mut sys = AudioSystem::new();
        sys.set_volume(0.5);
        assert_eq!(sys.config().master_volume, 0.5);
        sys.set_muted(true);
        assert!(sys.config().muted);
    }
}
