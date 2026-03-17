//! Deterministic game engine for VeloChain.
//!
//! Runs one game tick per block, processing player actions and
//! updating the world state deterministically. Inspired by Veloren's
//! ECS architecture but adapted for blockchain consensus.

pub mod error;
pub mod world;
pub mod systems;
pub mod ecs;

pub use error::GameEngineError;
pub use world::GameWorld;
