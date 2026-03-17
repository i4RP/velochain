//! Shared types for game API responses.
//!
//! These types are used by both the game engine and the RPC layer
//! to avoid circular dependencies.

use serde::{Deserialize, Serialize};

/// Player information returned by RPC queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub entity_id: u64,
    pub address: String,
    pub position: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    pub level: u32,
    pub is_alive: bool,
}

/// Entity snapshot returned by area-based queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub entity_type: String,
    pub position: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<[f32; 3]>,
}
