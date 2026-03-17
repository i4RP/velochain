//! Block producer - creates new blocks at regular intervals.
//!
//! For validators, this runs a loop that:
//! 1. Waits for the block interval
//! 2. Collects pending transactions from the pool
//! 3. Creates a new block with game tick + EVM execution
//! 4. Seals and broadcasts the block

use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};
use velochain_consensus::ConsensusEngine;
use velochain_primitives::Block;

use crate::chain::Chain;
use crate::error::NodeError;

/// Block producer that creates blocks on a timer.
pub struct BlockProducer {
    chain: Arc<Chain>,
    /// Block interval in milliseconds.
    block_interval_ms: u64,
}

impl BlockProducer {
    /// Create a new block producer.
    pub fn new(chain: Arc<Chain>, block_interval_ms: u64) -> Self {
        Self {
            chain,
            block_interval_ms,
        }
    }

    /// Start the block production loop.
    pub async fn start(&self) -> Result<(), NodeError> {
        if !self.chain.consensus().is_validator() {
            info!("Not a validator, block production disabled");
            return Ok(());
        }

        info!(
            "Block producer started, interval={}ms",
            self.block_interval_ms
        );

        let mut ticker = interval(Duration::from_millis(self.block_interval_ms));

        loop {
            ticker.tick().await;

            match self.produce_block() {
                Ok(block) => {
                    info!(
                        "Block produced: number={}, txs={}, game_tick={}, hash={}",
                        block.number(),
                        block.tx_count(),
                        block.header.game_tick,
                        block.hash()
                    );
                }
                Err(e) => {
                    error!("Failed to produce block: {}", e);
                }
            }
        }
    }

    /// Produce a single block.
    fn produce_block(&self) -> Result<Block, NodeError> {
        let parent = self.chain.head().ok_or(NodeError::NotInitialized)?;

        // 1. Prepare block header from consensus
        let header = self.chain.consensus().prepare_header(&parent)?;

        // 2. Collect pending transactions
        let pending_txs = self.chain.txpool().get_pending(1000);
        debug!(
            "Collected {} pending transactions for block {}",
            pending_txs.len(),
            header.number
        );

        // 3. Create block
        let mut block = Block::new(header, pending_txs);

        // 4. Seal block (sign with validator key)
        self.chain.consensus().seal_block(&mut block)?;

        // 5. Apply block to chain
        self.chain.apply_block(&block)?;

        Ok(block)
    }
}
