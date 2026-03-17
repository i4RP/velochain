//! Game systems that run each tick.
//!
//! Systems process entities deterministically each game tick.
//! They are run in a fixed order to ensure all nodes produce
//! the same result.

use crate::ecs::*;
use tracing::debug;

/// Run all game systems for one tick.
///
/// Systems must be run in this exact order for determinism.
pub fn run_tick(world: &mut EcsWorld, tick: u64) {
    physics_system(world, tick);
    npc_ai_system(world, tick);
    combat_system(world, tick);
    respawn_system(world, tick);
    debug!(
        "Game tick {} complete, entities={}",
        tick,
        world.entity_count()
    );
}

/// Physics system: apply velocity, gravity, collision.
fn physics_system(world: &mut EcsWorld, _tick: u64) {
    let entities: Vec<EntityId> = world.entities_with(|c| matches!(c, Component::Physics(_)));

    for entity_id in entities {
        if let Some(components) = world.get_components_mut(entity_id) {
            let mut new_pos = None;
            let mut velocity = None;

            // Read current state
            for component in components.iter() {
                match component {
                    Component::Position(pos) => new_pos = Some(pos.clone()),
                    Component::Physics(phys) => velocity = Some(phys.clone()),
                    _ => {}
                }
            }

            // Apply physics
            if let (Some(mut pos), Some(mut phys)) = (new_pos, velocity) {
                // Apply velocity
                pos.x += phys.velocity_x;
                pos.y += phys.velocity_y;
                pos.z += phys.velocity_z;

                // Simple gravity
                if !phys.is_grounded {
                    phys.velocity_z -= 9.81 * 0.016; // dt = 1/60
                }

                // Ground collision (simple: z >= 0)
                if pos.z <= 0.0 {
                    pos.z = 0.0;
                    phys.velocity_z = 0.0;
                    phys.is_grounded = true;
                }

                // Friction
                phys.velocity_x *= 0.9;
                phys.velocity_y *= 0.9;

                // Write back
                for component in components.iter_mut() {
                    match component {
                        Component::Position(p) => *p = pos.clone(),
                        Component::Physics(p) => *p = phys.clone(),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// NPC AI system: update NPC behaviors.
fn npc_ai_system(world: &mut EcsWorld, tick: u64) {
    let npcs: Vec<EntityId> = world.npc_entities();

    for entity_id in npcs {
        if let Some(components) = world.get_components_mut(entity_id) {
            let mut npc_data = None;
            let mut current_pos = None;

            for component in components.iter() {
                match component {
                    Component::Npc(npc) => npc_data = Some(npc.clone()),
                    Component::Position(pos) => current_pos = Some(pos.clone()),
                    _ => {}
                }
            }

            if let (Some(npc), Some(mut pos)) = (npc_data, current_pos) {
                match npc.behavior {
                    NpcBehavior::Patrol => {
                        // Simple patrol: move in a circle around home
                        let angle = (tick as f32) * 0.05;
                        let radius = 5.0;
                        pos.x = npc.home_position.x + angle.cos() * radius;
                        pos.y = npc.home_position.y + angle.sin() * radius;
                    }
                    NpcBehavior::Idle => {
                        // Stay at home position
                    }
                    _ => {}
                }

                // Write back position
                for component in components.iter_mut() {
                    if let Component::Position(p) = component {
                        *p = pos.clone();
                    }
                }
            }
        }
    }
}

/// Combat system: process damage and kills.
fn combat_system(world: &mut EcsWorld, _tick: u64) {
    let entities: Vec<EntityId> = world.entities_with(|c| matches!(c, Component::Health(_)));

    for entity_id in entities {
        if let Some(components) = world.get_components_mut(entity_id) {
            for component in components.iter_mut() {
                if let Component::Health(health) = component {
                    if health.current <= 0.0 && !health.is_dead {
                        health.is_dead = true;
                        health.current = 0.0;
                    }
                }
            }
        }
    }
}

/// Respawn system: handle dead players.
fn respawn_system(world: &mut EcsWorld, _tick: u64) {
    let players: Vec<EntityId> = world.player_entities();

    for entity_id in players {
        if let Some(components) = world.get_components_mut(entity_id) {
            let mut needs_respawn = false;

            for component in components.iter() {
                if let Component::Health(health) = component {
                    if health.is_dead {
                        needs_respawn = true;
                    }
                }
            }

            if needs_respawn {
                for component in components.iter_mut() {
                    match component {
                        Component::Health(health) => {
                            health.current = health.maximum;
                            health.is_dead = false;
                        }
                        Component::Position(pos) => {
                            // Respawn at origin
                            pos.x = 0.0;
                            pos.y = 0.0;
                            pos.z = 64.0;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
