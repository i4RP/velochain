//! Batch write operations for RocksDB.
//!
//! Groups multiple write operations into a single atomic write batch,
//! reducing disk I/O and improving block import performance.

use crate::error::StorageError;
use crate::tables;
use rocksdb::WriteBatch;
use std::sync::Arc;
use tracing::debug;
use velochain_primitives::{Block, BlockHeader};

/// A batch of write operations to be applied atomically.
pub struct WriteBatchOps {
    batch: WriteBatch,
    db: Arc<rocksdb::DB>,
    op_count: usize,
}

impl WriteBatchOps {
    /// Create a new write batch tied to the given database.
    pub(crate) fn new(db: Arc<rocksdb::DB>) -> Self {
        Self {
            batch: WriteBatch::default(),
            db,
            op_count: 0,
        }
    }

    /// Put a raw key-value pair into a column family.
    pub fn put_cf(&mut self, cf_name: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::Database(format!("Column family not found: {cf_name}")))?;
        self.batch.put_cf(&cf, key, value);
        self.op_count += 1;
        Ok(())
    }

    /// Delete a key from a column family.
    pub fn delete_cf(&mut self, cf_name: &str, key: &[u8]) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::Database(format!("Column family not found: {cf_name}")))?;
        self.batch.delete_cf(&cf, key);
        self.op_count += 1;
        Ok(())
    }

    /// Add a block header to the batch.
    pub fn put_header(
        &mut self,
        hash: &[u8; 32],
        header: &BlockHeader,
    ) -> Result<(), StorageError> {
        let encoded =
            bincode::serialize(header).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(tables::cf::HEADERS, hash, &encoded)?;
        self.put_cf(
            tables::cf::BLOCK_NUMBER_TO_HASH,
            &header.number.to_be_bytes(),
            hash,
        )?;
        Ok(())
    }

    /// Add a complete block (header + body + tx index) to the batch.
    pub fn put_block(&mut self, block: &Block) -> Result<(), StorageError> {
        let hash = block.hash();
        self.put_header(hash.as_ref(), &block.header)?;

        let body_encoded = bincode::serialize(&block.body)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(tables::cf::BODIES, hash.as_ref(), &body_encoded)?;

        // Index transactions
        for (idx, tx) in block.body.transactions.iter().enumerate() {
            let tx_index = crate::db::TxIndexEntry {
                block_hash: *hash.as_ref(),
                index: idx as u32,
            };
            let tx_index_encoded = bincode::serialize(&tx_index)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            self.put_cf(
                tables::cf::TRANSACTIONS,
                tx.hash.as_ref(),
                &tx_index_encoded,
            )?;
        }

        Ok(())
    }

    /// Add a receipt to the batch.
    pub fn put_receipt(
        &mut self,
        tx_hash: &[u8; 32],
        receipt_data: &[u8],
    ) -> Result<(), StorageError> {
        self.put_cf(tables::cf::RECEIPTS, tx_hash, receipt_data)
    }

    /// Add metadata to the batch.
    pub fn put_meta(&mut self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        self.put_cf(tables::cf::META, key.as_bytes(), value)
    }

    /// Get the number of operations in this batch.
    pub fn op_count(&self) -> usize {
        self.op_count
    }

    /// Commit the batch atomically to the database.
    pub fn commit(self) -> Result<(), StorageError> {
        debug!("Committing write batch with {} operations", self.op_count);
        self.db
            .write(self.batch)
            .map_err(|e| StorageError::Database(format!("Batch write failed: {e}")))?;
        Ok(())
    }
}
