//! Enhanced NPC AI with pathfinding, behavior patterns, and spawn management.
//!
//! NPCs follow deterministic behavior trees that are evaluated each tick.
//! All randomness uses seeded RNG for consensus-safe execution.

use crate::ecs::*;
use serde::{Deserialize, Serialize};

/// NPC behavior state machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AiState {
    /// Standing still at home position.
    Idle { ticks_remaining: u32 },
    /// Patrolling between waypoints.
    Patrolling { waypoint_index: usize },
    /// Chasing a target entity.
    Chasing { target_id: EntityId },
    /// Fleeing from a threat.
    Fleeing {
        threat_id: EntityId,
        ticks_remaining: u32,
    },
    /// Returning to home position.
    Returning,
    /// Dead, waiting for respawn.
    Dead { respawn_ticks: u32 },
}

/// NPC archetype defining base stats and behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcArchetype {
    pub npc_type: String,
    pub display_name: String,
    pub max_health: f32,
    pub attack_damage: f32,
    pub attack_range: f32,
    pub attack_cooldown: u32,
    pub move_speed: f32,
    pub aggro_range: f32,
    pub leash_range: f32,
    pub behavior_pattern: BehaviorPattern,
    pub respawn_ticks: u32,
    pub experience_reward: u64,
    pub level: u32,
}

/// High-level behavior patterns for NPCs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum BehaviorPattern {
    /// Stays in one place, does not attack.
    Passive,
    /// Walks a patrol route, does not attack unless attacked.
    PatrolPassive,
    /// Patrols and attacks players on sight.
    PatrolAggressive,
    /// Stands guard, attacks nearby players.
    Guardian,
    /// Wanders randomly, flees when attacked.
    Timid,
    /// Wanders randomly, attacks on sight.
    Predator,
}

/// Spawn point definition for NPCs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub archetype: String,
    pub position: [f32; 3],
    pub patrol_waypoints: Vec<[f32; 3]>,
    pub spawn_radius: f32,
    pub max_count: u32,
    pub current_count: u32,
    pub respawn_cooldown: u32,
    pub cooldown_remaining: u32,
}

/// Spawn manager tracks all spawn points and manages NPC lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnManager {
    pub spawn_points: Vec<SpawnPoint>,
    pub archetypes: Vec<NpcArchetype>,
}

impl SpawnManager {
    /// Create a new spawn manager with default archetypes.
    pub fn new() -> Self {
        Self {
            spawn_points: Vec::new(),
            archetypes: Self::default_archetypes(),
        }
    }

    /// Get archetype by npc_type name.
    pub fn get_archetype(&self, npc_type: &str) -> Option<&NpcArchetype> {
        self.archetypes.iter().find(|a| a.npc_type == npc_type)
    }

    /// Default NPC archetypes.
    fn default_archetypes() -> Vec<NpcArchetype> {
        vec![
            NpcArchetype {
                npc_type: "merchant".into(),
                display_name: "Wandering Merchant".into(),
                max_health: 100.0,
                attack_damage: 0.0,
                attack_range: 0.0,
                attack_cooldown: 0,
                move_speed: 0.5,
                aggro_range: 0.0,
                leash_range: 20.0,
                behavior_pattern: BehaviorPattern::Passive,
                respawn_ticks: 600,
                experience_reward: 0,
                level: 1,
            },
            NpcArchetype {
                npc_type: "guard".into(),
                display_name: "Town Guard".into(),
                max_health: 150.0,
                attack_damage: 12.0,
                attack_range: 2.0,
                attack_cooldown: 10,
                move_speed: 1.0,
                aggro_range: 8.0,
                leash_range: 30.0,
                behavior_pattern: BehaviorPattern::Guardian,
                respawn_ticks: 300,
                experience_reward: 25,
                level: 3,
            },
            NpcArchetype {
                npc_type: "wolf".into(),
                display_name: "Wild Wolf".into(),
                max_health: 60.0,
                attack_damage: 8.0,
                attack_range: 1.5,
                attack_cooldown: 8,
                move_speed: 1.5,
                aggro_range: 10.0,
                leash_range: 25.0,
                behavior_pattern: BehaviorPattern::Predator,
                respawn_ticks: 200,
                experience_reward: 15,
                level: 2,
            },
            NpcArchetype {
                npc_type: "rabbit".into(),
                display_name: "Forest Rabbit".into(),
                max_health: 15.0,
                attack_damage: 0.0,
                attack_range: 0.0,
                attack_cooldown: 0,
                move_speed: 2.0,
                aggro_range: 6.0,
                leash_range: 20.0,
                behavior_pattern: BehaviorPattern::Timid,
                respawn_ticks: 100,
                experience_reward: 5,
                level: 1,
            },
            NpcArchetype {
                npc_type: "dragon".into(),
                display_name: "Elder Dragon".into(),
                max_health: 1000.0,
                attack_damage: 50.0,
                attack_range: 4.0,
                attack_cooldown: 15,
                move_speed: 1.2,
                aggro_range: 15.0,
                leash_range: 40.0,
                behavior_pattern: BehaviorPattern::PatrolAggressive,
                respawn_ticks: 3000,
                experience_reward: 500,
                level: 10,
            },
            NpcArchetype {
                npc_type: "skeleton".into(),
                display_name: "Skeletal Warrior".into(),
                max_health: 80.0,
                attack_damage: 10.0,
                attack_range: 1.5,
                attack_cooldown: 12,
                move_speed: 0.8,
                aggro_range: 8.0,
                leash_range: 20.0,
                behavior_pattern: BehaviorPattern::PatrolAggressive,
                respawn_ticks: 250,
                experience_reward: 20,
                level: 3,
            },
        ]
    }

