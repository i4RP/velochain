//! Core primitive types for VeloChain.
//!
//! Defines the fundamental data structures used throughout the VeloChain node:
//! blocks, transactions, accounts, and related types.

pub mod account;
pub mod block;
pub mod crypto;
pub mod error;
pub mod genesis;
pub mod transaction;

pub use account::Account;
pub use block::{Block, BlockBody, BlockHeader};
pub use crypto::{recover_signer, Keypair};
pub use error::PrimitivesError;
pub use genesis::Genesis;
pub use transaction::{SignedTransaction, Transaction, TxType};

/// Chain ID for VeloChain (default).
pub const DEFAULT_CHAIN_ID: u64 = 27181;

/// Default block gas limit.
pub const DEFAULT_BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Default game tick interval in milliseconds (200ms = 5 ticks per second).
pub const DEFAULT_TICK_INTERVAL_MS: u64 = 200;

/// Block time in seconds (each block = 1 game tick).
pub const DEFAULT_BLOCK_TIME_SECS: u64 = 1;
