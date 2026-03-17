//! Procedural terrain generation using seed-based deterministic algorithms.
//!
//! Generates a 2D tile-based world with biomes, terrain heights, and
//! resource distribution. All generation is deterministic given the same seed.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Chunk size in tiles (16x16).
pub const CHUNK_SIZE: u32 = 16;

/// Tile types for the world map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TileType {
    /// Deep water (impassable).
    DeepWater,
    /// Shallow water (slow movement).
    ShallowWater,
    /// Sand/beach.
    Sand,
    /// Grass plains.
    Grass,
    /// Forest (contains trees, slows movement).
    Forest,
    /// Dense forest.
    DenseForest,
    /// Hills.
    Hills,
    /// Mountain (impassable on foot).
    Mountain,
    /// Snow-capped peak.
    Snow,
    /// Swamp (slow movement, dangerous).
    Swamp,
    /// Desert.
    Desert,
    /// Village/settlement.
    Village,
}

impl TileType {
    /// Whether entities can walk on this tile.
    pub fn is_walkable(&self) -> bool {
        !matches!(self, TileType::DeepWater | TileType::Mountain | TileType::Snow)
    }

    /// Movement speed multiplier on this tile (1.0 = normal).
    pub fn movement_multiplier(&self) -> f32 {
        match self {
            TileType::DeepWater | TileType::Mountain | TileType::Snow => 0.0,
            TileType::ShallowWater | TileType::Swamp => 0.5,
            TileType::Sand | TileType::Desert => 0.8,
            TileType::Forest => 0.7,
            TileType::DenseForest => 0.5,
            TileType::Hills => 0.6,
            TileType::Grass | TileType::Village => 1.0,
        }
    }
}

/// Biome types that determine terrain generation patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    Beach,
    Plains,
    Forest,
    Mountains,
    Desert,
    Swamp,
    Tundra,
}

/// A single chunk of the world (16x16 tiles).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Chunk coordinates (chunk-space, not tile-space).
    pub cx: i32,
    pub cy: i32,
    /// Tile data (CHUNK_SIZE x CHUNK_SIZE), row-major.
    pub tiles: Vec<TileType>,
    /// Height map (0.0 - 1.0) for each tile.
    pub heights: Vec<f32>,
    /// Biome of this chunk.
    pub biome: Biome,
}

impl Chunk {
    /// Get tile at local coordinates (lx, ly) within the chunk.
    pub fn get_tile(&self, lx: u32, ly: u32) -> TileType {
        if lx >= CHUNK_SIZE || ly >= CHUNK_SIZE {
            return TileType::DeepWater;
        }
        self.tiles[(ly * CHUNK_SIZE + lx) as usize]
    }

    /// Get height at local coordinates.
    pub fn get_height(&self, lx: u32, ly: u32) -> f32 {
        if lx >= CHUNK_SIZE || ly >= CHUNK_SIZE {
            return 0.0;
        }
        self.heights[(ly * CHUNK_SIZE + lx) as usize]
    }
}

/// Terrain generator using seed-based deterministic noise.
pub struct TerrainGenerator {
    seed: u64,
}

