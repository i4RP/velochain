//! EVM execution engine for VeloChain.
//!
//! Provides EVM execution for asset management (token transfers, NFTs)
//! via the revm library. Game logic runs natively in Rust;
//! the EVM is only used for economic/asset operations.

pub mod error;
pub mod executor;
#[cfg(test)]
mod tests;

pub use error::EvmError;
pub use executor::EvmExecutor;
