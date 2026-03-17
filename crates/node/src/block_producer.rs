//! Block producer - creates new blocks at regular intervals.
//!
//! For validators, this runs a loop that:
//! 1. Waits for the block interval
//! 2. Collects pending transactions from the pool
//! 3. Creates a new block with game tick + EVM execution
//! 4. Seals and broadcasts the block

use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use velochain_consensus::ConsensusEngine;
use velochain_network::NetworkService;
use velochain_primitives::Block;

use crate::chain::Chain;
use crate::error::NodeError;

/// Block producer that creates blocks on a timer.
pub struct BlockProducer {
    chain: Arc<Chain>,
    /// Block interval in milliseconds.
    block_interval_ms: u64,
    /// Optional network service for broadcasting blocks.
    network: Option<Arc<NetworkService>>,
}

impl BlockProducer {
    /// Create a new block producer.
    pub fn new(chain: Arc<Chain>, block_interval_ms: u64) -> Self {
        Self {
            chain,
            block_interval_ms,
            network: None,
        }
    }

    /// Set the network service for block broadcasting.
    pub fn with_network(mut self, network: Arc<NetworkService>) -> Self {
        self.network = Some(network);
        self
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

                    // Broadcast block to network peers
                    if let Some(ref network) = self.network {
                        if let Err(e) = network.broadcast_block(block) {
                            warn!("Failed to broadcast block: {}", e);
                        }
                    }
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
        let mut header = self.chain.consensus().prepare_header(&parent)?;

        // 2. Collect pending transactions (up to gas limit)
        let pending_txs = self.chain.txpool().get_pending(1000);
        let mut selected_txs = Vec::new();
        let mut gas_used: u64 = 0;
        for tx in pending_txs {
            let tx_gas = if tx.is_game_action() {
                21_000
            } else {
                tx.transaction.gas_limit
            };
            if gas_used + tx_gas > header.gas_limit {
                break;
            }
            gas_used += tx_gas;
            selected_txs.push(tx);
        }
        debug!(
            "Selected {} transactions for block {} (gas_used={})",
            selected_txs.len(),
            header.number,
            gas_used
        );

        // 3. Create block and compute transactions root
        let mut block = Block::new(header.clone(), selected_txs);
        header.transactions_root = block.compute_transactions_root();
        header.gas_used = gas_used;
        block.header = header;

        // 4. Seal block (sign with validator key)
        self.chain.consensus().seal_block(&mut block)?;

        // 5. Apply block to chain (computes state roots, stores receipts)
        self.chain.apply_block(&block)?;

        Ok(block)
    }
}
