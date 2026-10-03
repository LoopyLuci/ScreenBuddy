//! Screen Physics for ScreenBuddy
use glam::Vec2;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Monitor {
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        (
            self.x,
            self.y,
            self.x + self.width as i32,
            self.y + self.height as i32,
        )
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + self.width as i32 / 2,
            self.y + self.height as i32 / 2,
        )
    }
    pub fn clamp_position(&self, x: i32, y: i32, creature_size: u32) -> (i32, i32) {
        let max_x = self.x + self.width as i32 - creature_size as i32;
        let max_y = self.y + self.height as i32 - creature_size as i32;
        (x.clamp(self.x, max_x), y.clamp(self.y, max_y))
    }
}

#[derive(Debug, Clone)]
pub struct ScreenManager {
    monitors: Vec<Monitor>,
}

impl Default for ScreenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenManager {
    pub fn new() -> Self {
        Self {
            monitors: vec![Monitor {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }],
        }
    }
    pub fn refresh(&mut self) {}
    pub fn monitors(&self) -> &[Monitor] {
        &self.monitors
    }
    pub fn primary(&self) -> Option<&Monitor> {
        self.monitors.first()
    }
    pub fn virtual_screen_bounds(&self) -> (i32, i32, i32, i32) {
        self.monitors.iter().fold((0, 0, 0, 0), |acc, m| {
            (
                acc.0.min(m.x),
                acc.1.min(m.y),
                acc.2.max(m.x + m.width as i32),
                acc.3.max(m.y + m.height as i32),
            )
        })
    }
    pub fn monitor_at(&self, x: i32, y: i32) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.contains(x, y))
    }
    pub fn clamp_to_screen(&self, x: i32, y: i32, creature_size: u32) -> (i32, i32) {
        self.monitors
            .first()
            .map(|m| m.clamp_position(x, y, creature_size))
            .unwrap_or((x, y))
    }
    pub fn random_position(&self, creature_size: u32) -> Option<(i32, i32)> {
        self.monitors.first().map(|m| {
            let mut rng = rand::thread_rng();
            (
                rng.gen_range(m.x..m.x + m.width as i32 - creature_size as i32),
                rng.gen_range(m.y..m.y + m.height as i32 - creature_size as i32),
            )
        })
    }
    pub fn width(&self) -> u32 {
        self.monitors.first().map(|m| m.width).unwrap_or(1920)
    }
    pub fn height(&self) -> u32 {
        self.monitors.first().map(|m| m.height).unwrap_or(1080)
    }
}

#[derive(Debug, Clone)]
pub struct CreaturePhysics {
    pub position: Vec2,
    pub velocity: Vec2,
    pub size: u32,
    pub mass: f32,
    pub wander_timer: f32,
    pub wander_interval: f32,
    pub target: Option<Vec2>,
    pub is_dragged: bool,
    pub drag_offset: Vec2,
    pub personality: CreaturePersonality,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CreaturePersonality {
    Curious,   // Follows cursor
    Shy,       // Flees from cursor
    Lazy,      // Moves slowly
    Energetic, // Moves quickly
    Neutral,   // Default wander
}

impl CreaturePhysics {
    pub fn new(x: f32, y: f32, size: u32) -> Self {
        Self {
            position: Vec2::new(x, y),
            velocity: Vec2::ZERO,
            size,
            mass: 1.0,
            wander_timer: 0.0,
            wander_interval: 2.0,
            target: None,
            is_dragged: false,
            drag_offset: Vec2::ZERO,
            personality: CreaturePersonality::Neutral,
        }
    }

    pub fn with_personality(mut self, personality: CreaturePersonality) -> Self {
        self.personality = personality;
        self
    }

