use std::sync::Arc;

use crate::render::texture::Texture;

/// A single sprite instance
#[derive(Debug, Clone)]
pub struct Sprite {
    pub texture_id: u64,
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub source_rect: Option<[f32; 4]>,
}

impl Sprite {
    pub fn new(texture_id: u64, position: [f32; 2]) -> Self {
        Self {
            texture_id,
            position,
            scale: [1.0, 1.0],
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            source_rect: None,
        }
    }

    pub fn with_scale(mut self, scale: [f32; 2]) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_source_rect(mut self, rect: [f32; 4]) -> Self {
        self.source_rect = Some(rect);
        self
    }
}

/// Batches multiple sprites for efficient rendering
///
/// The wgpu handles are held for the submit path, which is not implemented yet.
#[allow(dead_code)]
pub struct SpriteBatch {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    format: wgpu::TextureFormat,
    sprites: Vec<Sprite>,
}

impl SpriteBatch {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            device,
            queue,
            format,
            sprites: Vec::new(),
        }
    }

    pub fn add(&mut self, texture: &Texture, position: [f32; 2], scale: [f32; 2]) {
        let sprite = Sprite::new(texture.id, position).with_scale(scale);
        self.sprites.push(sprite);
    }

    pub fn clear(&mut self) {
        self.sprites.clear();
    }

    pub fn flush(&mut self) {
        // TODO: Implement actual batch rendering
        // For now, just clear the batch
        self.sprites.clear();
    }

    pub fn sprite_count(&self) -> usize {
        self.sprites.len()
    }
}
