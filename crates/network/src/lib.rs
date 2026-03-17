//! P2P networking for VeloChain.
//!
//! Handles peer discovery, block propagation, and transaction gossip
//! using libp2p with gossipsub for message broadcasting.

pub mod error;
pub mod peer_manager;
pub mod protocol;
pub mod service;
pub mod sync;
#[cfg(test)]
mod multinode_tests;

pub use error::NetworkError;
pub use peer_manager::PeerManager;
pub use service::{NetworkService, NetworkEvent};
pub use sync::SyncEngine;

// Re-export libp2p types used by consumers
pub use libp2p::Multiaddr;
