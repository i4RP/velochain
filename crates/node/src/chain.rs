//! Chain management - tracks the canonical chain and processes blocks.

use std::sync::Arc;
use tracing::{debug, info, warn};
use velochain_consensus::{ConsensusEngine, PoaConsensus};
use velochain_evm::EvmExecutor;
use velochain_game_engine::GameWorld;
use velochain_primitives::{Block, BlockHeader};
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

use crate::error::NodeError;

/// The blockchain state and chain management.
pub struct Chain {
    /// Persistent storage.
    db: Arc<Database>,
    /// World state.
    state: Arc<WorldState>,
    /// Game world.
    game_world: Arc<GameWorld>,
    /// Transaction pool.
    txpool: Arc<TransactionPool>,
    /// Consensus engine.
    consensus: Arc<PoaConsensus>,
    /// EVM executor.
    evm: parking_lot::Mutex<EvmExecutor>,
    /// Current chain head.
    head: parking_lot::RwLock<Option<BlockHeader>>,
    /// Chain ID.
    chain_id: u64,
}

impl Chain {
    /// Create a new chain instance.
    pub fn new(
        db: Arc<Database>,
        state: Arc<WorldState>,
        game_world: Arc<GameWorld>,
        txpool: Arc<TransactionPool>,
        consensus: Arc<PoaConsensus>,
        chain_id: u64,
    ) -> Self {
        Self {
            db,
            state,
            game_world,
            txpool,
            consensus,
            evm: parking_lot::Mutex::new(EvmExecutor::new(chain_id)),
            head: parking_lot::RwLock::new(None),
            chain_id,
        }
    }

    /// Initialize the chain with the genesis block.
    pub fn init_genesis(&self, genesis_header: BlockHeader) -> Result<(), NodeError> {
        let genesis_block = Block::new(genesis_header.clone(), vec![]);

        // Store genesis block
        self.db.put_block(&genesis_block)?;
        self.db.put_latest_block_number(0)?;

        // Set chain head
        *self.head.write() = Some(genesis_header.clone());

        info!(
            "Genesis block initialized: hash={}, game_tick=0",
            genesis_block.hash()
        );

        Ok(())
    }

    /// Get the current chain head.
    pub fn head(&self) -> Option<BlockHeader> {
        self.head.read().clone()
    }

    /// Get the current block number.
    pub fn block_number(&self) -> u64 {
        self.head
            .read()
            .as_ref()
            .map(|h| h.number)
            .unwrap_or(0)
    }

    /// Process and apply a new block.
    pub fn apply_block(&self, block: &Block) -> Result<(), NodeError> {
        let parent = self.head().ok_or(NodeError::NotInitialized)?;

        // 1. Verify consensus rules
        self.consensus.verify_block(block, &parent)?;

        // 2. Execute EVM transactions
        let mut evm_txs = Vec::new();
        let mut game_actions = Vec::new();

        for tx in &block.body.transactions {
            if tx.is_game_action() {
                if let Some(action) = tx.game_action() {
                    // Use a placeholder address since sender recovery isn't implemented yet
                    game_actions.push(("0x0000000000000000000000000000000000000000".to_string(), action.clone()));
                }
            } else {
                evm_txs.push(tx.clone());
            }
        }

        // Execute EVM transactions
        {
            let mut evm = self.evm.lock();
            for tx in &evm_txs {
                match evm.execute_tx(tx) {
                    Ok(outcome) => {
                        debug!(
                            "EVM tx executed: hash={}, success={}, gas_used={}",
                            tx.hash, outcome.success, outcome.gas_used
                        );
                    }
                    Err(e) => {
                        warn!("EVM tx failed: hash={}, error={}", tx.hash, e);
                    }
                }
            }
        }

        // 3. Run game tick with player actions
        let _game_state_root = self.game_world.tick(&game_actions)?;

        // 4. Commit state changes
        let state_root = self.state.commit()?;

        // 5. Store block
        self.db.put_block(block)?;
        self.db.put_latest_block_number(block.number())?;

        // 6. Remove included transactions from pool
        let tx_hashes: Vec<_> = block
            .body
            .transactions
            .iter()
            .map(|tx| tx.hash)
            .collect();
        self.txpool.remove_included(&tx_hashes);

        // 7. Update chain head
        *self.head.write() = Some(block.header.clone());

        info!(
            "Block applied: number={}, txs={}, game_tick={}, state_root={}",
            block.number(),
            block.tx_count(),
            block.header.game_tick,
            state_root
        );

        Ok(())
    }

    /// Get references to subsystems.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn state(&self) -> &Arc<WorldState> {
        &self.state
    }

    pub fn game_world(&self) -> &Arc<GameWorld> {
        &self.game_world
    }

    pub fn txpool(&self) -> &Arc<TransactionPool> {
        &self.txpool
    }

    pub fn consensus(&self) -> &Arc<PoaConsensus> {
        &self.consensus
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}
