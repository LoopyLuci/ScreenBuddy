use image::DynamicImage;
use std::path::Path;

pub struct SpriteAnimator {
    frames: usize,
    fps: f64,
    current_frame: usize,
    accumulator: f64,
}

impl SpriteAnimator {
    pub fn new(frames: usize, fps: f64) -> Self {
        Self {
            frames,
            fps,
            current_frame: 0,
            accumulator: 0.0,
        }
    }

    pub fn update(&mut self, time: f32) {
        let frame_duration = 1.0 / self.fps;
        self.accumulator += time as f64;
        self.current_frame = (self.accumulator / frame_duration) as usize % self.frames;
    }

    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.accumulator = 0.0;
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame
    }
}
