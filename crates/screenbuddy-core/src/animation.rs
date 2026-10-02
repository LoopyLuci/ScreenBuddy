//! Animation System for ScreenBuddy
//!
//! Manages sprite sheets and frame-based animations for creature states.
//! Supports: idle, walk, fly, sleep, celebrate, and custom animations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Animation state types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationState {
    Idle,
    Walk,
    Fly,
    Sleep,
    Celebrate,
    Custom(u32),
}

impl std::fmt::Display for AnimationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationState::Idle => write!(f, "idle"),
            AnimationState::Walk => write!(f, "walk"),
            AnimationState::Fly => write!(f, "fly"),
            AnimationState::Sleep => write!(f, "sleep"),
            AnimationState::Celebrate => write!(f, "celebrate"),
            AnimationState::Custom(id) => write!(f, "custom_{}", id),
        }
    }
}

/// A single animation (collection of frames)
#[derive(Debug, Clone)]
pub struct Animation {
    pub state: AnimationState,
    pub frames: Vec<PathBuf>,
    pub current_frame: usize,
    pub fps: f64,
    pub looping: bool,
    pub time_accumulator: f64,
}

impl Animation {
    pub fn new(state: AnimationState, fps: f64, looping: bool) -> Self {
        Self {
            state,
            frames: Vec::new(),
            current_frame: 0,
            fps,
            looping,
            time_accumulator: 0.0,
        }
    }

    /// Add a frame from a file path
    pub fn add_frame(&mut self, path: PathBuf) {
        self.frames.push(path);
    }

    /// Add multiple frames
    pub fn add_frames(&mut self, paths: Vec<PathBuf>) {
        self.frames.extend(paths);
    }

    /// Update the animation
    pub fn update(&mut self, delta_time: f64) {
        if self.frames.is_empty() {
            return;
        }

        self.time_accumulator += delta_time;
        let frame_duration = 1.0 / self.fps;

        while self.time_accumulator >= frame_duration {
            self.time_accumulator -= frame_duration;
            self.current_frame += 1;

            if self.current_frame >= self.frames.len() {
                if self.looping {
                    self.current_frame = 0;
                } else {
                    self.current_frame = self.frames.len() - 1;
                }
            }
        }
    }

    /// Get the current frame path
    pub fn current_frame_path(&self) -> Option<&PathBuf> {
        self.frames.get(self.current_frame)
    }

    /// Get the current frame index
    pub fn frame_index(&self) -> usize {
        self.current_frame
    }

    /// Get the total number of frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Check if the animation has finished (non-looping only)
    pub fn is_finished(&self) -> bool {
        if self.looping {
            return false;
        }
        self.current_frame >= self.frames.len().saturating_sub(1)
    }

    /// Reset to the first frame
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.time_accumulator = 0.0;
    }

    /// Set the FPS
    pub fn set_fps(&mut self, fps: f64) {
        self.fps = fps;
    }
}

/// Animation controller managing multiple animations
pub struct AnimationController {
    animations: HashMap<AnimationState, Animation>,
    current_state: AnimationState,
    default_state: AnimationState,
}

impl AnimationController {
    pub fn new(default_state: AnimationState) -> Self {
        Self {
            animations: HashMap::new(),
            current_state: default_state,
            default_state,
        }
    }

    /// Add an animation
    pub fn add_animation(&mut self, animation: Animation) {
        self.animations.insert(animation.state, animation);
    }

    /// Get an animation by state
    pub fn get(&self, state: AnimationState) -> Option<&Animation> {
        self.animations.get(&state)
    }

    /// Get a mutable animation
    pub fn get_mut(&mut self, state: AnimationState) -> Option<&mut Animation> {
        self.animations.get_mut(&state)
    }

    /// Switch to a different animation state
    pub fn set_state(&mut self, state: AnimationState) -> bool {
        if self.animations.contains_key(&state) {
            if self.current_state != state {
                // Reset the new animation
                if let Some(anim) = self.animations.get_mut(&state) {
                    anim.reset();
                }
                self.current_state = state;
            }
            true
        } else {
            false
        }
    }

    /// Get the current animation state
    pub fn current_state(&self) -> AnimationState {
        self.current_state
    }

    /// The state this controller was constructed with.
    pub fn default_state(&self) -> AnimationState {
        self.default_state
    }

