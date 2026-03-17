//! Transaction pool implementation.
//!
//! Manages pending and queued transactions with nonce validation
//! and gas-price-based ordering for block inclusion.

use crate::error::TxPoolError;
use alloy_primitives::{Address, B256};
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, info};
use velochain_primitives::SignedTransaction;

/// Maximum number of transactions in the pool.
const DEFAULT_MAX_POOL_SIZE: usize = 10_000;

/// Per-sender transaction queue, ordered by nonce.
#[derive(Default)]
struct SenderQueue {
    /// Transactions ordered by nonce.
    txs: BTreeMap<u64, SignedTransaction>,
}

/// Transaction pool managing pending and queued transactions.
pub struct TransactionPool {
    /// All transactions indexed by hash.
    by_hash: Arc<DashMap<B256, SignedTransaction>>,
    /// Transactions grouped by sender, ordered by nonce.
    by_sender: Arc<DashMap<Address, SenderQueue>>,
    /// Maximum pool size.
    max_size: usize,
}

impl TransactionPool {
    /// Create a new transaction pool.
    pub fn new() -> Self {
        Self {
            by_hash: Arc::new(DashMap::new()),
            by_sender: Arc::new(DashMap::new()),
            max_size: DEFAULT_MAX_POOL_SIZE,
        }
    }

    /// Create a new transaction pool with custom max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            by_hash: Arc::new(DashMap::new()),
            by_sender: Arc::new(DashMap::new()),
            max_size,
        }
    }

    /// Add a transaction to the pool.
    ///
    /// Validates signature, checks for duplicates, and inserts into the
    /// sender-ordered queue.
    pub fn add_transaction(&self, tx: SignedTransaction) -> Result<B256, TxPoolError> {
        let hash = tx.hash;

        // Check if already exists
        if self.by_hash.contains_key(&hash) {
            return Err(TxPoolError::AlreadyExists(format!("{}", hash)));
        }

        // Check pool capacity
        if self.by_hash.len() >= self.max_size {
            return Err(TxPoolError::PoolFull {
                capacity: self.max_size,
                current: self.by_hash.len(),
            });
        }

        // Recover sender from signature
        let sender = tx
            .sender()
            .map_err(|e| TxPoolError::InvalidTransaction(format!("Invalid signature: {e}")))?;

        let nonce = tx.transaction.nonce;

        // Check for nonce collision within same sender
        if let Some(queue) = self.by_sender.get(&sender) {
            if queue.txs.contains_key(&nonce) {
                return Err(TxPoolError::InvalidTransaction(format!(
                    "Nonce {} already used by sender {:?}",
                    nonce, sender
                )));
            }
        }

        // Insert into hash index
        self.by_hash.insert(hash, tx.clone());

        // Insert into sender queue
        self.by_sender
            .entry(sender)
            .or_default()
            .txs
            .insert(nonce, tx);

        debug!(
            "Transaction added to pool: hash={}, sender={:?}, nonce={}",
            hash, sender, nonce
        );

        Ok(hash)
    }

    /// Remove a transaction from the pool by hash.
    pub fn remove_transaction(&self, hash: &B256) -> Option<SignedTransaction> {
        if let Some((_, tx)) = self.by_hash.remove(hash) {
            // Also remove from sender queue
            if let Ok(sender) = tx.sender() {
                if let Some(mut queue) = self.by_sender.get_mut(&sender) {
                    queue.txs.remove(&tx.transaction.nonce);
                    if queue.txs.is_empty() {
                        drop(queue);
                        self.by_sender.remove(&sender);
                    }
                }
            }
            Some(tx)
        } else {
            None
        }
    }

    /// Get pending transactions for block inclusion.
    ///
    /// Returns up to `max_count` transactions. For each sender, transactions
    /// are ordered by nonce (ascending). Across senders, highest gas price first.
    pub fn get_pending(&self, max_count: usize) -> Vec<SignedTransaction> {
        let mut all_txs: Vec<SignedTransaction> = Vec::new();

        // Collect transactions with contiguous nonces per sender
        for entry in self.by_sender.iter() {
            let queue = entry.value();
            let mut expected_nonce: Option<u64> = None;
            for (&nonce, tx) in &queue.txs {
                match expected_nonce {
                    Some(expected) if nonce != expected => break,
                    _ => {
                        expected_nonce = Some(nonce + 1);
                        all_txs.push(tx.clone());
                    }
                }
            }
        }

        // Sort by gas price descending, then nonce ascending
        all_txs.sort_by(|a, b| {
            let a_price = a.transaction.gas_price.unwrap_or(0);
            let b_price = b.transaction.gas_price.unwrap_or(0);
            b_price
                .cmp(&a_price)
                .then_with(|| a.transaction.nonce.cmp(&b.transaction.nonce))
        });

        all_txs.truncate(max_count);
        all_txs
    }

    /// Get all game action transactions from the pool.
    pub fn get_game_actions(&self) -> Vec<SignedTransaction> {
        self.by_hash
            .iter()
            .filter(|entry| entry.value().is_game_action())
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Remove transactions that have been included in a block.
    pub fn remove_included(&self, tx_hashes: &[B256]) {
        for hash in tx_hashes {
            self.remove_transaction(hash);
        }
        debug!(
            "Removed {} included transactions from pool",
            tx_hashes.len()
        );
    }

    /// Get the number of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.by_hash.len()
    }

    /// Get the number of queued transactions.
    pub fn queued_count(&self) -> usize {
        0
    }

    /// Get total transaction count.
    pub fn total_count(&self) -> usize {
        self.by_hash.len()
    }

    /// Clear all transactions from the pool.
    pub fn clear(&self) {
        self.by_hash.clear();
        self.by_sender.clear();
        info!("Transaction pool cleared");
    }

    /// Check if a transaction exists in the pool.
    pub fn contains(&self, hash: &B256) -> bool {
        self.by_hash.contains_key(hash)
    }

    /// Get the pending nonce for a sender (next expected nonce).
    pub fn pending_nonce(&self, sender: &Address) -> Option<u64> {
        self.by_sender
            .get(sender)
            .and_then(|queue| queue.txs.keys().last().map(|last_nonce| last_nonce + 1))
    }

    /// Get all transactions for a specific sender.
    pub fn get_sender_txs(&self, sender: &Address) -> Vec<SignedTransaction> {
        self.by_sender
            .get(sender)
            .map(|queue| queue.txs.values().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for TransactionPool {
    fn default() -> Self {
        Self::new()
    }
}
