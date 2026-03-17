//! Transaction pool implementation.

use crate::error::TxPoolError;
use alloy_primitives::B256;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};
use velochain_primitives::SignedTransaction;

/// Maximum number of transactions in the pool.
const DEFAULT_MAX_POOL_SIZE: usize = 10_000;

/// Transaction pool managing pending and queued transactions.
pub struct TransactionPool {
    /// Pending transactions ready for inclusion (sorted by gas price).
    pending: Arc<DashMap<B256, SignedTransaction>>,
    /// Queued transactions with future nonces.
    queued: Arc<DashMap<B256, SignedTransaction>>,
    /// Maximum pool size.
    max_size: usize,
}

impl TransactionPool {
    /// Create a new transaction pool.
    pub fn new() -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            queued: Arc::new(DashMap::new()),
            max_size: DEFAULT_MAX_POOL_SIZE,
        }
    }

    /// Create a new transaction pool with custom max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            queued: Arc::new(DashMap::new()),
            max_size,
        }
    }

    /// Add a transaction to the pool.
    pub fn add_transaction(&self, tx: SignedTransaction) -> Result<B256, TxPoolError> {
        let hash = tx.hash;

        // Check if already exists
        if self.pending.contains_key(&hash) || self.queued.contains_key(&hash) {
            return Err(TxPoolError::AlreadyExists(format!("{}", hash)));
        }

        // Check pool capacity
        let total = self.pending.len() + self.queued.len();
        if total >= self.max_size {
            return Err(TxPoolError::PoolFull {
                capacity: self.max_size,
                current: total,
            });
        }

        // For now, add all transactions to pending
        // In a full implementation, we would check nonces and sort
        self.pending.insert(hash, tx);
        debug!("Transaction added to pool: {}", hash);

        Ok(hash)
    }

    /// Remove a transaction from the pool.
    pub fn remove_transaction(&self, hash: &B256) -> Option<SignedTransaction> {
        self.pending
            .remove(hash)
            .or_else(|| self.queued.remove(hash))
            .map(|(_, tx)| tx)
    }

    /// Get pending transactions for block inclusion.
    ///
    /// Returns up to `max_count` transactions, sorted by gas price (highest first).
    pub fn get_pending(&self, max_count: usize) -> Vec<SignedTransaction> {
        let mut txs: Vec<SignedTransaction> = self
            .pending
            .iter()
            .take(max_count)
            .map(|entry| entry.value().clone())
            .collect();

        // Sort by gas price descending (prioritize higher gas price)
        txs.sort_by(|a, b| {
            let a_price = a.transaction.gas_price.unwrap_or(0);
            let b_price = b.transaction.gas_price.unwrap_or(0);
            b_price.cmp(&a_price)
        });

        txs.truncate(max_count);
        txs
    }

    /// Get all game action transactions from pending.
    pub fn get_game_actions(&self) -> Vec<SignedTransaction> {
        self.pending
            .iter()
            .filter(|entry| entry.value().is_game_action())
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Remove transactions that have been included in a block.
    pub fn remove_included(&self, tx_hashes: &[B256]) {
        for hash in tx_hashes {
            self.pending.remove(hash);
            self.queued.remove(hash);
        }
        debug!("Removed {} included transactions from pool", tx_hashes.len());
    }

    /// Get the number of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get the number of queued transactions.
    pub fn queued_count(&self) -> usize {
        self.queued.len()
    }

    /// Get total transaction count.
    pub fn total_count(&self) -> usize {
        self.pending.len() + self.queued.len()
    }

    /// Clear all transactions from the pool.
    pub fn clear(&self) {
        self.pending.clear();
        self.queued.clear();
        info!("Transaction pool cleared");
    }

    /// Check if a transaction exists in the pool.
    pub fn contains(&self, hash: &B256) -> bool {
        self.pending.contains_key(hash) || self.queued.contains_key(hash)
    }
}

impl Default for TransactionPool {
    fn default() -> Self {
        Self::new()
    }
}