    /// Return to the default state, restarting its animation.
    ///
    /// Unlike `set_state`, this does not require the default animation to be
    /// registered: the controller falls back to an empty frame list rather than
    /// refusing the transition.
    pub fn reset(&mut self) {
        self.current_state = self.default_state;
        if let Some(anim) = self.animations.get_mut(&self.default_state) {
            anim.reset();
        }
    }

    /// Update the current animation
    pub fn update(&mut self, delta_time: f64) {
        if let Some(anim) = self.animations.get_mut(&self.current_state) {
            anim.update(delta_time);
        }
    }

    /// Get the current frame path
    pub fn current_frame_path(&self) -> Option<&PathBuf> {
        self.animations
            .get(&self.current_state)
            .and_then(|anim| anim.current_frame_path())
    }

    /// Get the current frame index
    pub fn current_frame_index(&self) -> usize {
        self.animations
            .get(&self.current_state)
            .map(|anim| anim.frame_index())
            .unwrap_or(0)
    }

    /// Check if the current animation is finished
    pub fn is_current_finished(&self) -> bool {
        self.animations
            .get(&self.current_state)
            .map(|anim| anim.is_finished())
            .unwrap_or(true)
    }

    /// Reset the current animation
    pub fn reset_current(&mut self) {
        if let Some(anim) = self.animations.get_mut(&self.current_state) {
            anim.reset();
        }
    }

    /// Get all available states
    pub fn available_states(&self) -> Vec<AnimationState> {
        self.animations.keys().cloned().collect()
    }

    /// Check if a state has an animation
    pub fn has_state(&self, state: AnimationState) -> bool {
        self.animations.contains_key(&state)
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new(AnimationState::Idle)
    }
}

/// Sprite sheet loader
pub struct SpriteSheetLoader;

