//! JSON-RPC server for VeloChain.
//!
//! Provides Ethereum-compatible JSON-RPC endpoints plus
//! game-specific endpoints for querying world state.
//! Supports both HTTP and WebSocket connections for real-time
//! game state streaming via subscriptions.

pub mod admin_api;
pub mod error;
pub mod eth_api;
pub mod game_api;
pub mod server;
pub mod session;
pub mod subscriptions;

pub use error::RpcError;
pub use server::RpcServer;
pub use session::SessionManager;
pub use subscriptions::{new_event_channel, EventSender, GameEvent};
