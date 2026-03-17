//! RocksDB-backed database implementation.

use crate::error::StorageError;
use crate::tables::{self, ALL_COLUMN_FAMILIES};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use velochain_primitives::{Block, BlockHeader};

/// Main database handle wrapping RocksDB.
pub struct Database {
    db: Arc<DB>,
}

impl Database {
    /// Open or create a database at the given path.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cfs: Vec<ColumnFamilyDescriptor> = ALL_COLUMN_FAMILIES
            .iter()
            .map(|name| {
                let cf_opts = Options::default();
                ColumnFamilyDescriptor::new(*name, cf_opts)
            })
            .collect();

        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        info!("Database opened at {}", path.display());
        Ok(Self { db: Arc::new(db) })
    }

    /// Get a raw value from a column family.
    pub fn get_cf(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::Database(format!("Column family not found: {cf_name}")))?;
        self.db
            .get_cf(&cf, key)
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    /// Put a raw value into a column family.
    pub fn put_cf(&self, cf_name: &str, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::Database(format!("Column family not found: {cf_name}")))?;
        self.db
            .put_cf(&cf, key, value)
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    /// Delete a key from a column family.
    pub fn delete_cf(&self, cf_name: &str, key: &[u8]) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(cf_name)
            .ok_or_else(|| StorageError::Database(format!("Column family not found: {cf_name}")))?;
        self.db
            .delete_cf(&cf, key)
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    // --- Block operations ---

    /// Store a block header.
    pub fn put_header(&self, hash: &[u8; 32], header: &BlockHeader) -> Result<(), StorageError> {
        let encoded =
            bincode::serialize(header).map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(tables::cf::HEADERS, hash, &encoded)?;
        // Also store the block number -> hash mapping
        self.put_cf(
            tables::cf::BLOCK_NUMBER_TO_HASH,
            &header.number.to_be_bytes(),
            hash,
        )?;
        Ok(())
    }

    /// Get a block header by hash.
    pub fn get_header(&self, hash: &[u8; 32]) -> Result<Option<BlockHeader>, StorageError> {
        match self.get_cf(tables::cf::HEADERS, hash)? {
            Some(data) => {
                let header = bincode::deserialize(&data)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(header))
            }
            None => Ok(None),
        }
    }

    /// Get a block hash by number.
    pub fn get_block_hash_by_number(&self, number: u64) -> Result<Option<[u8; 32]>, StorageError> {
        match self.get_cf(tables::cf::BLOCK_NUMBER_TO_HASH, &number.to_be_bytes())? {
            Some(data) => {
                if data.len() != 32 {
                    return Err(StorageError::Corruption(
                        "Invalid block hash length".to_string(),
                    ));
                }
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data);
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }

    /// Store a complete block.
    pub fn put_block(&self, block: &Block) -> Result<(), StorageError> {
        let hash = block.hash();
        self.put_header(hash.as_ref(), &block.header)?;

        let body_encoded = bincode::serialize(&block.body)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.put_cf(tables::cf::BODIES, hash.as_ref(), &body_encoded)?;

        // Index each transaction
        for (idx, tx) in block.body.transactions.iter().enumerate() {
            let tx_index = TxIndexEntry {
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

    // --- Metadata operations ---

    /// Store chain metadata.
    pub fn put_meta(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        self.put_cf(tables::cf::META, key.as_bytes(), value)
    }

    /// Get chain metadata.
    pub fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.get_cf(tables::cf::META, key.as_bytes())
    }

    /// Store the latest block number.
    pub fn put_latest_block_number(&self, number: u64) -> Result<(), StorageError> {
        self.put_meta("latest_block_number", &number.to_be_bytes())
    }

    /// Get the latest block number.
    pub fn get_latest_block_number(&self) -> Result<Option<u64>, StorageError> {
        match self.get_meta("latest_block_number")? {
            Some(data) => {
                if data.len() != 8 {
                    return Err(StorageError::Corruption(
                        "Invalid block number length".to_string(),
                    ));
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data);
                Ok(Some(u64::from_be_bytes(bytes)))
            }
            None => Ok(None),
        }
    }

    /// Get a block body by hash.
    pub fn get_body(
        &self,
        hash: &[u8; 32],
    ) -> Result<Option<velochain_primitives::BlockBody>, StorageError> {
        match self.get_cf(tables::cf::BODIES, hash)? {
            Some(data) => {
                let body = bincode::deserialize(&data)
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(body))
            }
            None => Ok(None),
        }
    }

    // --- Receipt operations ---

    /// Store a transaction receipt.
    pub fn put_receipt(&self, tx_hash: &[u8; 32], receipt_data: &[u8]) -> Result<(), StorageError> {
        self.put_cf(tables::cf::RECEIPTS, tx_hash, receipt_data)
    }

    /// Get a transaction receipt by hash.
    pub fn get_receipt(&self, tx_hash: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError> {
        self.get_cf(tables::cf::RECEIPTS, tx_hash)
    }

    // --- Game state operations ---

    /// Store game state data.
    pub fn put_game_state(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.put_cf(tables::cf::GAME_STATE, key, value)
    }

    /// Get game state data.
    pub fn get_game_state(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.get_cf(tables::cf::GAME_STATE, key)
    }

    /// Create a new write batch for atomic multi-operation writes.
    pub fn write_batch(&self) -> crate::batch::WriteBatchOps {
        crate::batch::WriteBatchOps::new(Arc::clone(&self.db))
    }
}

/// Index entry for locating a transaction within a block.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TxIndexEntry {
    pub(crate) block_hash: [u8; 32],
    pub(crate) index: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_header(number: u64) -> BlockHeader {
        let mut h = BlockHeader::genesis(Default::default());
        h.number = number;
        h.timestamp = 1000 + number;
        h.game_tick = number;
        h
    }

    #[test]
    fn test_open_database() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path());
        assert!(db.is_ok());
    }

    #[test]
    fn test_put_get_meta() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.put_meta("test_key", b"test_value").unwrap();
        let value = db.get_meta("test_key").unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }

    #[test]
    fn test_get_missing_meta() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let value = db.get_meta("nonexistent").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_put_get_header() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let header = make_test_header(1);
        let hash = header.hash();
        db.put_header(hash.as_ref(), &header).unwrap();

        let retrieved = db.get_header(hash.as_ref()).unwrap().unwrap();
        assert_eq!(retrieved.number, 1);
        assert_eq!(retrieved.timestamp, 1001);
    }

    #[test]
    fn test_get_missing_header() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let hash = [0u8; 32];
        assert!(db.get_header(&hash).unwrap().is_none());
    }

    #[test]
    fn test_block_number_to_hash() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let header = make_test_header(5);
        let hash = header.hash();
        db.put_header(hash.as_ref(), &header).unwrap();

        let retrieved_hash = db.get_block_hash_by_number(5).unwrap().unwrap();
        let hash_bytes: &[u8; 32] = hash.as_ref();
        assert_eq!(retrieved_hash, *hash_bytes);
    }

    #[test]
    fn test_get_missing_block_hash() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        assert!(db.get_block_hash_by_number(999).unwrap().is_none());
    }

    #[test]
    fn test_put_get_block() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let header = make_test_header(1);
        let block = Block::new(header, vec![]);
        let hash = block.hash();
        db.put_block(&block).unwrap();

        let body = db.get_body(hash.as_ref()).unwrap().unwrap();
        assert!(body.transactions.is_empty());

        let header = db.get_header(hash.as_ref()).unwrap().unwrap();
        assert_eq!(header.number, 1);
    }

    #[test]
    fn test_latest_block_number() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        assert!(db.get_latest_block_number().unwrap().is_none());
        db.put_latest_block_number(42).unwrap();
        assert_eq!(db.get_latest_block_number().unwrap(), Some(42));
        db.put_latest_block_number(100).unwrap();
        assert_eq!(db.get_latest_block_number().unwrap(), Some(100));
    }

    #[test]
    fn test_receipt_operations() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let hash = [0xABu8; 32];
        let data = b"receipt_data_here";
        db.put_receipt(&hash, data).unwrap();
        let retrieved = db.get_receipt(&hash).unwrap().unwrap();
        assert_eq!(retrieved, data.to_vec());
    }

    #[test]
    fn test_game_state_operations() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.put_game_state(b"world", b"serialized_world").unwrap();
        let retrieved = db.get_game_state(b"world").unwrap().unwrap();
        assert_eq!(retrieved, b"serialized_world".to_vec());
    }

    #[test]
    fn test_delete_cf() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.put_meta("to_delete", b"value").unwrap();
        assert!(db.get_meta("to_delete").unwrap().is_some());
        db.delete_cf(tables::cf::META, b"to_delete").unwrap();
        assert!(db.get_meta("to_delete").unwrap().is_none());
    }

    #[test]
    fn test_multiple_blocks_sequential() {
        let dir = tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();

        let genesis = make_test_header(0);
        let genesis_hash = genesis.hash();
        let block0 = Block::new(genesis, vec![]);
        db.put_block(&block0).unwrap();

        let mut header1 = make_test_header(1);
        header1.parent_hash = genesis_hash;
        let header1 = header1;
        let block1 = Block::new(header1, vec![]);
        db.put_block(&block1).unwrap();

        db.put_latest_block_number(1).unwrap();
        assert_eq!(db.get_latest_block_number().unwrap(), Some(1));

        let hash0 = db.get_block_hash_by_number(0).unwrap().unwrap();
        let hash1 = db.get_block_hash_by_number(1).unwrap().unwrap();
        assert_ne!(hash0, hash1);
    }
}