    pub fn update(
        &mut self,
        dt: f32,
        screen: &ScreenManager,
        cursor_pos: Option<Vec2>,
        all_creatures: &[Vec2],
    ) {
        // If dragged, follow cursor
        if self.is_dragged {
            if let Some(cursor) = cursor_pos {
                self.position = cursor + self.drag_offset;
                self.velocity = Vec2::ZERO;
            }
        } else if let Some(target) = self.target {
            // Move toward target
            let dir = target - self.position;
            let dist = dir.length();
            if dist > 5.0 {
                self.velocity = dir.normalize() * 100.0;
            } else {
                self.target = None;
                self.velocity = Vec2::ZERO;
            }
        } else {
            // Wander behavior
            self.wander_timer += dt;
            if self.wander_timer >= self.wander_interval {
                self.wander_timer = 0.0;
                self.wander_interval = 1.5 + rand::random::<f32>() * 3.0;
                let mut rng = rand::thread_rng();
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let speed = match self.personality {
                    CreaturePersonality::Energetic => 50.0 + rng.gen_range(0.0..80.0),
                    CreaturePersonality::Lazy => 10.0 + rng.gen_range(0.0..20.0),
                    _ => 30.0 + rng.gen_range(0.0..50.0),
                };
                self.velocity = Vec2::new(angle.cos() * speed, angle.sin() * speed);
            }

            // Personality-based cursor interaction
            if let Some(cursor) = cursor_pos {
                let to_cursor = cursor - self.position;
                let dist = to_cursor.length();
                if dist < 150.0 {
                    match self.personality {
                        CreaturePersonality::Curious => {
                            // Follow cursor
                            self.velocity = to_cursor.normalize() * 40.0;
                        }
                        CreaturePersonality::Shy => {
                            // Flee from cursor
                            self.velocity = -to_cursor.normalize() * 60.0;
                        }
                        _ => {}
                    }
                }
            }
        }

        // Collision avoidance with other creatures
        for other_pos in all_creatures {
            if *other_pos == self.position {
                continue;
            }
            let diff = self.position - *other_pos;
            let dist = diff.length();
            let min_dist = self.size as f32 * 1.5;
            if dist < min_dist && dist > 0.0 {
                let push = diff.normalize() * (min_dist - dist) * 2.0;
                self.velocity += push;
            }
        }

        self.position.x += self.velocity.x * dt;
        self.position.y += self.velocity.y * dt;

        // Clamp to screen
        if let Some(monitor) = screen.primary() {
            let (min_x, min_y, max_x, max_y) = monitor.bounds();
            let size = self.size as f32;
            if self.position.x < min_x as f32 {
                self.position.x = min_x as f32;
                self.velocity.x = self.velocity.x.abs();
            }
            if self.position.y < min_y as f32 {
                self.position.y = min_y as f32;
                self.velocity.y = self.velocity.y.abs();
            }
            if self.position.x + size > max_x as f32 {
                self.position.x = max_x as f32 - size;
                self.velocity.x = -self.velocity.x.abs();
            }
            if self.position.y + size > max_y as f32 {
                self.position.y = max_y as f32 - size;
                self.velocity.y = -self.velocity.y.abs();
            }
        }

        // Damping
        self.velocity *= 0.98;
    }

    pub fn start_drag(&mut self, cursor: Vec2) {
        self.is_dragged = true;
        self.drag_offset = self.position - cursor;
        self.velocity = Vec2::ZERO;
    }

    pub fn stop_drag(&mut self) {
        self.is_dragged = false;
    }

    pub fn contains_point(&self, point: Vec2) -> bool {
        let half_size = self.size as f32 / 2.0;
        let center = self.position + Vec2::new(half_size, half_size);
        (point - center).length() < half_size
    }

    pub fn set_target(&mut self, x: f32, y: f32) {
        self.target = Some(Vec2::new(x, y));
    }

    pub fn clear_target(&mut self) {
        self.target = None;
    }

    pub fn apply_force(&mut self, fx: f32, fy: f32) {
        self.velocity.x += fx / self.mass;
        self.velocity.y += fy / self.mass;
    }

    pub fn set_velocity(&mut self, vx: f32, vy: f32) {
        self.velocity = Vec2::new(vx, vy);
    }

    pub fn position_tuple(&self) -> (i32, i32) {
        (self.position.x as i32, self.position.y as i32)
    }
    pub fn is_moving(&self) -> bool {
        self.velocity.x.abs() > 0.1 || self.velocity.y.abs() > 0.1
    }
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creature_physics_creation() {
        let p = CreaturePhysics::new(100.0, 100.0, 64);
        assert_eq!(p.position_tuple(), (100, 100));
    }

