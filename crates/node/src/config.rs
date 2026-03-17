//! Node configuration.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use velochain_primitives::Genesis;

/// Complete node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Data directory for storage.
    pub data_dir: PathBuf,
    /// Genesis configuration.
    pub genesis: Genesis,
    /// RPC server bind address.
    pub rpc_addr: SocketAddr,
    /// P2P listen address.
    pub p2p_addr: String,
    /// Boot node addresses.
    pub boot_nodes: Vec<String>,
    /// Whether this node is a validator.
    pub is_validator: bool,
    /// Validator private key (hex, for signing blocks).
    /// WARNING: In production, this should come from a secure keystore.
    pub validator_key: Option<String>,
    /// Maximum peers.
    pub max_peers: usize,
    /// Log level.
    pub log_level: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./velochain-data"),
            genesis: Genesis::default(),
            rpc_addr: "127.0.0.1:8545".parse().unwrap(),
            p2p_addr: "/ip4/0.0.0.0/tcp/30303".to_string(),
            boot_nodes: Vec::new(),
            is_validator: false,
            validator_key: None,
            max_peers: 50,
            log_level: "info".to_string(),
        }
    }
}