impl SpriteSheetLoader {
    /// Load a sprite sheet from a directory
    /// Expects files named: state_0.png, state_1.png, etc.
    pub fn load_from_directory(
        dir: &PathBuf,
        state: AnimationState,
        fps: f64,
        looping: bool,
    ) -> Result<Animation, String> {
        let mut animation = Animation::new(state, fps, looping);

        if dir.exists() {
            let prefix = format!("{}_", state);
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&prefix) && name.ends_with(".png")
                })
                .collect();

            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                animation.add_frame(entry.path());
            }
        }

        Ok(animation)
    }

    /// Load all animations from a directory structure
    pub fn load_all_from_directory(base_dir: &Path) -> Result<Vec<Animation>, String> {
        let mut animations = Vec::new();

        let states = vec![
            (AnimationState::Idle, "idle"),
            (AnimationState::Walk, "walk"),
            (AnimationState::Fly, "fly"),
            (AnimationState::Sleep, "sleep"),
            (AnimationState::Celebrate, "celebrate"),
        ];

        for (state, name) in states {
            let dir = base_dir.join(name);
            if dir.exists() {
                match Self::load_from_directory(&dir, state, 12.0, true) {
                    Ok(anim) => animations.push(anim),
                    Err(e) => eprintln!("Warning: Failed to load animation {}: {}", name, e),
                }
            }
        }

        Ok(animations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_state_display() {
        assert_eq!(format!("{}", AnimationState::Idle), "idle");
        assert_eq!(format!("{}", AnimationState::Walk), "walk");
        assert_eq!(format!("{}", AnimationState::Custom(42)), "custom_42");
    }

    #[test]
    fn test_animation_creation() {
        let anim = Animation::new(AnimationState::Idle, 12.0, true);
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.fps, 12.0);
        assert!(anim.looping);
        assert_eq!(anim.frame_count(), 0);
    }

    #[test]
    fn test_animation_add_frames() {
        let mut anim = Animation::new(AnimationState::Idle, 12.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        assert_eq!(anim.frame_count(), 2);
    }

    #[test]
    fn test_controller_default_state_is_observable() {
        let c = AnimationController::new(AnimationState::Walk);
        assert_eq!(c.current_state(), AnimationState::Walk);
        assert_eq!(c.default_state(), AnimationState::Walk);
    }

    #[test]
    fn test_controller_reset_returns_to_default() {
        let mut c = AnimationController::new(AnimationState::Idle);
        c.add_animation(Animation::new(AnimationState::Idle, 1.0, true));
        c.add_animation(Animation::new(AnimationState::Walk, 1.0, true));
        assert!(c.set_state(AnimationState::Walk));
        assert_eq!(c.current_state(), AnimationState::Walk);

        c.reset();
        assert_eq!(c.current_state(), AnimationState::Idle);
    }

    #[test]
    fn test_controller_reset_restarts_the_animation() {
        let mut c = AnimationController::new(AnimationState::Idle);
        let mut idle = Animation::new(AnimationState::Idle, 1.0, true);
        idle.add_frame(PathBuf::from("f0.png"));
        idle.add_frame(PathBuf::from("f1.png"));
        c.add_animation(idle);

        // Advance off the first frame, then reset and confirm it rewinds.
        c.update(1.5);
        assert_eq!(c.get(AnimationState::Idle).unwrap().frame_index(), 1);
        c.reset();
        assert_eq!(c.get(AnimationState::Idle).unwrap().frame_index(), 0);
    }

    #[test]
    fn test_controller_reset_works_without_default_registered() {
        // set_state refuses unregistered states; reset must not.
        let mut c = AnimationController::new(AnimationState::Fly);
        assert!(!c.set_state(AnimationState::Walk));
        c.reset();
        assert_eq!(c.current_state(), AnimationState::Fly);
    }

    #[test]
    fn test_animation_update() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true); // 1 FPS
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));

        anim.update(0.5);
        assert_eq!(anim.frame_index(), 0);

        anim.update(0.6); // Total 1.1s, should advance
        assert_eq!(anim.frame_index(), 1);
    }

    #[test]
    fn test_animation_looping() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));

        anim.update(3.0); // 3 seconds at 1 FPS = 3 frames, should loop
        assert_eq!(anim.frame_index(), 1); // 3 % 2 = 1
    }

    #[test]
    fn test_animation_non_looping() {
        let mut anim = Animation::new(AnimationState::Celebrate, 1.0, false);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));

        anim.update(5.0); // Way past end
        assert_eq!(anim.frame_index(), 1); // Stays at last frame
        assert!(anim.is_finished());
    }

    #[test]
    fn test_animation_reset() {
        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));

        anim.update(1.5);
        assert_eq!(anim.frame_index(), 1);

        anim.reset();
        assert_eq!(anim.frame_index(), 0);
    }

    #[test]
    fn test_animation_controller() {
        let mut controller = AnimationController::new(AnimationState::Idle);

        let mut idle_anim = Animation::new(AnimationState::Idle, 12.0, true);
        idle_anim.add_frame(PathBuf::from("idle_0.png"));
        controller.add_animation(idle_anim);

        let mut walk_anim = Animation::new(AnimationState::Walk, 12.0, true);
        walk_anim.add_frame(PathBuf::from("walk_0.png"));
        controller.add_animation(walk_anim);

        assert!(controller.has_state(AnimationState::Idle));
        assert!(controller.has_state(AnimationState::Walk));
        assert!(!controller.has_state(AnimationState::Fly));

        assert!(controller.set_state(AnimationState::Walk));
        assert_eq!(controller.current_state(), AnimationState::Walk);
    }

    #[test]
    fn test_animation_controller_update() {
        let mut controller = AnimationController::new(AnimationState::Idle);

        let mut anim = Animation::new(AnimationState::Idle, 1.0, true);
        anim.add_frame(PathBuf::from("frame_0.png"));
        anim.add_frame(PathBuf::from("frame_1.png"));
        controller.add_animation(anim);

        controller.update(1.5);
        assert_eq!(controller.current_frame_index(), 1);
    }

    #[test]
    fn test_animation_controller_states() {
        let mut controller = AnimationController::new(AnimationState::Idle);

        let idle = Animation::new(AnimationState::Idle, 12.0, true);
        controller.add_animation(idle);

        let states = controller.available_states();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0], AnimationState::Idle);
    }

    #[test]
    fn test_sprite_sheet_loader() {
        // This test just verifies the loader doesn't panic on missing dir
        let result = SpriteSheetLoader::load_from_directory(
            &PathBuf::from("/nonexistent"),
            AnimationState::Idle,
            12.0,
            true,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().frame_count(), 0);
    }
}
