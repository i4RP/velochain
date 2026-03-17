//! VeloChain node orchestrator.
//!
//! Coordinates all subsystems: consensus, game engine, EVM, networking,
//! transaction pool, storage, and RPC server.

pub mod config;
pub mod config_file;
pub mod error;
pub mod chain;
pub mod block_producer;
pub mod logging;
pub mod metrics;
pub mod reorg;
pub mod shutdown;
pub mod snapshot;

pub use config::NodeConfig;
pub use config_file::ConfigFile;
pub use error::NodeError;
pub use chain::{Chain, TransactionReceipt, ReceiptLog};
pub use block_producer::BlockProducer;
pub use logging::{LogConfig, init_logging};
pub use metrics::NodeMetrics;
pub use shutdown::ShutdownController;
pub use snapshot::{SnapshotMeta, export_snapshot, import_snapshot, read_snapshot_meta};