    /// Generate initial spawn points around spawn area.
    pub fn generate_spawn_points(&mut self, seed: u64) {
        use sha3::{Digest, Keccak256};

        let spawn_configs: Vec<(&str, f32, f32, u32, f32)> = vec![
            // (npc_type, x, y, max_count, spawn_radius)
            ("merchant", 5.0, 5.0, 1, 3.0),
            ("guard", 0.0, 10.0, 2, 5.0),
            ("guard", 10.0, 0.0, 2, 5.0),
            ("wolf", 30.0, 30.0, 3, 10.0),
            ("wolf", -20.0, 25.0, 2, 8.0),
            ("rabbit", 15.0, 15.0, 4, 12.0),
            ("rabbit", -10.0, -10.0, 3, 10.0),
            ("skeleton", 40.0, -30.0, 3, 8.0),
            ("skeleton", -35.0, 40.0, 2, 6.0),
            ("dragon", 60.0, 60.0, 1, 5.0),
        ];

        for (i, (npc_type, base_x, base_y, max_count, radius)) in spawn_configs.iter().enumerate() {
            // Deterministic offset from seed
            let mut hasher = Keccak256::new();
            hasher.update(seed.to_le_bytes());
            hasher.update((i as u64).to_le_bytes());
            let hash = hasher.finalize();
            let offset_x = (hash[0] as f32 / 255.0 - 0.5) * 5.0;
            let offset_y = (hash[1] as f32 / 255.0 - 0.5) * 5.0;

            let mut waypoints = Vec::new();
            // Generate patrol waypoints for patrol-type NPCs
            if let Some(arch) = self.get_archetype(npc_type) {
                if matches!(
                    arch.behavior_pattern,
                    BehaviorPattern::PatrolPassive
                        | BehaviorPattern::PatrolAggressive
                        | BehaviorPattern::Guardian
                ) {
                    for wp in 0..4 {
                        let angle = (wp as f32) * std::f32::consts::FRAC_PI_2;
                        waypoints.push([
                            base_x + offset_x + angle.cos() * radius * 0.5,
                            base_y + offset_y + angle.sin() * radius * 0.5,
                            0.0,
                        ]);
                    }
                }
            }

            self.spawn_points.push(SpawnPoint {
                archetype: npc_type.to_string(),
                position: [base_x + offset_x, base_y + offset_y, 0.0],
                patrol_waypoints: waypoints,
                spawn_radius: *radius,
                max_count: *max_count,
                current_count: 0,
                respawn_cooldown: self
                    .get_archetype(npc_type)
                    .map(|a| a.respawn_ticks)
                    .unwrap_or(200),
                cooldown_remaining: 0,
            });
        }
    }

    /// Process spawn logic for one tick. Returns list of (archetype, position, waypoints) to spawn.
    pub fn tick_spawns(&mut self) -> Vec<(String, [f32; 3], Vec<[f32; 3]>)> {
        let mut to_spawn = Vec::new();

        for sp in &mut self.spawn_points {
            if sp.current_count < sp.max_count {
                if sp.cooldown_remaining > 0 {
                    sp.cooldown_remaining -= 1;
                } else {
                    to_spawn.push((
                        sp.archetype.clone(),
                        sp.position,
                        sp.patrol_waypoints.clone(),
                    ));
                    sp.current_count += 1;
                    sp.cooldown_remaining = sp.respawn_cooldown;
                }
            }
        }

        to_spawn
    }

    /// Notify that an NPC of the given type died.
    pub fn notify_death(&mut self, npc_type: &str, position: [f32; 3]) {
        // Find the closest spawn point of this type and decrement count
        let mut best_idx = None;
        let mut best_dist = f32::MAX;

        for (i, sp) in self.spawn_points.iter().enumerate() {
            if sp.archetype == npc_type && sp.current_count > 0 {
                let dx = sp.position[0] - position[0];
                let dy = sp.position[1] - position[1];
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(i);
                }
            }
        }

