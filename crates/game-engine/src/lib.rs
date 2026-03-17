//! Deterministic game engine for VeloChain.
//!
//! Runs one game tick per block, processing player actions and
//! updating the world state deterministically. Inspired by Veloren's
//! ECS architecture but adapted for blockchain consensus.

pub mod combat;
pub mod ecs;
pub mod error;
pub mod events;
pub mod game_api_types;
pub mod items;
pub mod npc_ai;
pub mod systems;
pub mod terrain;
pub mod world;

pub use combat::CombatStats;
pub use error::GameEngineError;
pub use events::EventManager;
pub use game_api_types::{EntitySnapshot, PlayerInfo};
pub use items::ItemRegistry;
pub use npc_ai::SpawnManager;
pub use terrain::WorldTerrain;
pub use world::GameWorld;
