//! TOML configuration file support with environment variable overrides.
//!
//! Loads node configuration from a TOML file, with the ability to override
//! any setting via environment variables prefixed with `VELOCHAIN_`.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::NodeError;

/// TOML-friendly node configuration.
///
/// All fields are optional; missing fields use defaults from `NodeConfig`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Node section.
    #[serde(default)]
    pub node: NodeSection,
    /// RPC section.
    #[serde(default)]
    pub rpc: RpcSection,
    /// Network section.
    #[serde(default)]
    pub network: NetworkSection,
    /// Validator section.
    #[serde(default)]
    pub validator: ValidatorSection,
    /// Logging section.
    #[serde(default)]
    pub logging: LoggingSection,
}

/// Node-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSection {
    /// Data directory path.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Path to genesis.json.
    #[serde(default)]
    pub genesis_file: Option<PathBuf>,
}

impl Default for NodeSection {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            genesis_file: None,
        }
    }
}

/// RPC configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcSection {
    /// RPC listen address.
    #[serde(default = "default_rpc_addr")]
    pub addr: String,
    /// Maximum WebSocket connections.
    #[serde(default = "default_max_ws")]
    pub max_ws_connections: u32,
    /// Enable admin API.
    #[serde(default)]
    pub enable_admin: bool,
}

impl Default for RpcSection {
    fn default() -> Self {
        Self {
            addr: default_rpc_addr(),
            max_ws_connections: default_max_ws(),
            enable_admin: false,
        }
    }
}

/// P2P network configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSection {
    /// P2P listen address (multiaddr format).
    #[serde(default = "default_p2p_addr")]
    pub listen_addr: String,
    /// Boot node multiaddresses.
    #[serde(default)]
    pub bootnodes: Vec<String>,
    /// Maximum peer connections.
    #[serde(default = "default_max_peers")]
    pub max_peers: usize,
}

impl Default for NetworkSection {
    fn default() -> Self {
        Self {
            listen_addr: default_p2p_addr(),
            bootnodes: Vec::new(),
            max_peers: default_max_peers(),
        }
    }
}

/// Validator configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidatorSection {
    /// Whether this node acts as a validator.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the keystore file.
    #[serde(default)]
    pub keystore_file: Option<PathBuf>,
    /// Direct hex-encoded private key (not recommended for production).
    #[serde(default)]
    pub private_key: Option<String>,
}

/// Logging configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSection {
    /// Global log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Per-module log level overrides (e.g., "velochain_rpc=debug").
    #[serde(default)]
    pub modules: Vec<String>,
    /// Whether to include timestamps in log output.
    #[serde(default = "default_true")]
    pub timestamps: bool,
    /// Whether to use JSON-formatted log output.
    #[serde(default)]
    pub json: bool,
}

impl Default for LoggingSection {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            modules: Vec::new(),
            timestamps: true,
            json: false,
        }
    }
}

// Default value functions
fn default_data_dir() -> PathBuf {
    PathBuf::from("./velochain-data")
}
fn default_rpc_addr() -> String {
    "127.0.0.1:8545".to_string()
}
fn default_max_ws() -> u32 {
    100
}
fn default_p2p_addr() -> String {
    "/ip4/0.0.0.0/tcp/30303".to_string()
}
fn default_max_peers() -> usize {
    50
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_true() -> bool {
    true
}

impl ConfigFile {
    /// Load configuration from a TOML file.
    pub fn load(path: &Path) -> Result<Self, NodeError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| NodeError::Config(format!("Failed to read config file: {e}")))?;
        let config: Self = toml::from_str(&contents)
            .map_err(|e| NodeError::Config(format!("Failed to parse TOML config: {e}")))?;
        info!("Configuration loaded from {:?}", path);
        Ok(config)
    }

    /// Apply environment variable overrides.
    ///
    /// Environment variables follow the pattern `VELOCHAIN_SECTION_KEY`.
    /// For example:
    /// - `VELOCHAIN_NODE_DATA_DIR` → node.data_dir
    /// - `VELOCHAIN_RPC_ADDR` → rpc.addr
    /// - `VELOCHAIN_VALIDATOR_ENABLED` → validator.enabled
    /// - `VELOCHAIN_LOGGING_LEVEL` → logging.level
    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("VELOCHAIN_NODE_DATA_DIR") {
            self.node.data_dir = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("VELOCHAIN_RPC_ADDR") {
            self.rpc.addr = val;
        }
        if let Ok(val) = std::env::var("VELOCHAIN_RPC_MAX_WS") {
            if let Ok(n) = val.parse() {
                self.rpc.max_ws_connections = n;
            }
        }
        if let Ok(val) = std::env::var("VELOCHAIN_RPC_ENABLE_ADMIN") {
            self.rpc.enable_admin = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("VELOCHAIN_NETWORK_LISTEN") {
            self.network.listen_addr = val;
        }
        if let Ok(val) = std::env::var("VELOCHAIN_NETWORK_MAX_PEERS") {
            if let Ok(n) = val.parse() {
                self.network.max_peers = n;
            }
        }
        if let Ok(val) = std::env::var("VELOCHAIN_VALIDATOR_ENABLED") {
            self.validator.enabled = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("VELOCHAIN_VALIDATOR_KEY") {
            self.validator.private_key = Some(val);
        }
        if let Ok(val) = std::env::var("VELOCHAIN_LOG_LEVEL") {
            self.logging.level = val;
        }
        if let Ok(val) = std::env::var("VELOCHAIN_LOG_JSON") {
            self.logging.json = val == "1" || val.eq_ignore_ascii_case("true");
        }
    }

    /// Get the RPC socket address (parsed).
    pub fn rpc_socket_addr(&self) -> Result<SocketAddr, NodeError> {
        self.rpc
            .addr
            .parse()
            .map_err(|e| NodeError::Config(format!("Invalid RPC address '{}': {e}", self.rpc.addr)))
    }

    /// Build the tracing filter string from logging configuration.
    pub fn log_filter_string(&self) -> String {
        let mut filter = self.logging.level.clone();
        for module_filter in &self.logging.modules {
            filter.push(',');
            filter.push_str(module_filter);
        }
        filter
    }

    /// Write a default configuration file to the given path.
    pub fn write_default(path: &Path) -> Result<(), NodeError> {
        let default = Self::default();
        let toml_str = toml::to_string_pretty(&default)
            .map_err(|e| NodeError::Config(format!("Failed to serialize default config: {e}")))?;
        std::fs::write(path, toml_str)
            .map_err(|e| NodeError::Config(format!("Failed to write config file: {e}")))?;
        info!("Default configuration written to {:?}", path);
        Ok(())
    }
}