impl TerrainGenerator {
    /// Create a new terrain generator with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Generate a chunk at the given chunk coordinates.
    pub fn generate_chunk(&self, cx: i32, cy: i32) -> Chunk {
        let size = (CHUNK_SIZE * CHUNK_SIZE) as usize;
        let mut tiles = Vec::with_capacity(size);
        let mut heights = Vec::with_capacity(size);

        // Determine biome for this chunk based on large-scale noise
        let biome = self.biome_at_chunk(cx, cy);

        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let world_x = cx as f64 * CHUNK_SIZE as f64 + lx as f64;
                let world_y = cy as f64 * CHUNK_SIZE as f64 + ly as f64;

                let height = self.height_at(world_x, world_y);
                let tile = self.tile_from_height_and_biome(height, biome, world_x, world_y);

                heights.push(height as f32);
                tiles.push(tile);
            }
        }

        Chunk {
            cx,
            cy,
            tiles,
            heights,
            biome,
        }
    }

    /// Determine the biome at a chunk position.
    fn biome_at_chunk(&self, cx: i32, cy: i32) -> Biome {
        // Use large-scale noise for biome selection
        let temperature = self.noise2d(cx as f64 * 0.1, cy as f64 * 0.1, 1);
        let moisture = self.noise2d(cx as f64 * 0.1, cy as f64 * 0.1, 2);

        // Distance from origin affects ocean probability
        let dist = ((cx * cx + cy * cy) as f64).sqrt();

        if dist > 20.0 && temperature < 0.3 {
            return Biome::Ocean;
        }

        match (temperature > 0.5, moisture > 0.5) {
            (true, true) => {
                if temperature > 0.8 {
                    Biome::Swamp
                } else {
                    Biome::Forest
                }
            }
            (true, false) => Biome::Desert,
            (false, true) => {
                if temperature < 0.2 {
                    Biome::Tundra
                } else {
                    Biome::Plains
                }
            }
            (false, false) => {
                if moisture < 0.3 {
                    Biome::Mountains
                } else {
                    Biome::Beach
                }
            }
        }
    }

    /// Get the height at a world position (0.0 - 1.0).
    fn height_at(&self, x: f64, y: f64) -> f64 {
        // Multi-octave noise for terrain height
        let mut height = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 0.02;
        let mut total_amplitude = 0.0;

        for octave in 0..4 {
            height += self.noise2d(x * frequency, y * frequency, octave + 10) * amplitude;
            total_amplitude += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        height / total_amplitude
    }

    /// Convert height and biome to a tile type.
    fn tile_from_height_and_biome(
        &self,
        height: f64,
        biome: Biome,
        x: f64,
        y: f64,
    ) -> TileType {
        match biome {
            Biome::Ocean => {
                if height < 0.3 {
                    TileType::DeepWater
                } else {
                    TileType::ShallowWater
                }
            }
            Biome::Beach => {
                if height < 0.3 {
                    TileType::ShallowWater
                } else if height < 0.5 {
                    TileType::Sand
                } else {
                    TileType::Grass
                }
            }
            Biome::Plains => {
                let detail = self.noise2d(x * 0.1, y * 0.1, 50);
                if height < 0.25 {
                    TileType::ShallowWater
                } else if detail > 0.7 {
                    TileType::Forest
                } else if detail > 0.85 {
                    TileType::Village
                } else {
                    TileType::Grass
                }
            }
            Biome::Forest => {
                let detail = self.noise2d(x * 0.15, y * 0.15, 51);
                if height < 0.2 {
                    TileType::ShallowWater
                } else if detail > 0.6 {
                    TileType::DenseForest
                } else {
                    TileType::Forest
                }
            }
            Biome::Mountains => {
                if height > 0.8 {
                    TileType::Snow
                } else if height > 0.6 {
                    TileType::Mountain
                } else if height > 0.4 {
                    TileType::Hills
                } else {
                    TileType::Grass
                }
            }
            Biome::Desert => {
                let detail = self.noise2d(x * 0.08, y * 0.08, 52);
                if height < 0.15 {
                    TileType::ShallowWater
                } else if detail > 0.85 {
                    TileType::Sand
                } else {
                    TileType::Desert
                }
            }
            Biome::Swamp => {
                let detail = self.noise2d(x * 0.12, y * 0.12, 53);
                if detail < 0.4 {
                    TileType::ShallowWater
                } else {
                    TileType::Swamp
                }
            }
            Biome::Tundra => {
                if height > 0.7 {
                    TileType::Snow
                } else if height > 0.5 {
                    TileType::Hills
                } else {
                    TileType::Grass
                }
            }
        }
    }

    /// Simple deterministic 2D noise function (hash-based).
    ///
    /// Returns a value in [0.0, 1.0) for any input coordinates.
    /// Uses the seed to ensure determinism across nodes.
    fn noise2d(&self, x: f64, y: f64, octave: u64) -> f64 {
        // Integer coordinates for grid cell
        let ix = x.floor() as i64;
        let iy = y.floor() as i64;
        let fx = x - x.floor();
        let fy = y - y.floor();

        // Smooth interpolation (cubic hermite)
        let sx = fx * fx * (3.0 - 2.0 * fx);
        let sy = fy * fy * (3.0 - 2.0 * fy);

        // Hash corners
        let n00 = self.hash2d(ix, iy, octave);
        let n10 = self.hash2d(ix + 1, iy, octave);
        let n01 = self.hash2d(ix, iy + 1, octave);
        let n11 = self.hash2d(ix + 1, iy + 1, octave);

        // Bilinear interpolation
        let nx0 = n00 + sx * (n10 - n00);
        let nx1 = n01 + sx * (n11 - n01);
        nx0 + sy * (nx1 - nx0)
    }

    /// Deterministic hash function for 2D coordinates.
    fn hash2d(&self, x: i64, y: i64, octave: u64) -> f64 {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(self.seed.to_le_bytes());
        hasher.update(x.to_le_bytes());
        hasher.update(y.to_le_bytes());
        hasher.update(octave.to_le_bytes());
        let result = hasher.finalize();
        let val = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
        val as f64 / u32::MAX as f64
    }
}

