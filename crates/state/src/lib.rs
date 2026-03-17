//! Blockchain state management for VeloChain.
//!
//! Manages the world state including accounts, game state,
//! and state transitions when blocks are applied.

pub mod error;
pub mod world_state;

pub use error::StateError;
pub use world_state::WorldState;
