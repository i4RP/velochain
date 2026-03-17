//! JSON-RPC server for VeloChain.
//!
//! Provides Ethereum-compatible JSON-RPC endpoints plus
//! game-specific endpoints for querying world state.
//! Supports both HTTP and WebSocket connections for real-time
//! game state streaming via subscriptions.

pub mod error;
pub mod server;
pub mod eth_api;
pub mod game_api;
pub mod admin_api;
pub mod subscriptions;
pub mod session;

pub use error::RpcError;
pub use server::RpcServer;
pub use session::SessionManager;
pub use subscriptions::{EventSender, GameEvent, new_event_channel};