        if let Some(idx) = best_idx {
            self.spawn_points[idx].current_count =
                self.spawn_points[idx].current_count.saturating_sub(1);
            self.spawn_points[idx].cooldown_remaining = self.spawn_points[idx].respawn_cooldown;
        }
    }
}

impl Default for SpawnManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate distance between two 2D positions.
pub fn distance_2d(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

/// Simple A* pathfinding result (next step direction).
///
/// For blockchain execution, we use simplified direct-line movement
/// rather than full A* to keep tick computation bounded.
pub fn move_toward(from: &PositionComponent, target: &[f32; 3], speed: f32) -> (f32, f32) {
    let dx = target[0] - from.x;
    let dy = target[1] - from.y;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist < 0.1 {
        return (0.0, 0.0);
    }

    let nx = dx / dist;
    let ny = dy / dist;
    (nx * speed, ny * speed)
}

/// Find the nearest player entity position.
pub fn find_nearest_player(
    ecs: &EcsWorld,
    from: &PositionComponent,
    max_range: f32,
) -> Option<(EntityId, f32)> {
    let players = ecs.player_entities();
    let mut nearest: Option<(EntityId, f32)> = None;

    for player_id in players {
        if let Some(components) = ecs.get_components(player_id) {
            let mut is_alive = true;
            let mut player_pos = None;

            for comp in components {
                match comp {
                    Component::Position(pos) => player_pos = Some(pos),
                    Component::Health(h) => is_alive = !h.is_dead,
                    _ => {}
                }
            }

            if !is_alive {
                continue;
            }

            if let Some(pos) = player_pos {
                let dx = pos.x - from.x;
                let dy = pos.y - from.y;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= max_range && (nearest.is_none() || dist < nearest.unwrap().1) {
                    nearest = Some((player_id, dist));
                }
            }
        }
    }

    nearest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_manager_default_archetypes() {
        let manager = SpawnManager::new();
        assert!(manager.get_archetype("merchant").is_some());
        assert!(manager.get_archetype("wolf").is_some());
        assert!(manager.get_archetype("dragon").is_some());
        assert!(manager.get_archetype("nonexistent").is_none());
    }

    #[test]
    fn test_spawn_point_generation() {
        let mut manager = SpawnManager::new();
        manager.generate_spawn_points(42);
        assert!(!manager.spawn_points.is_empty());
    }

    #[test]
    fn test_spawn_tick() {
        let mut manager = SpawnManager::new();
        manager.generate_spawn_points(42);
        let spawns = manager.tick_spawns();
        // First tick should spawn some NPCs
        assert!(!spawns.is_empty());
    }

    #[test]
    fn test_move_toward() {
        let from = PositionComponent {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let target = [10.0, 0.0, 0.0];
        let (dx, dy) = move_toward(&from, &target, 1.0);
        assert!((dx - 1.0).abs() < 0.01);
        assert!(dy.abs() < 0.01);
    }

    #[test]
    fn test_move_toward_already_at_target() {
        let from = PositionComponent {
            x: 5.0,
            y: 5.0,
            z: 0.0,
        };
        let target = [5.0, 5.0, 0.0];
        let (dx, dy) = move_toward(&from, &target, 1.0);
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn test_distance_2d() {
        let a = [0.0, 0.0, 0.0];
        let b = [3.0, 4.0, 0.0];
        assert!((distance_2d(&a, &b) - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_behavior_patterns() {
        let manager = SpawnManager::new();
        let merchant = manager.get_archetype("merchant").unwrap();
        assert_eq!(merchant.behavior_pattern, BehaviorPattern::Passive);
        assert_eq!(merchant.attack_damage, 0.0);

        let wolf = manager.get_archetype("wolf").unwrap();
        assert_eq!(wolf.behavior_pattern, BehaviorPattern::Predator);
        assert!(wolf.attack_damage > 0.0);
    }

    #[test]
    fn test_notify_death() {
        let mut manager = SpawnManager::new();
        manager.generate_spawn_points(42);

        // Spawn some NPCs first
        let spawns = manager.tick_spawns();
        let initial_wolf_count: u32 = manager
            .spawn_points
            .iter()
            .filter(|sp| sp.archetype == "wolf")
            .map(|sp| sp.current_count)
            .sum();

        if !spawns.is_empty() {
            let wolf_spawns: Vec<_> = spawns.iter().filter(|(t, _, _)| t == "wolf").collect();
            if let Some((_, pos, _)) = wolf_spawns.first() {
                manager.notify_death("wolf", *pos);
                let after_count: u32 = manager
                    .spawn_points
                    .iter()
                    .filter(|sp| sp.archetype == "wolf")
                    .map(|sp| sp.current_count)
                    .sum();
                assert!(after_count < initial_wolf_count);
            }
        }
    }
}
