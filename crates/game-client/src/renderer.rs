//! Entity rendering system.
//!
//! Renders players, NPCs, and ground items as colored sprites.
//! Each entity type has a distinct shape and color.

use bevy::prelude::*;
use crate::camera::LocalPlayer;
use crate::terrain_render::TILE_PIXEL_SIZE;

/// Plugin for entity rendering.
pub struct RendererPlugin;

impl Plugin for RendererPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EntityRegistry::default())
            .add_systems(Update, (
                player_movement,
                update_entity_sprites,
                animate_ground_items,
            ));
    }
}

/// Registry of known entities and their server-side data.
#[derive(Resource, Default)]
pub struct EntityRegistry {
    /// Local player's entity ID on the server.
    pub local_player_id: Option<u64>,
    /// Local player address.
    pub local_address: String,
}

/// Server-synced entity data component.
#[derive(Component)]
pub struct GameEntity {
    /// Server entity ID.
    pub server_id: u64,
    /// Entity type string (e.g., "player:0x...", "npc:wolf").
    pub entity_type: String,
    /// Current health.
    pub health: f32,
    /// Maximum health.
    pub max_health: f32,
    /// Is this entity alive?
    pub is_alive: bool,
}

/// NPC marker component.
#[derive(Component)]
pub struct NpcMarker {
    pub npc_type: String,
}

/// Ground item marker component.
#[derive(Component)]
pub struct GroundItemMarker {
    pub item_id: u32,
    pub quantity: u32,
    pub drop_id: u64,
}

/// Spawn the local player sprite.
pub fn spawn_local_player(
    commands: &mut Commands,
    position: Vec3,
    server_id: u64,
    address: &str,
) -> Entity {
    commands
        .spawn((
            Sprite {
                color: Color::srgb(0.2, 0.6, 1.0),
                custom_size: Some(Vec2::new(TILE_PIXEL_SIZE * 0.8, TILE_PIXEL_SIZE * 0.8)),
                ..default()
            },
            Transform::from_translation(position),
            LocalPlayer,
            GameEntity {
                server_id,
                entity_type: format!("player:{}", address),
                health: 100.0,
                max_health: 100.0,
                is_alive: true,
            },
        ))
        .id()
}

/// Spawn a remote player sprite.
pub fn spawn_remote_player(
    commands: &mut Commands,
    position: Vec3,
    server_id: u64,
    address: &str,
) -> Entity {
    commands
        .spawn((
            Sprite {
                color: Color::srgb(0.3, 0.8, 0.3),
                custom_size: Some(Vec2::new(TILE_PIXEL_SIZE * 0.7, TILE_PIXEL_SIZE * 0.7)),
                ..default()
            },
            Transform::from_translation(position),
            GameEntity {
                server_id,
                entity_type: format!("player:{}", address),
                health: 100.0,
                max_health: 100.0,
                is_alive: true,
            },
        ))
        .id()
}

/// Spawn an NPC sprite.
pub fn spawn_npc(
    commands: &mut Commands,
    position: Vec3,
    server_id: u64,
    npc_type: &str,
    health: f32,
    max_health: f32,
) -> Entity {
    let color = npc_color(npc_type);
    let size = npc_size(npc_type);

    commands
        .spawn((
            Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(position),
            GameEntity {
                server_id,
                entity_type: format!("npc:{}", npc_type),
                health,
                max_health,
                is_alive: true,
            },
            NpcMarker {
                npc_type: npc_type.to_string(),
            },
        ))
        .id()
}

/// Spawn a ground item sprite.
pub fn spawn_ground_item(
    commands: &mut Commands,
    position: Vec3,
    item_id: u32,
    quantity: u32,
    drop_id: u64,
) -> Entity {
    commands
        .spawn((
            Sprite {
                color: Color::srgb(1.0, 0.85, 0.0),
                custom_size: Some(Vec2::new(TILE_PIXEL_SIZE * 0.4, TILE_PIXEL_SIZE * 0.4)),
                ..default()
            },
            Transform::from_translation(position),
            GroundItemMarker {
                item_id,
                quantity,
                drop_id,
            },
        ))
        .id()
}

/// Get NPC color based on type.
pub fn npc_color(npc_type: &str) -> Color {
    match npc_type {
        "merchant" => Color::srgb(0.9, 0.7, 0.2),
        "guard" => Color::srgb(0.5, 0.5, 0.7),
        "wolf" => Color::srgb(0.5, 0.4, 0.3),
        "rabbit" => Color::srgb(0.8, 0.75, 0.65),
        "dragon" => Color::srgb(0.8, 0.1, 0.1),
        "skeleton" => Color::srgb(0.85, 0.85, 0.8),
        _ => Color::srgb(0.6, 0.6, 0.6),
    }
}

/// Get NPC sprite size based on type.
pub fn npc_size(npc_type: &str) -> Vec2 {
    let base = TILE_PIXEL_SIZE;
    match npc_type {
        "dragon" => Vec2::new(base * 1.5, base * 1.5),
        "rabbit" => Vec2::new(base * 0.4, base * 0.4),
        "wolf" => Vec2::new(base * 0.7, base * 0.5),
        _ => Vec2::new(base * 0.7, base * 0.7),
    }
}

/// Player movement system using WASD/arrow keys.
fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<LocalPlayer>>,
) {
    let Ok(mut transform) = query.get_single_mut() else {
        return;
    };

    let speed = 100.0 * time.delta_secs();
    let mut direction = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction != Vec2::ZERO {
        let movement = direction.normalize() * speed;
        transform.translation.x += movement.x;
        transform.translation.y += movement.y;
    }
}

/// Update sprite colors based on entity health.
fn update_entity_sprites(
    mut query: Query<(&GameEntity, &mut Sprite)>,
) {
    for (entity, mut sprite) in query.iter_mut() {
        if !entity.is_alive {
            // Gray out dead entities
            sprite.color = Color::srgba(0.3, 0.3, 0.3, 0.5);
        }
    }
}

/// Animate ground items with a gentle bob.
fn animate_ground_items(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<GroundItemMarker>>,
) {
    for mut transform in query.iter_mut() {
        // Gentle floating animation
        let bob = (time.elapsed_secs() * 2.0).sin() * 2.0;
        transform.translation.z = 1.0 + bob;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npc_colors_all_types() {
        let types = ["merchant", "guard", "wolf", "rabbit", "dragon", "skeleton", "unknown"];
        for t in types {
            let _color = npc_color(t);
        }
    }

    #[test]
    fn test_npc_sizes() {
        let dragon_size = npc_size("dragon");
        let rabbit_size = npc_size("rabbit");
        assert!(dragon_size.x > rabbit_size.x);
        assert!(dragon_size.y > rabbit_size.y);
    }

    #[test]
    fn test_entity_registry_default() {
        let reg = EntityRegistry::default();
        assert!(reg.local_player_id.is_none());
        assert!(reg.local_address.is_empty());
    }
}