    #[test]
    fn test_creature_physics_update() {
        let screen = ScreenManager::new();
        let mut p = CreaturePhysics::new(100.0, 100.0, 64);
        p.set_velocity(10.0, 5.0);
        p.update(1.0 / 60.0, &screen, None, &[]);
        assert!(p.position.x > 100.0);
    }

    #[test]
    fn test_creature_physics_bounce() {
        let screen = ScreenManager::new();
        let mut p = CreaturePhysics::new(0.0, 0.0, 64);
        p.set_velocity(-10.0, -10.0);
        p.update(1.0 / 60.0, &screen, None, &[]);
        assert!(p.velocity.x >= 0.0);
    }

    #[test]
    fn test_creature_physics_force() {
        let mut p = CreaturePhysics::new(0.0, 0.0, 64);
        p.apply_force(10.0, 20.0);
        assert_eq!(p.velocity.x, 10.0);
        assert_eq!(p.velocity.y, 20.0);
    }

    #[test]
    fn test_creature_physics_speed() {
        let mut p = CreaturePhysics::new(0.0, 0.0, 64);
        p.set_velocity(3.0, 4.0);
        assert_eq!(p.speed(), 5.0);
    }

    #[test]
    fn test_creature_physics_target() {
        let screen = ScreenManager::new();
        let mut p = CreaturePhysics::new(0.0, 0.0, 64);
        p.set_target(100.0, 100.0);
        for _ in 0..60 {
            p.update(1.0 / 60.0, &screen, None, &[]);
        }
        assert!(p.position.x > 0.0 || p.position.y > 0.0);
    }

    #[test]
    fn test_creature_physics_drag() {
        let _screen = ScreenManager::new();
        let mut p = CreaturePhysics::new(100.0, 100.0, 64);
        p.start_drag(Vec2::new(150.0, 150.0));
        assert!(p.is_dragged);
        p.stop_drag();
        assert!(!p.is_dragged);
    }

    #[test]
    fn test_creature_physics_contains_point() {
        let p = CreaturePhysics::new(100.0, 100.0, 64);
        assert!(p.contains_point(Vec2::new(132.0, 132.0)));
        assert!(!p.contains_point(Vec2::new(200.0, 200.0)));
    }

    #[test]
    fn test_creature_physics_collision_avoidance() {
        let screen = ScreenManager::new();
        let mut p1 = CreaturePhysics::new(100.0, 100.0, 64);
        let p2_pos = Vec2::new(110.0, 100.0); // Very close
        p1.update(1.0 / 60.0, &screen, None, &[p2_pos]);
        // Should have been pushed away
        assert!(p1.velocity.x < 0.0); // Pushed left
    }

    #[test]
    fn test_monitor_bounds() {
        let m = Monitor {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(m.bounds(), (0, 0, 1920, 1080));
    }

    #[test]
    fn test_monitor_center() {
        let m = Monitor {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(m.center(), (960, 540));
    }

    #[test]
    fn test_personality_cursor_interaction() {
        let screen = ScreenManager::new();
        let mut p = CreaturePhysics::new(100.0, 100.0, 64);
        p.personality = CreaturePersonality::Curious;
        // Cursor nearby should attract
        let cursor = Some(Vec2::new(110.0, 110.0));
        p.update(1.0 / 60.0, &screen, cursor, &[]);
        // Should move toward cursor
        assert!(p.velocity.length() > 0.0);
    }

    #[test]
    fn test_monitor_clamp() {
        let m = Monitor {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(m.clamp_position(-10, -10, 64), (0, 0));
    }

    #[test]
    fn test_monitor_contains() {
        let m = Monitor {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert!(m.contains(100, 100));
        assert!(!m.contains(2000, 2000));
    }

    #[test]
    fn test_screen_manager_creation() {
        let s = ScreenManager::new();
        assert!(!s.monitors().is_empty());
    }

    #[test]
    fn test_screen_manager_primary() {
        let s = ScreenManager::new();
        assert!(s.primary().is_some());
    }

    #[test]
    fn test_screen_manager_virtual_bounds() {
        let s = ScreenManager::new();
        let bounds = s.virtual_screen_bounds();
        assert!(bounds.2 > 0 && bounds.3 > 0);
    }
}
