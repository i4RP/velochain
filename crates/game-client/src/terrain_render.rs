//! Terrain rendering system.
//!
//! Renders the procedurally generated terrain from Phase 10
//! as colored 2D tile sprites using Bevy's sprite system.

use bevy::prelude::*;
use velochain_game_engine::terrain::{Biome, TerrainGenerator, TileType, CHUNK_SIZE};

/// Plugin for terrain rendering.
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainRenderConfig::default())
            .insert_resource(TerrainState::default())
            .add_systems(Startup, init_terrain)
            .add_systems(Update, update_visible_chunks);
    }
}

/// Size of each tile in pixels.
pub const TILE_PIXEL_SIZE: f32 = 16.0;

/// Configuration for terrain rendering.
#[derive(Resource)]
pub struct TerrainRenderConfig {
    /// World seed for terrain generation.
    pub seed: u64,
    /// How many chunks to render around the camera.
    pub render_distance: i32,
}

impl Default for TerrainRenderConfig {
    fn default() -> Self {
        Self {
            seed: 12345,
            render_distance: 3,
        }
    }
}

/// State tracking which chunks are currently rendered.
#[derive(Resource, Default)]
pub struct TerrainState {
    /// Currently loaded chunk coordinates.
    pub loaded_chunks: Vec<(i32, i32)>,
    /// Last camera chunk position (to detect when we need to load new chunks).
    pub last_camera_chunk: (i32, i32),
}

/// Marker component for terrain tile entities.
#[derive(Component)]
pub struct TerrainTile {
    pub chunk_x: i32,
    pub chunk_y: i32,
}

fn init_terrain(
    mut commands: Commands,
    config: Res<TerrainRenderConfig>,
    mut state: ResMut<TerrainState>,
) {
    let generator = TerrainGenerator::new(config.seed);

    // Generate initial chunks around origin
    let rd = config.render_distance;
    for cy in -rd..=rd {
        for cx in -rd..=rd {
            spawn_chunk(&mut commands, &generator, cx, cy);
            state.loaded_chunks.push((cx, cy));
        }
    }
}

fn update_visible_chunks(
    mut commands: Commands,
    config: Res<TerrainRenderConfig>,
    mut state: ResMut<TerrainState>,
    camera_query: Query<&Transform, With<crate::camera::MainCamera>>,
    tiles_query: Query<(Entity, &TerrainTile)>,
) {
    let Ok(cam_transform) = camera_query.get_single() else {
        return;
    };

    // Calculate which chunk the camera is in
    let cam_chunk_x =
        (cam_transform.translation.x / (CHUNK_SIZE as f32 * TILE_PIXEL_SIZE)).floor() as i32;
    let cam_chunk_y =
        (cam_transform.translation.y / (CHUNK_SIZE as f32 * TILE_PIXEL_SIZE)).floor() as i32;

    // Only update if camera moved to a new chunk
    if (cam_chunk_x, cam_chunk_y) == state.last_camera_chunk {
        return;
    }
    state.last_camera_chunk = (cam_chunk_x, cam_chunk_y);

    let rd = config.render_distance;
    let generator = TerrainGenerator::new(config.seed);

    // Determine which chunks should be visible
    let mut desired_chunks = Vec::new();
    for cy in (cam_chunk_y - rd)..=(cam_chunk_y + rd) {
        for cx in (cam_chunk_x - rd)..=(cam_chunk_x + rd) {
            desired_chunks.push((cx, cy));
        }
    }

    // Despawn chunks that are no longer visible
    let chunks_to_remove: Vec<(i32, i32)> = state
        .loaded_chunks
        .iter()
        .filter(|c| !desired_chunks.contains(c))
        .copied()
        .collect();

    for (entity, tile) in tiles_query.iter() {
        if chunks_to_remove.contains(&(tile.chunk_x, tile.chunk_y)) {
            commands.entity(entity).despawn();
        }
    }

    // Spawn new chunks
    for &(cx, cy) in &desired_chunks {
        if !state.loaded_chunks.contains(&(cx, cy)) {
            spawn_chunk(&mut commands, &generator, cx, cy);
        }
    }

    state.loaded_chunks = desired_chunks;
}

fn spawn_chunk(commands: &mut Commands, generator: &TerrainGenerator, chunk_x: i32, chunk_y: i32) {
    let chunk = generator.generate_chunk(chunk_x, chunk_y);

    for ty in 0..CHUNK_SIZE {
        for tx in 0..CHUNK_SIZE {
            let tile = chunk.get_tile(tx, ty);
            let color = tile_color(tile);

            let world_x = (chunk_x * CHUNK_SIZE as i32 + tx as i32) as f32 * TILE_PIXEL_SIZE;
            let world_y = (chunk_y * CHUNK_SIZE as i32 + ty as i32) as f32 * TILE_PIXEL_SIZE;

            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(TILE_PIXEL_SIZE, TILE_PIXEL_SIZE)),
                    ..default()
                },
                Transform::from_xyz(world_x, world_y, 0.0),
                TerrainTile { chunk_x, chunk_y },
            ));
        }
    }
}

/// Map tile types to colors for rendering.
pub fn tile_color(tile: TileType) -> Color {
    match tile {
        TileType::DeepWater => Color::srgb(0.05, 0.15, 0.5),
        TileType::ShallowWater => Color::srgb(0.15, 0.35, 0.65),
        TileType::Sand => Color::srgb(0.85, 0.8, 0.55),
        TileType::Grass => Color::srgb(0.25, 0.65, 0.2),
        TileType::Forest => Color::srgb(0.1, 0.45, 0.1),
        TileType::DenseForest => Color::srgb(0.05, 0.3, 0.05),
        TileType::Hills => Color::srgb(0.5, 0.55, 0.35),
        TileType::Mountain => Color::srgb(0.45, 0.4, 0.35),
        TileType::Snow => Color::srgb(0.9, 0.92, 0.95),
        TileType::Swamp => Color::srgb(0.25, 0.35, 0.15),
        TileType::Desert => Color::srgb(0.9, 0.75, 0.4),
        TileType::Village => Color::srgb(0.7, 0.55, 0.35),
    }
}

/// Map biome types to display names.
pub fn biome_name(biome: Biome) -> &'static str {
    match biome {
        Biome::Ocean => "Ocean",
        Biome::Beach => "Beach",
        Biome::Plains => "Plains",
        Biome::Forest => "Forest",
        Biome::Mountains => "Mountains",
        Biome::Desert => "Desert",
        Biome::Swamp => "Swamp",
        Biome::Tundra => "Tundra",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_colors_all_variants() {
        // Ensure all tile types have a color mapping (no panic)
        let tiles = [
            TileType::DeepWater,
            TileType::ShallowWater,
            TileType::Sand,
            TileType::Grass,
            TileType::Forest,
            TileType::DenseForest,
            TileType::Hills,
            TileType::Mountain,
            TileType::Snow,
            TileType::Swamp,
            TileType::Desert,
            TileType::Village,
        ];
        for tile in tiles {
            let _color = tile_color(tile);
        }
    }

    #[test]
    fn test_biome_names() {
        assert_eq!(biome_name(Biome::Ocean), "Ocean");
        assert_eq!(biome_name(Biome::Forest), "Forest");
        assert_eq!(biome_name(Biome::Tundra), "Tundra");
    }

    #[test]
    fn test_terrain_config_defaults() {
        let config = TerrainRenderConfig::default();
        assert!(config.render_distance > 0);
        assert!(config.seed > 0);
    }
}
