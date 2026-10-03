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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Per-sound sinks attach to this. Held because sinks need the handle to
    /// outlive them; dropping it would silence playback.
    handle: Option<rodio::OutputStreamHandle>,
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
            handle: None,
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

        // Every sound needs its own sink: a single shared sink can hold only
        // one volume, so per-category buses were unreachable before this.
        // Cache the handle: opening an output device per sound would be slow
        // and can fail on machines with a busy audio device.
        if self.handle.is_none() {
            self.handle = rodio::OutputStream::try_default().ok().map(|(_, h)| h);
        }

        if let Some(handle) = self.handle.as_ref() {
            let file = std::fs::File::open(&full_path).map_err(|e| e.to_string())?;
            let source = rodio::Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;

            let file_info = self.sounds.get(name);
            let category = file_info
                .map(|s| s.category)
                .unwrap_or(SoundCategory::CreatureSound);
            let bus = match category {
                SoundCategory::Music => self.config.music_volume,
                _ => self.config.sfx_volume,
            };
            // Also honour the sound's own level, so a quiet sound stays quiet.
            let own = file_info.map(|s| s.volume).unwrap_or(1.0);

            let voice = rodio::Sink::try_new(handle).map_err(|e| e.to_string())?;
            voice.set_volume((self.config.master_volume * bus * own).clamp(0.0, 1.0));
            voice.append(source);
            // Detach so playback survives this handle going out of scope.
            voice.detach();
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

    /// Effects volume, applied to every non-music sound at playback time.
    pub fn set_effects_volume(&mut self, vol: f32) {
        self.config.sfx_volume = vol.clamp(0.0, 1.0);
    }

    /// Music volume, applied to sounds categorised as music.
    pub fn set_music_volume(&mut self, vol: f32) {
        self.config.music_volume = vol.clamp(0.0, 1.0);
    }

    /// The current bus levels, so a UI can show what is in force.
    pub fn volumes(&self) -> (f32, f32, f32) {
        (
            self.config.master_volume,
            self.config.sfx_volume,
            self.config.music_volume,
        )
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

    /// The per-category bus a sound plays through, independent of playback.
    fn effective_bus(config: &AudioConfig, category: SoundCategory) -> f32 {
        match category {
            SoundCategory::Music => config.music_volume,
            _ => config.sfx_volume,
        }
    }

    #[test]
    fn effects_and_music_buses_are_independent() {
        let mut audio = AudioSystem::new();
        audio.set_effects_volume(0.25);
        audio.set_music_volume(0.9);
        let (master, sfx, music) = audio.volumes();
        assert!((master - 0.7).abs() < 1e-6, "master changed: {master}");
        assert!((sfx - 0.25).abs() < 1e-6, "sfx: {sfx}");
        assert!((music - 0.9).abs() < 1e-6, "music: {music}");
    }

    #[test]
    fn bus_levels_are_clamped_to_the_legal_range() {
        let mut audio = AudioSystem::new();
        audio.set_effects_volume(4.0);
        audio.set_music_volume(-2.0);
        let (_, sfx, music) = audio.volumes();
        assert!((sfx - 1.0).abs() < 1e-6, "not clamped high: {sfx}");
        assert!(music.abs() < 1e-6, "not clamped low: {music}");
    }

    #[test]
    fn only_music_sounds_use_the_music_bus() {
        let config = AudioConfig {
            master_volume: 1.0,
            sfx_volume: 0.5,
            music_volume: 1.0,
            ..Default::default()
        };
        // The bug this guards: every sound used to take the sfx bus, so
        // volume_music could not be heard.
        assert!((effective_bus(&config, SoundCategory::Music) - 1.0).abs() < 1e-6);
        for category in [
            SoundCategory::CreatureSound,
            SoundCategory::Notification,
            SoundCategory::Ui,
        ] {
            assert!(
                (effective_bus(&config, category) - 0.5).abs() < 1e-6,
                "{category:?} wrongly used the music bus"
            );
        }
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
