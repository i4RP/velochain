//! P2P networking for VeloChain.
//!
//! Handles peer discovery, block propagation, and transaction gossip
//! using libp2p with gossipsub for message broadcasting.

pub mod error;
#[cfg(test)]
mod multinode_tests;
pub mod peer_manager;
pub mod protocol;
pub mod service;
pub mod sync;

pub use error::NetworkError;
pub use peer_manager::PeerManager;
pub use service::{NetworkEvent, NetworkService};
pub use sync::SyncEngine;

// Re-export libp2p types used by consumers
pub use libp2p::Multiaddr;
