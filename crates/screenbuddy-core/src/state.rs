use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::creature::{Creature, CreatureStateDef};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Walking,
    Flying,
    Sleeping,
    Celebrating,
    Custom(String),
}

impl State {
    pub fn name(&self) -> String {
        match self {
            State::Idle => "idle".into(),
            State::Walking => "walk".into(),
            State::Flying => "fly".into(),
            State::Sleeping => "sleep".into(),
            State::Celebrating => "celebrate".into(),
            State::Custom(s) => s.clone(),
        }
    }
    pub fn animation_name(&self) -> String {
        self.name()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureState {
    pub id: String,
    pub position: Vec2,
    pub velocity: Vec2,
    pub current_state: String,
    pub current_animation: String,
    pub animation_time: f32,
    pub energy: f32,
    pub metadata: HashMap<String, String>,
}

impl Default for CreatureState {
    fn default() -> Self {
        Self {
            id: String::new(),
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            current_state: "idle".into(),
            current_animation: "idle".into(),
            animation_time: 0.0,
            energy: 1.0,
            metadata: HashMap::new(),
        }
    }
}

pub struct CreatureStateMachine {
    states: HashMap<String, CreatureStateDef>,
    current_state: State,
    previous_state: State,
    time_in_state: Duration,
    idle_timer: f32,
    position: Vec2,
    velocity: Vec2,
    bob_timer: f32,
}

#[derive(Debug, Clone)]
pub enum StateChangeEvent {
    Entered(State),
    Left(State),
    AnimationChanged(String),
    PositionChanged(Vec2),
}

impl CreatureStateMachine {
    pub fn new(creature: &Creature) -> Self {
        let current = State::Idle;
        Self {
            states: creature.behavior.states.clone().unwrap_or_default(),
            current_state: current.clone(),
            previous_state: current,
            time_in_state: Duration::ZERO,
            idle_timer: 0.0,
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            bob_timer: 0.0,
        }
    }

    pub fn current_state(&self) -> &State {
        &self.current_state
    }
    pub fn position(&self) -> Vec2 {
        self.position
    }
    pub fn time_in_state(&self) -> Duration {
        self.time_in_state
    }
    pub fn is_animating(&self) -> bool {
        !matches!(self.current_state, State::Sleeping | State::Idle)
    }
    pub fn current_animation(&self) -> String {
        self.current_state.animation_name()
    }

    pub fn update(&mut self, dt: f32) -> Vec<StateChangeEvent> {
        let mut events = Vec::new();
        self.time_in_state += Duration::from_secs_f32(dt);
        self.bob_timer += dt;
        self.idle_timer += dt;

        match &self.current_state {
            State::Idle => {
                self.position.x += (self.bob_timer * 2.0).sin() * 0.1;
            }
            State::Walking => {
                self.position.x += self.velocity.x * dt;
                self.position.y += (self.bob_timer * 4.0).sin() * 0.5;
                if self.position.x > 500.0 {
                    self.velocity.x = -self.velocity.x.abs();
                }
                if self.position.x < 0.0 {
                    self.velocity.x = self.velocity.x.abs();
                }
            }
            State::Flying => {
                self.velocity.y = (self.bob_timer * 1.5).sin() * 20.0;
                self.position.x += self.velocity.x * dt;
                self.position.y += self.velocity.y * dt;
                self.position.y = self.position.y.clamp(50.0, 600.0);
            }
            State::Celebrating => {
                self.position.y += (self.bob_timer * 8.0).abs().sin() * 2.0;
            }
            _ => {}
        }

        let idle_timeout = self
            .states
            .get("idle")
            .and_then(|s| s.auto_transition.as_ref())
            .and_then(|t| t.after_seconds)
            .unwrap_or(15.0);
        if self.current_state == State::Idle && self.idle_timer >= idle_timeout {
            self.idle_timer = 0.0;
            self.change_state(State::Walking, &mut events);
            self.velocity.x = 30.0;
        }
        let walk_timeout = self
            .states
            .get("walk")
            .and_then(|s| s.auto_transition.as_ref())
            .and_then(|t| t.after_seconds)
            .unwrap_or(30.0);
        if self.current_state == State::Walking && self.time_in_state.as_secs_f32() >= walk_timeout
        {
            self.change_state(State::Idle, &mut events);
        }
        let celebrate_timeout = self
            .states
            .get("celebrate")
            .and_then(|s| s.auto_transition.as_ref())
            .and_then(|t| t.after_seconds)
            .unwrap_or(3.0);
        if self.current_state == State::Celebrating
            && self.time_in_state.as_secs_f32() >= celebrate_timeout
        {
            self.change_state(State::Idle, &mut events);
        }
        events
    }

    pub fn change_state(&mut self, new: State, events: &mut Vec<StateChangeEvent>) {
        if self.current_state != new {
            events.push(StateChangeEvent::Left(self.current_state.clone()));
            self.previous_state = self.current_state.clone();
            self.current_state = new.clone();
            self.time_in_state = Duration::ZERO;
            self.idle_timer = 0.0;
            events.push(StateChangeEvent::Entered(new.clone()));
            events.push(StateChangeEvent::AnimationChanged(new.animation_name()));
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    pub creatures: HashMap<String, CreatureState>,
    pub active_creature: Option<String>,
    pub config: AppConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub transparent: bool,
    pub always_on_top: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            creatures: HashMap::new(),
            active_creature: None,
            config: AppConfig {
                window_width: 256,
                window_height: 256,
                transparent: true,
                always_on_top: true,
            },
        }
    }
}

impl AppState {
    pub fn add_creature(&mut self, id: String, state: CreatureState) {
        self.creatures.insert(id, state);
    }
    pub fn get_creature(&self, id: &str) -> Option<&CreatureState> {
        self.creatures.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_creature() -> Creature {
        use crate::creature::{AnimationFormat, Behavior};
        Creature {
            id: "test".into(),
            name: "Test".into(),
            category: "test".into(),
            rarity: None,
            description: None,
            version: None,
            author: None,
            tags: None,
            visual: crate::creature::Visual {
                animation_format: AnimationFormat::SpriteSheet,
                texture: "test.png".into(),
                atlas: None,
                skeleton: None,
                default_scale: 1.0,
                idle_animation: "idle".into(),
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
                default_state: "idle".into(),
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
        }
    }

    #[test]
    fn test_state_machine_creation() {
        let sm = CreatureStateMachine::new(&test_creature());
        assert_eq!(*sm.current_state(), State::Idle);
    }

    #[test]
    fn test_state_change() {
        let mut sm = CreatureStateMachine::new(&test_creature());
        let mut events = Vec::new();
        sm.change_state(State::Walking, &mut events);
        assert_eq!(*sm.current_state(), State::Walking);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_auto_transition() {
        let mut sm = CreatureStateMachine::new(&test_creature());
        for _ in 0..960 {
            let events = sm.update(1.0 / 60.0);
            if !events.is_empty() {
                break;
            }
        }
        assert_eq!(*sm.current_state(), State::Walking);
    }
}
