use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

pub type CreatureId = String;

/// Complete creature definition as loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creature {
    pub id: CreatureId,
    pub name: String,
    pub category: String,
    pub rarity: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,

    pub visual: Visual,
    pub audio: Option<Audio>,
    pub behavior: Behavior,
    pub ai: Option<Ai>,
    pub permissions: Option<Permissions>,
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Visual {
    #[serde(rename = "animation_format")]
    pub animation_format: AnimationFormat,
    pub texture: String,
    pub atlas: Option<String>,
    pub skeleton: Option<String>,
    #[serde(default = "default_scale")]
    pub default_scale: f32,
    pub idle_animation: String,
    pub walk_animation: Option<String>,
    pub fly_animation: Option<String>,
    pub action_animations: Option<HashMap<String, String>>,
    pub animation_fps: Option<u32>,
    pub sprite_size: Option<[u32; 2]>,
    pub frames_per_animation: Option<HashMap<String, u32>>,
    pub particle_effects: Option<ParticleEffects>,
    pub color_palette: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationFormat {
    SpriteSheet,
    Spine,
    Live2d,
    Frame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audio {
    pub idle_sound: Option<String>,
    pub click_sound: Option<String>,
    pub notification_sound: Option<String>,
    pub celebration_sound: Option<String>,
    pub error_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Behavior {
    pub script: Option<String>,
    pub default_state: String,
    #[serde(default)]
    pub autonomous_actions: bool,
    pub interaction_radius: Option<f32>,
    pub movement_speed: Option<f32>,
    pub idle_timeout: Option<f32>,
    pub personality: Option<HashMap<String, f32>>,
    pub states: Option<HashMap<String, CreatureStateDef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTransition {
    pub after_seconds: Option<f32>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureStateDef {
    pub description: Option<String>,
    pub animation: Option<String>,
    pub loop_animation: Option<bool>,
    pub movement: Option<String>,
    pub speed_range: Option<Vec<f32>>,
    pub can_transition_to: Option<Vec<String>>,
    pub auto_transition: Option<AutoTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ai {
    pub system_prompt: Option<String>,
    pub voice_id: Option<String>,
    pub specializations: Option<Vec<String>>,
    pub response_style: Option<String>,
    pub max_response_length: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub can_read_screen: bool,
    #[serde(default)]
    pub can_read_clipboard: bool,
    #[serde(default)]
    pub can_access_files: bool,
    pub can_use_tools: Option<Vec<String>>,
    #[serde(default)]
    pub can_send_notifications: bool,
    #[serde(default)]
    pub can_access_microphone: bool,
    #[serde(default)]
    pub can_access_camera: bool,
    #[serde(default)]
    pub can_modify_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleEffects {
    pub trail: Option<String>,
    pub ambient: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub schema_version: Option<String>,
}

fn default_scale() -> f32 {
    1.0
}

/// Validates a creature definition
pub fn validate_creature(creature: &Creature) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if creature.id.is_empty() {
        errors.push("Creature ID cannot be empty".to_string());
    }

    if creature.name.is_empty() {
        errors.push("Creature name cannot be empty".to_string());
    }

    if creature.category.is_empty() {
        errors.push("Creature category cannot be empty".to_string());
    }

    if creature.visual.texture.is_empty() {
        errors.push("Visual texture path cannot be empty".to_string());
    }

    if creature.behavior.default_state.is_empty() {
        errors.push("Behavior default_state cannot be empty".to_string());
    }

    // Validate personality values are in range [0.0, 1.0]
    if let Some(ref personality) = creature.behavior.personality {
        for (trait_name, value) in personality {
            if *value < 0.0 || *value > 1.0 {
                errors.push(format!(
                    "Personality trait '{}' must be between 0.0 and 1.0, got {}",
                    trait_name, value
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(errors))
    }
}

/// Loads a creature from a JSON file
pub fn load_creature_from_file<P: AsRef<Path>>(path: P) -> Result<Creature> {
    let content = std::fs::read_to_string(path.as_ref())?;
    let creature: Creature = serde_json::from_str(&content)?;
    validate_creature(&creature)?;
    Ok(creature)
}

/// Loads all creatures from a directory
pub fn load_creatures_from_dir<P: AsRef<Path>>(dir: P) -> Result<HashMap<CreatureId, Creature>> {
    let mut creatures = HashMap::new();

    for entry in std::fs::read_dir(dir.as_ref())? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match load_creature_from_file(&path) {
                Ok(creature) => {
                    creatures.insert(creature.id.clone(), creature);
                }
                Err(e) => {
                    tracing::warn!("Failed to load creature from {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(creatures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_companion_bird() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../toolkit/godot-project/creatures/companion-bird-01.json"
        );
        if std::path::Path::new(path).exists() {
            let creature = load_creature_from_file(path).unwrap();
            assert_eq!(creature.id, "companion-bird-01");
            assert_eq!(creature.category, "animal");
        }
    }

    #[test]
    fn test_validate_creature_minimal() {
        let creature = Creature {
            id: "test-01".to_string(),
            name: "Test".to_string(),
            category: "test".to_string(),
            rarity: None,
            description: None,
            version: None,
            author: None,
            tags: None,
            visual: Visual {
                animation_format: AnimationFormat::SpriteSheet,
                texture: "test.png".to_string(),
                atlas: None,
                skeleton: None,
                default_scale: 1.0,
                idle_animation: "idle".to_string(),
                walk_animation: None,
                fly_animation: None,
                action_animations: None,
                animation_fps: None,
                sprite_size: None,
                frames_per_animation: None,
                particle_effects: None,
                color_palette: None,
            },
            audio: None,
            behavior: Behavior {
                script: None,
                default_state: "idle".to_string(),
                autonomous_actions: false,
                interaction_radius: None,
                movement_speed: None,
                idle_timeout: None,
                personality: None,
                states: None,
            },
            ai: None,
            permissions: None,
            metadata: None,
        };

        assert!(validate_creature(&creature).is_ok());
    }

    #[test]
    fn test_validate_creature_invalid_personality() {
        let mut personality = HashMap::new();
        personality.insert("curiosity".to_string(), 1.5);

        let creature = Creature {
            id: "test-02".to_string(),
            name: "Test".to_string(),
            category: "test".to_string(),
            rarity: None,
            description: None,
            version: None,
            author: None,
            tags: None,
            visual: Visual {
                animation_format: AnimationFormat::SpriteSheet,
                texture: "test.png".to_string(),
                atlas: None,
                skeleton: None,
                default_scale: 1.0,
                idle_animation: "idle".to_string(),
                walk_animation: None,
                fly_animation: None,
                action_animations: None,
                animation_fps: None,
                sprite_size: None,
                frames_per_animation: None,
                particle_effects: None,
                color_palette: None,
            },
            audio: None,
            behavior: Behavior {
                script: None,
                default_state: "idle".to_string(),
                autonomous_actions: false,
                interaction_radius: None,
                movement_speed: None,
                idle_timeout: None,
                personality: Some(personality),
                states: None,
            },
            ai: None,
            permissions: None,
            metadata: None,
        };

        let result = validate_creature(&creature);
        assert!(result.is_err());
        match result {
            Err(Error::Validation(errors)) => {
                assert!(errors.iter().any(|e| e.contains("curiosity")));
            }
            _ => panic!("Expected validation error"),
        }
    }
}