/// World terrain manager that caches generated chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldTerrain {
    seed: u64,
    /// Cached chunks keyed by (cx, cy).
    chunks: HashMap<(i32, i32), Chunk>,
}

impl WorldTerrain {
    /// Create a new world terrain with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            chunks: HashMap::new(),
        }
    }

    /// Get or generate the chunk at the given chunk coordinates.
    pub fn get_chunk(&mut self, cx: i32, cy: i32) -> &Chunk {
        if !self.chunks.contains_key(&(cx, cy)) {
            let gen = TerrainGenerator::new(self.seed);
            let chunk = gen.generate_chunk(cx, cy);
            self.chunks.insert((cx, cy), chunk);
        }
        self.chunks.get(&(cx, cy)).unwrap()
    }

    /// Get the tile at world coordinates (tile-space).
    pub fn get_tile_at(&mut self, world_x: i32, world_y: i32) -> TileType {
        let cx = world_x.div_euclid(CHUNK_SIZE as i32);
        let cy = world_y.div_euclid(CHUNK_SIZE as i32);
        let lx = world_x.rem_euclid(CHUNK_SIZE as i32) as u32;
        let ly = world_y.rem_euclid(CHUNK_SIZE as i32) as u32;
        self.get_chunk(cx, cy).get_tile(lx, ly)
    }

    /// Get the height at world coordinates.
    pub fn get_height_at(&mut self, world_x: i32, world_y: i32) -> f32 {
        let cx = world_x.div_euclid(CHUNK_SIZE as i32);
        let cy = world_y.div_euclid(CHUNK_SIZE as i32);
        let lx = world_x.rem_euclid(CHUNK_SIZE as i32) as u32;
        let ly = world_y.rem_euclid(CHUNK_SIZE as i32) as u32;
        self.get_chunk(cx, cy).get_height(lx, ly)
    }

    /// Check if a position is walkable (world float coords).
    pub fn is_walkable(&mut self, x: f32, y: f32) -> bool {
        let tile = self.get_tile_at(x.floor() as i32, y.floor() as i32);
        tile.is_walkable()
    }

    /// Get movement speed multiplier at position.
    pub fn movement_speed_at(&mut self, x: f32, y: f32) -> f32 {
        let tile = self.get_tile_at(x.floor() as i32, y.floor() as i32);
        tile.movement_multiplier()
    }

    /// Get the biome at world float coordinates.
    pub fn biome_at(&mut self, x: f32, y: f32) -> Biome {
        let cx = (x / CHUNK_SIZE as f32).floor() as i32;
        let cy = (y / CHUNK_SIZE as f32).floor() as i32;
        self.get_chunk(cx, cy).biome
    }

    /// Number of cached chunks.
    pub fn cached_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Get the seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Generate initial chunks around spawn (load a 5x5 area around origin).
    pub fn generate_spawn_area(&mut self) {
        for cy in -2..=2 {
            for cx in -2..=2 {
                self.get_chunk(cx, cy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_generation_deterministic() {
        let gen = TerrainGenerator::new(12345);
        let chunk1 = gen.generate_chunk(0, 0);
        let chunk2 = gen.generate_chunk(0, 0);
        assert_eq!(chunk1.tiles, chunk2.tiles);
        assert_eq!(chunk1.heights, chunk2.heights);
    }

    #[test]
    fn test_chunk_size() {
        let gen = TerrainGenerator::new(42);
        let chunk = gen.generate_chunk(0, 0);
        assert_eq!(chunk.tiles.len(), (CHUNK_SIZE * CHUNK_SIZE) as usize);
        assert_eq!(chunk.heights.len(), (CHUNK_SIZE * CHUNK_SIZE) as usize);
    }

    #[test]
    fn test_different_seeds_different_terrain() {
        let gen1 = TerrainGenerator::new(1);
        let gen2 = TerrainGenerator::new(2);
        let chunk1 = gen1.generate_chunk(0, 0);
        let chunk2 = gen2.generate_chunk(0, 0);
        assert_ne!(chunk1.tiles, chunk2.tiles);
    }

    #[test]
    fn test_world_terrain_cache() {
        let mut terrain = WorldTerrain::new(42);
        assert_eq!(terrain.cached_chunk_count(), 0);
        terrain.get_chunk(0, 0);
        assert_eq!(terrain.cached_chunk_count(), 1);
        // Accessing same chunk should not increase count
        terrain.get_chunk(0, 0);
        assert_eq!(terrain.cached_chunk_count(), 1);
    }

    #[test]
    fn test_world_terrain_tile_lookup() {
        let mut terrain = WorldTerrain::new(42);
        let tile = terrain.get_tile_at(0, 0);
        // Tile should be a valid type
        assert!(matches!(
            tile,
            TileType::DeepWater
                | TileType::ShallowWater
                | TileType::Sand
                | TileType::Grass
                | TileType::Forest
                | TileType::DenseForest
                | TileType::Hills
                | TileType::Mountain
                | TileType::Snow
                | TileType::Swamp
                | TileType::Desert
                | TileType::Village
        ));
    }

    #[test]
    fn test_tile_walkability() {
        assert!(TileType::Grass.is_walkable());
        assert!(TileType::Forest.is_walkable());
        assert!(TileType::Sand.is_walkable());
        assert!(!TileType::DeepWater.is_walkable());
        assert!(!TileType::Mountain.is_walkable());
        assert!(!TileType::Snow.is_walkable());
    }

    #[test]
    fn test_spawn_area_generation() {
        let mut terrain = WorldTerrain::new(42);
        terrain.generate_spawn_area();
        assert_eq!(terrain.cached_chunk_count(), 25); // 5x5
    }

    #[test]
    fn test_negative_chunk_coords() {
        let gen = TerrainGenerator::new(42);
        let chunk = gen.generate_chunk(-1, -1);
        assert_eq!(chunk.cx, -1);
        assert_eq!(chunk.cy, -1);
        assert_eq!(chunk.tiles.len(), (CHUNK_SIZE * CHUNK_SIZE) as usize);
    }

    #[test]
    fn test_height_values_in_range() {
        let gen = TerrainGenerator::new(42);
        let chunk = gen.generate_chunk(0, 0);
        for h in &chunk.heights {
            assert!(*h >= 0.0 && *h <= 1.0, "Height {} out of range", h);
        }
    }
}
