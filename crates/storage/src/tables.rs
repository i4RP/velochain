//! Database table definitions.
//!
//! Each table is stored as a RocksDB column family.

/// Column family names for the database.
pub mod cf {
    /// Block headers: block_hash -> BlockHeader (serialized)
    pub const HEADERS: &str = "headers";
    /// Block bodies: block_hash -> BlockBody (serialized)
    pub const BODIES: &str = "bodies";
    /// Block number to hash mapping: block_number (u64 BE) -> block_hash
    pub const BLOCK_NUMBER_TO_HASH: &str = "block_number_to_hash";
    /// Transaction index: tx_hash -> (block_hash, tx_index)
    pub const TRANSACTIONS: &str = "transactions";
    /// Receipts: tx_hash -> TransactionReceipt (serialized)
    pub const RECEIPTS: &str = "receipts";
    /// Account state: address -> Account (serialized)
    pub const ACCOUNTS: &str = "accounts";
    /// Contract code: code_hash -> bytecode
    pub const CODE: &str = "code";
    /// Account storage: (address, slot) -> value
    pub const STORAGE: &str = "storage";
    /// Game state: key -> value (game world data)
    pub const GAME_STATE: &str = "game_state";
    /// Chain metadata: key -> value
    pub const META: &str = "meta";
}

/// All column families used by the database.
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    cf::HEADERS,
    cf::BODIES,
    cf::BLOCK_NUMBER_TO_HASH,
    cf::TRANSACTIONS,
    cf::RECEIPTS,
    cf::ACCOUNTS,
    cf::CODE,
    cf::STORAGE,
    cf::GAME_STATE,
    cf::META,
];
