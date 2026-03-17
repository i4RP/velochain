//! VeloChain node orchestrator.
//!
//! Coordinates all subsystems: consensus, game engine, EVM, networking,
//! transaction pool, storage, and RPC server.

pub mod config;
pub mod error;
pub mod chain;
pub mod block_producer;

pub use config::NodeConfig;
pub use error::NodeError;
pub use chain::Chain;
pub use block_producer::BlockProducer;
