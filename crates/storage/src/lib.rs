//! Storage layer for VeloChain.
//!
//! Provides persistent key-value storage backed by RocksDB,
//! with support for block, transaction, and state data.

pub mod db;
pub mod error;
pub mod tables;

pub use db::Database;
pub use error::StorageError;
