//! Core primitive types for VeloChain.
//!
//! Defines the fundamental data structures used throughout the VeloChain node:
//! blocks, transactions, accounts, and related types.

pub mod block;
pub mod transaction;
pub mod account;
pub mod genesis;
pub mod error;

pub use block::{Block, BlockHeader, BlockBody};
pub use transaction::{Transaction, TxType, SignedTransaction};
pub use account::Account;
pub use genesis::Genesis;
pub use error::PrimitivesError;

/// Chain ID for VeloChain (default).
pub const DEFAULT_CHAIN_ID: u64 = 27181;

/// Default block gas limit.
pub const DEFAULT_BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Default game tick interval in milliseconds (200ms = 5 ticks per second).
pub const DEFAULT_TICK_INTERVAL_MS: u64 = 200;

/// Block time in seconds (each block = 1 game tick).
pub const DEFAULT_BLOCK_TIME_SECS: u64 = 1;
