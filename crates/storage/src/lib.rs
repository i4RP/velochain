//! Storage layer for VeloChain.
//!
//! Provides persistent key-value storage backed by RocksDB,
//! with support for block, transaction, and state data.

pub mod batch;
pub mod cache;
pub mod db;
pub mod error;
pub mod pruning;
pub mod tables;

pub use batch::WriteBatchOps;
pub use cache::ChainCache;
pub use db::Database;
pub use error::StorageError;
pub use pruning::PruningEngine;
