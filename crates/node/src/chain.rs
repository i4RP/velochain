//! Chain management - tracks the canonical chain and processes blocks.

use alloy_primitives::{Address, B256};
use serde::{Deserialize, Serialize};
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

/// A transaction receipt recording the outcome of a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Transaction hash.
    pub tx_hash: B256,
    /// Block number the transaction was included in.
    pub block_number: u64,
    /// Block hash the transaction was included in.
    pub block_hash: B256,
    /// Index of the transaction within the block.
    pub index: u32,
    /// Whether the transaction executed successfully.
    pub success: bool,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Cumulative gas used in the block up to and including this transaction.
    pub cumulative_gas_used: u64,
    /// Contract address created (if any).
    pub contract_address: Option<Address>,
    /// Logs emitted by the transaction.
    pub logs: Vec<ReceiptLog>,
}

/// A log entry within a transaction receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptLog {
    /// Address of the contract that emitted the log.
    pub address: Address,
    /// Log topics.
    pub topics: Vec<B256>,
    /// Log data.
    pub data: Vec<u8>,
}

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

    /// Restore the chain head from the database (call on node startup).
    pub fn restore_head(&self) -> Result<(), NodeError> {
        if let Some(number) = self.db.get_latest_block_number()? {
            if let Some(hash) = self.db.get_block_hash_by_number(number)? {
                if let Some(header) = self.db.get_header(&hash)? {
                    info!("Restored chain head: block={}, hash=0x{}", number, hex::encode(hash));
                    *self.head.write() = Some(header);
                    return Ok(());
                }
            }
        }
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
        let mut total_gas_used: u64 = 0;

        for tx in &block.body.transactions {
            if tx.is_game_action() {
                if let Some(action) = tx.game_action() {
                    let sender = tx.sender().map_err(|e| NodeError::Internal(format!("Failed to recover sender: {e}")))?;
                    game_actions.push((format!("{:?}", sender), action.clone()));
                    // Game actions use a fixed gas amount
                    total_gas_used += 21_000;
                }
            } else {
                evm_txs.push(tx.clone());
            }
        }

        // Execute EVM transactions
        let mut receipts: Vec<TransactionReceipt> = Vec::new();
        {
            let mut evm = self.evm.lock();
            evm.reset(); // Clean slate for this block

            // Load sender accounts into EVM before execution
            for tx in &evm_txs {
                if let Ok(sender) = tx.sender() {
                    evm.load_account(sender, &self.state);
                }
                if let Some(to) = tx.transaction.to {
                    evm.load_account(to, &self.state);
                }
            }

            for tx in &evm_txs {
                let sender = tx.sender().map_err(|e| NodeError::Internal(format!("Sender recovery: {e}")))?;

                // Nonce verification: sender nonce must match tx nonce
                let expected_nonce = self.state.get_nonce(&sender)
                    .map_err(|e| NodeError::Internal(format!("Nonce read: {e}")))?;
                if tx.transaction.nonce != expected_nonce {
                    warn!(
                        "Nonce mismatch: tx={} expected={} got={}, skipping",
                        tx.hash, expected_nonce, tx.transaction.nonce
                    );
                    receipts.push(TransactionReceipt {
                        tx_hash: tx.hash,
                        block_number: block.number(),
                        block_hash: B256::ZERO,
                        index: receipts.len() as u32,
                        success: false,
                        gas_used: 0,
                        cumulative_gas_used: total_gas_used,
                        contract_address: None,
                        logs: vec![],
                    });
                    continue;
                }

                match evm.execute_tx(tx) {
                    Ok(outcome) => {
                        total_gas_used += outcome.gas_used;

                        // Deduct gas cost from sender balance
                        let gas_price = tx.transaction.gas_price.unwrap_or(1);
                        let gas_cost = alloy_primitives::U256::from(outcome.gas_used) * alloy_primitives::U256::from(gas_price);
                        if let Err(e) = self.state.sub_balance(&sender, gas_cost) {
                            debug!("Gas deduction failed for {}: {}", sender, e);
                        }

                        // Increment sender nonce
                        if let Err(e) = self.state.increment_nonce(&sender) {
                            warn!("Nonce increment failed for {}: {}", sender, e);
                        }

                        debug!(
                            "EVM tx executed: hash={}, success={}, gas_used={}",
                            tx.hash, outcome.success, outcome.gas_used
                        );
                        receipts.push(TransactionReceipt {
                            tx_hash: tx.hash,
                            block_number: block.number(),
                            block_hash: B256::ZERO,
                            index: receipts.len() as u32,
                            success: outcome.success,
                            gas_used: outcome.gas_used,
                            cumulative_gas_used: total_gas_used,
                            contract_address: outcome.contract_address,
                            logs: outcome.logs.iter().map(|l| ReceiptLog {
                                address: l.address,
                                topics: l.topics.clone(),
                                data: l.data.clone(),
                            }).collect(),
                        });
                    }
                    Err(e) => {
                        warn!("EVM tx failed: hash={}, error={}", tx.hash, e);
                        // Still increment nonce on failure (Ethereum behavior)
                        if let Err(e) = self.state.increment_nonce(&sender) {
                            warn!("Nonce increment failed for {}: {}", sender, e);
                        }
                        receipts.push(TransactionReceipt {
                            tx_hash: tx.hash,
                            block_number: block.number(),
                            block_hash: B256::ZERO,
                            index: receipts.len() as u32,
                            success: false,
                            gas_used: 0,
                            cumulative_gas_used: total_gas_used,
                            contract_address: None,
                            logs: vec![],
                        });
                    }
                }
            }

            // Flush EVM state changes back to WorldState
            if let Err(e) = evm.flush_to_state(&self.state) {
                warn!("Failed to flush EVM state: {}", e);
            }
        }

        // Process game action transactions: nonce check, gas deduct, create receipts
        let mut game_cumulative_gas: u64 = total_gas_used;
        for tx in &block.body.transactions {
            if tx.is_game_action() {
                let sender = tx.sender().map_err(|e| NodeError::Internal(format!("Sender recovery: {e}")))?;

                // Nonce verification for game actions
                let expected_nonce = self.state.get_nonce(&sender)
                    .map_err(|e| NodeError::Internal(format!("Nonce read: {e}")))?;
                let nonce_ok = tx.transaction.nonce == expected_nonce;

                if nonce_ok {
                    // Increment nonce
                    if let Err(e) = self.state.increment_nonce(&sender) {
                        warn!("Game action nonce increment failed for {}: {}", sender, e);
                    }
                    // Deduct fixed gas cost
                    let gas_cost = alloy_primitives::U256::from(21_000u64);
                    if let Err(e) = self.state.sub_balance(&sender, gas_cost) {
                        debug!("Game action gas deduction failed for {}: {}", sender, e);
                    }
                }

                game_cumulative_gas += 21_000;
                receipts.push(TransactionReceipt {
                    tx_hash: tx.hash,
                    block_number: block.number(),
                    block_hash: B256::ZERO,
                    index: receipts.len() as u32,
                    success: nonce_ok,
                    gas_used: 21_000,
                    cumulative_gas_used: game_cumulative_gas,
                    contract_address: None,
                    logs: vec![],
                });
            }
        }

        // 3. Run game tick with player actions
        let game_state_root = self.game_world.tick(&game_actions)?;

        // 4. Persist game world state to database
        if let Ok(game_data) = self.game_world.serialize_state() {
            if let Err(e) = self.db.put_game_state(b"world", &game_data) {
                warn!("Failed to persist game state: {}", e);
            }
        }

        // 5. Commit account state changes to persistent storage
        let state_root = self.state.commit()?;

        // 6. Store block
        let block_hash = block.hash();
        self.db.put_block(block)?;
        self.db.put_latest_block_number(block.number())?;

        // 6b. Store receipts with correct block hash
        for receipt in &mut receipts {
            receipt.block_hash = block_hash;
        }
        self.store_receipts(&receipts)?;

        // 7. Remove included transactions from pool
        let tx_hashes: Vec<_> = block
            .body
            .transactions
            .iter()
            .map(|tx| tx.hash)
            .collect();
        self.txpool.remove_included(&tx_hashes);

        // 8. Update chain head
        *self.head.write() = Some(block.header.clone());

        info!(
            "Block applied: number={}, txs={}, gas_used={}, game_tick={}, state_root={}, game_root={}",
            block.number(),
            block.tx_count(),
            total_gas_used,
            block.header.game_tick,
            state_root,
            game_state_root
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

    /// Forcefully set the chain head to a specific block header.
    /// Used during chain reorganizations.
    pub fn set_head(&self, header: BlockHeader) -> Result<(), NodeError> {
        let number = header.number;
        self.db.put_latest_block_number(number)?;
        *self.head.write() = Some(header);
        tracing::info!("Chain head set to block {}", number);
        Ok(())
    }

    /// Store transaction receipts to the database.
    fn store_receipts(&self, receipts: &[TransactionReceipt]) -> Result<(), NodeError> {
        for receipt in receipts {
            let data = serde_json::to_vec(receipt)
                .map_err(|e| NodeError::Internal(format!("Receipt serialization: {e}")))?;
            self.db.put_receipt(receipt.tx_hash.as_ref(), &data)?;
        }
        Ok(())
    }

    /// Get a transaction receipt by transaction hash.
    pub fn get_receipt(&self, tx_hash: &B256) -> Result<Option<TransactionReceipt>, NodeError> {
        let hash_bytes: &[u8; 32] = tx_hash.as_ref();
        match self.db.get_receipt(hash_bytes)? {
            Some(data) => {
                let receipt: TransactionReceipt = serde_json::from_slice(&data)
                    .map_err(|e| NodeError::Internal(format!("Receipt deserialization: {e}")))?;
                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }

    /// Get a block by number.
    pub fn get_block_by_number(&self, number: u64) -> Result<Option<Block>, NodeError> {
        let hash = match self.db.get_block_hash_by_number(number)? {
            Some(h) => h,
            None => return Ok(None),
        };
        self.get_block_by_hash(&B256::from(hash))
    }

    /// Get a block by hash.
    pub fn get_block_by_hash(&self, hash: &B256) -> Result<Option<Block>, NodeError> {
        let hash_bytes: [u8; 32] = hash.0;
        let header = match self.db.get_header(&hash_bytes)? {
            Some(h) => h,
            None => return Ok(None),
        };
        let body = match self.db.get_body(&hash_bytes)? {
            Some(b) => b,
            None => return Ok(None),
        };
        Ok(Some(Block { header, body }))
    }
}
