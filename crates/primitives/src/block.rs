use alloy_primitives::{Address, Bloom, B256, B64, U256};
use serde::{Deserialize, Serialize};

use crate::transaction::SignedTransaction;

/// Block header containing metadata about a block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    /// Hash of the parent block.
    pub parent_hash: B256,
    /// Hash of the ommers/uncles list (always empty hash for PoA).
    pub ommers_hash: B256,
    /// Address of the block producer (validator).
    pub beneficiary: Address,
    /// Root hash of the world state trie after this block.
    pub state_root: B256,
    /// Root hash of the transactions trie.
    pub transactions_root: B256,
    /// Root hash of the receipts trie.
    pub receipts_root: B256,
    /// Root hash of the game state trie after this block's tick.
    pub game_state_root: B256,
    /// Bloom filter for logs.
    pub logs_bloom: Bloom,
    /// Block difficulty (1 for PoA).
    pub difficulty: U256,
    /// Block number (height).
    pub number: u64,
    /// Maximum gas allowed in this block.
    pub gas_limit: u64,
    /// Total gas used by transactions in this block.
    pub gas_used: u64,
    /// Block timestamp (Unix seconds).
    pub timestamp: u64,
    /// Game tick number corresponding to this block.
    pub game_tick: u64,
    /// Arbitrary extra data (used for validator signature in PoA).
    pub extra_data: Vec<u8>,
    /// Mix hash (unused in PoA, reserved).
    pub mix_hash: B256,
    /// Nonce (unused in PoA, reserved).
    pub nonce: B64,
    /// EIP-1559 base fee per gas (optional).
    pub base_fee_per_gas: Option<u64>,
}

impl BlockHeader {
    /// Compute the hash of this block header.
    pub fn hash(&self) -> B256 {
        use sha3::{Digest, Keccak256};
        let encoded = serde_json::to_vec(self).unwrap_or_default();
        let hash = Keccak256::digest(&encoded);
        B256::from_slice(&hash)
    }

    /// Create a genesis block header.
    pub fn genesis(game_state_root: B256) -> Self {
        Self {
            parent_hash: B256::ZERO,
            ommers_hash: B256::ZERO,
            beneficiary: Address::ZERO,
            state_root: B256::ZERO,
            transactions_root: B256::ZERO,
            receipts_root: B256::ZERO,
            game_state_root,
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(1),
            number: 0,
            gas_limit: crate::DEFAULT_BLOCK_GAS_LIMIT,
            gas_used: 0,
            timestamp: 0,
            game_tick: 0,
            extra_data: Vec::new(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: Some(1_000_000_000), // 1 gwei
        }
    }
}

/// Block body containing transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockBody {
    /// Signed transactions included in this block.
    pub transactions: Vec<SignedTransaction>,
}

/// A complete block: header + body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub header: BlockHeader,
    pub body: BlockBody,
}

impl Block {
    /// Hash of this block (hash of the header).
    pub fn hash(&self) -> B256 {
        self.header.hash()
    }

    /// Block number.
    pub fn number(&self) -> u64 {
        self.header.number
    }

    /// Number of transactions in this block.
    pub fn tx_count(&self) -> usize {
        self.body.transactions.len()
    }

    /// Create a new block.
    pub fn new(header: BlockHeader, transactions: Vec<SignedTransaction>) -> Self {
        Self {
            header,
            body: BlockBody { transactions },
        }
    }
}
