use alloy_primitives::{Address, Bloom, B256, B64, U256};
use alloy_rlp::Encodable;
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

/// Manual RLP encoding for BlockHeader.
/// We encode all fields in order as an RLP list, which is the standard
/// Ethereum approach. We include the custom `game_state_root` field.
impl Encodable for BlockHeader {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        alloy_rlp::Header {
            list: true,
            payload_length: self.rlp_payload_length(),
        }
        .encode(out);
        self.parent_hash.encode(out);
        self.ommers_hash.encode(out);
        self.beneficiary.encode(out);
        self.state_root.encode(out);
        self.transactions_root.encode(out);
        self.receipts_root.encode(out);
        self.game_state_root.encode(out);
        self.logs_bloom.encode(out);
        self.difficulty.encode(out);
        self.number.encode(out);
        self.gas_limit.encode(out);
        self.gas_used.encode(out);
        self.timestamp.encode(out);
        self.game_tick.encode(out);
        self.extra_data.encode(out);
        self.mix_hash.encode(out);
        self.nonce.encode(out);
        if let Some(base_fee) = self.base_fee_per_gas {
            base_fee.encode(out);
        }
    }

    fn length(&self) -> usize {
        let payload = self.rlp_payload_length();
        payload + alloy_rlp::length_of_length(payload)
    }
}

impl BlockHeader {
    fn rlp_payload_length(&self) -> usize {
        self.parent_hash.length()
            + self.ommers_hash.length()
            + self.beneficiary.length()
            + self.state_root.length()
            + self.transactions_root.length()
            + self.receipts_root.length()
            + self.game_state_root.length()
            + self.logs_bloom.length()
            + self.difficulty.length()
            + self.number.length()
            + self.gas_limit.length()
            + self.gas_used.length()
            + self.timestamp.length()
            + self.game_tick.length()
            + self.extra_data.length()
            + self.mix_hash.length()
            + self.nonce.length()
            + self.base_fee_per_gas.map_or(0, |v| v.length())
    }

    /// Compute the hash of this block header using RLP encoding (Ethereum-compatible).
    pub fn hash(&self) -> B256 {
        use sha3::{Digest, Keccak256};
        let mut buf = Vec::new();
        self.encode(&mut buf);
        B256::from_slice(&Keccak256::digest(&buf))
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

    /// Compute the transactions root hash (keccak256 of ordered tx hashes).
    pub fn compute_transactions_root(&self) -> B256 {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        for tx in &self.body.transactions {
            hasher.update(tx.hash.as_slice());
        }
        B256::from_slice(&hasher.finalize())
    }
}
