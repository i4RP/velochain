//! World state management.
//!
//! Provides read/write access to account state and game state,
//! backed by the storage layer.

use alloy_primitives::{Address, B256, U256};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use velochain_primitives::Account;
use velochain_storage::Database;

use crate::error::StateError;

/// The world state, providing access to accounts and game data.
pub struct WorldState {
    /// Underlying database.
    db: Arc<Database>,
    /// In-memory account cache for the current block.
    account_cache: RwLock<HashMap<Address, Account>>,
    /// Dirty accounts that have been modified.
    dirty_accounts: RwLock<HashMap<Address, Account>>,
    /// Current state root hash.
    state_root: RwLock<B256>,
}

impl WorldState {
    /// Create a new world state backed by the given database.
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            account_cache: RwLock::new(HashMap::new()),
            dirty_accounts: RwLock::new(HashMap::new()),
            state_root: RwLock::new(B256::ZERO),
        }
    }

    /// Get the current state root.
    pub fn state_root(&self) -> B256 {
        *self.state_root.read()
    }

    /// Get an account by address.
    pub fn get_account(&self, address: &Address) -> Result<Option<Account>, StateError> {
        // Check cache first
        if let Some(account) = self.account_cache.read().get(address) {
            return Ok(Some(account.clone()));
        }

        // Check dirty accounts
        if let Some(account) = self.dirty_accounts.read().get(address) {
            return Ok(Some(account.clone()));
        }

        // Load from database
        let key = address.as_slice();
        match self.db.get_game_state(key)? {
            Some(data) => {
                let account: Account = serde_json::from_slice(&data)
                    .map_err(|e| StateError::Serialization(e.to_string()))?;
                // Cache it
                self.account_cache
                    .write()
                    .insert(*address, account.clone());
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    /// Get an account, creating a default one if it doesn't exist.
    pub fn get_or_create_account(&self, address: &Address) -> Result<Account, StateError> {
        match self.get_account(address)? {
            Some(account) => Ok(account),
            None => Ok(Account::default()),
        }
    }

    /// Update an account.
    pub fn put_account(&self, address: &Address, account: &Account) -> Result<(), StateError> {
        self.dirty_accounts
            .write()
            .insert(*address, account.clone());
        self.account_cache
            .write()
            .insert(*address, account.clone());
        Ok(())
    }

    /// Get account balance.
    pub fn get_balance(&self, address: &Address) -> Result<U256, StateError> {
        Ok(self
            .get_account(address)?
            .map(|a| a.balance)
            .unwrap_or(U256::ZERO))
    }

    /// Get account nonce.
    pub fn get_nonce(&self, address: &Address) -> Result<u64, StateError> {
        Ok(self.get_account(address)?.map(|a| a.nonce).unwrap_or(0))
    }

    /// Increment account nonce.
    pub fn increment_nonce(&self, address: &Address) -> Result<(), StateError> {
        let mut account = self.get_or_create_account(address)?;
        account.nonce += 1;
        self.put_account(address, &account)
    }

    /// Add balance to an account.
    pub fn add_balance(&self, address: &Address, amount: U256) -> Result<(), StateError> {
        let mut account = self.get_or_create_account(address)?;
        account.balance = account.balance.checked_add(amount).ok_or_else(|| {
            StateError::InvalidTransition("Balance overflow".to_string())
        })?;
        self.put_account(address, &account)
    }

    /// Subtract balance from an account.
    pub fn sub_balance(&self, address: &Address, amount: U256) -> Result<(), StateError> {
        let mut account = self.get_or_create_account(address)?;
        if account.balance < amount {
            return Err(StateError::InsufficientBalance {
                has: account.balance.to_string(),
                needs: amount.to_string(),
            });
        }
        account.balance -= amount;
        self.put_account(address, &account)
    }

    /// Commit all dirty state to the database.
    pub fn commit(&self) -> Result<B256, StateError> {
        let dirty = self.dirty_accounts.write().drain().collect::<Vec<_>>();

        for (address, account) in &dirty {
            let key = address.as_slice();
            let value = serde_json::to_vec(account)
                .map_err(|e| StateError::Serialization(e.to_string()))?;
            self.db.put_game_state(key, &value)?;
        }

        // Compute new state root (simplified: hash of all dirty account data)
        let new_root = self.compute_state_root()?;
        *self.state_root.write() = new_root;

        Ok(new_root)
    }

    /// Clear the in-memory caches (e.g., after a revert).
    pub fn clear_cache(&self) {
        self.account_cache.write().clear();
        self.dirty_accounts.write().clear();
    }

    /// Compute the state root hash.
    fn compute_state_root(&self) -> Result<B256, StateError> {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();

        // Hash all cached accounts deterministically
        let cache = self.account_cache.read();
        let mut sorted_accounts: Vec<_> = cache.iter().collect();
        sorted_accounts.sort_by_key(|(addr, _)| *addr);

        for (address, account) in sorted_accounts {
            hasher.update(address.as_slice());
            let data = serde_json::to_vec(account)
                .map_err(|e| StateError::Serialization(e.to_string()))?;
            hasher.update(&data);
        }

        let hash = hasher.finalize();
        Ok(B256::from_slice(&hash))
    }

    /// Get the underlying database reference.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }
}
