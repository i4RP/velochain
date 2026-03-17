use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// Transaction type identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TxType {
    /// Legacy transaction (type 0).
    Legacy = 0,
    /// EIP-2930 access list transaction (type 1).
    AccessList = 1,
    /// EIP-1559 dynamic fee transaction (type 2).
    DynamicFee = 2,
    /// Game action transaction (type 100) - custom for VeloChain.
    GameAction = 100,
}

/// Game-specific action types that can be embedded in transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameAction {
    /// Move player to position (fixed-point: milliunits).
    Move { x: i64, y: i64, z: i64 },
    /// Attack an entity.
    Attack { target_entity_id: u64 },
    /// Interact with world object.
    Interact { target_entity_id: u64 },
    /// Place a block in the world.
    PlaceBlock { x: i32, y: i32, z: i32, block_type: u16 },
    /// Break a block in the world.
    BreakBlock { x: i32, y: i32, z: i32 },
    /// Craft an item.
    Craft { recipe_id: u32 },
    /// Chat message.
    Chat { message: String },
    /// Respawn player.
    Respawn,
}

/// Core transaction data (unsigned).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    /// Transaction type.
    pub tx_type: TxType,
    /// Chain ID for replay protection.
    pub chain_id: u64,
    /// Sender's nonce.
    pub nonce: u64,
    /// Gas price (for legacy transactions).
    pub gas_price: Option<u128>,
    /// Max fee per gas (EIP-1559).
    pub max_fee_per_gas: Option<u128>,
    /// Max priority fee per gas (EIP-1559).
    pub max_priority_fee_per_gas: Option<u128>,
    /// Gas limit for this transaction.
    pub gas_limit: u64,
    /// Recipient address (None for contract creation).
    pub to: Option<Address>,
    /// Value to transfer in wei.
    pub value: U256,
    /// Input data (calldata or game action).
    pub input: Vec<u8>,
    /// Game action (for GameAction transaction type).
    pub game_action: Option<GameAction>,
}

/// ECDSA signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Signature {
    /// Recovery ID.
    pub v: u64,
    /// R component.
    pub r: U256,
    /// S component.
    pub s: U256,
}

/// A signed transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignedTransaction {
    /// The transaction data.
    pub transaction: Transaction,
    /// The ECDSA signature.
    pub signature: Signature,
    /// Cached transaction hash.
    pub hash: B256,
}

impl SignedTransaction {
    /// Compute the hash of this transaction.
    pub fn compute_hash(tx: &Transaction, sig: &Signature) -> B256 {
        use sha3::{Digest, Keccak256};
        let mut data = serde_json::to_vec(tx).unwrap_or_default();
        data.extend_from_slice(&serde_json::to_vec(sig).unwrap_or_default());
        let hash = Keccak256::digest(&data);
        B256::from_slice(&hash)
    }

    /// Create a new signed transaction.
    pub fn new(transaction: Transaction, signature: Signature) -> Self {
        let hash = Self::compute_hash(&transaction, &signature);
        Self {
            transaction,
            signature,
            hash,
        }
    }

    /// Get the sender address from the signature.
    pub fn sender(&self) -> Result<Address, crate::PrimitivesError> {
        // TODO: Recover sender from ECDSA signature using k256
        // For now, return a placeholder
        Err(crate::PrimitivesError::SignatureError(
            "Signature recovery not yet implemented".to_string(),
        ))
    }

    /// Check if this is a game action transaction.
    pub fn is_game_action(&self) -> bool {
        self.transaction.tx_type == TxType::GameAction
    }

    /// Get the game action if this is a game action transaction.
    pub fn game_action(&self) -> Option<&GameAction> {
        self.transaction.game_action.as_ref()
    }
}
