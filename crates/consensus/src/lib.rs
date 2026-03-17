//! Consensus engine for VeloChain.
//!
//! Implements Proof-of-Authority (PoA / Clique) consensus where
//! each block corresponds to one game tick.

pub mod error;
pub mod poa;
#[cfg(test)]
mod tests;

pub use error::ConsensusError;
pub use poa::PoaConsensus;

use async_trait::async_trait;
use velochain_primitives::{Block, BlockHeader};

/// Trait defining a consensus engine.
#[async_trait]
pub trait ConsensusEngine: Send + Sync {
    /// Verify that a block header is valid according to consensus rules.
    fn verify_header(
        &self,
        header: &BlockHeader,
        parent: &BlockHeader,
    ) -> Result<(), ConsensusError>;

    /// Verify that a complete block is valid.
    fn verify_block(&self, block: &Block, parent: &BlockHeader) -> Result<(), ConsensusError> {
        self.verify_header(&block.header, parent)?;
        Ok(())
    }

    /// Check if we are currently a validator and should produce blocks.
    fn is_validator(&self) -> bool;

    /// Prepare a new block header for proposal.
    fn prepare_header(&self, parent: &BlockHeader) -> Result<BlockHeader, ConsensusError>;

    /// Seal (sign) a block that we produced.
    fn seal_block(&self, block: &mut Block) -> Result<(), ConsensusError>;

    /// Get the current validator set.
    fn validators(&self) -> Vec<alloy_primitives::Address>;
}
