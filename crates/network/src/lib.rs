//! P2P networking for VeloChain.
//!
//! Handles peer discovery, block propagation, and transaction gossip
//! using libp2p with gossipsub for message broadcasting.

pub mod error;
pub mod service;
pub mod protocol;

pub use error::NetworkError;
pub use service::{NetworkService, NetworkEvent};

// Re-export libp2p types used by consumers
pub use libp2p::Multiaddr;
