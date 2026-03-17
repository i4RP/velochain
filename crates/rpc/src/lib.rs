//! JSON-RPC server for VeloChain.
//!
//! Provides Ethereum-compatible JSON-RPC endpoints plus
//! game-specific endpoints for querying world state.

pub mod error;
pub mod server;
pub mod eth_api;
pub mod game_api;

pub use error::RpcError;
pub use server::RpcServer;
