use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

use crate::crypto::{self, Keypair};
use crate::PrimitivesError;

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
    PlaceBlock {
        x: i32,
        y: i32,
        z: i32,
        block_type: u16,
    },
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

impl Transaction {
    /// Compute the signing hash for this transaction.
    pub fn signing_hash(&self) -> B256 {
        let encoded = serde_json::to_vec(self).unwrap_or_default();
        B256::from_slice(&Keccak256::digest(&encoded))
    }

    /// Sign this transaction with the given keypair, producing a SignedTransaction.
    pub fn sign(self, keypair: &Keypair) -> Result<SignedTransaction, PrimitivesError> {
        let hash = self.signing_hash();
        let (sig, recid) = keypair.sign_hash(&hash)?;
        let sig_bytes = sig.to_bytes();

        let mut r_bytes = [0u8; 32];
        let mut s_bytes = [0u8; 32];
        r_bytes.copy_from_slice(&sig_bytes[..32]);
        s_bytes.copy_from_slice(&sig_bytes[32..]);

        let signature = Signature {
            v: recid.to_byte() as u64,
            r: U256::from_be_bytes(r_bytes),
            s: U256::from_be_bytes(s_bytes),
        };

        let tx_hash = SignedTransaction::compute_hash(&self, &signature);
        Ok(SignedTransaction {
            transaction: self,
            signature,
            hash: tx_hash,
        })
    }

    /// Create a game action transaction.
    pub fn new_game_action(chain_id: u64, nonce: u64, action: GameAction) -> Self {
        Self {
            tx_type: TxType::GameAction,
            chain_id,
            nonce,
            gas_price: Some(1_000_000_000), // 1 gwei
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas_limit: 100_000,
            to: None,
            value: U256::ZERO,
            input: Vec::new(),
            game_action: Some(action),
        }
    }
}

impl SignedTransaction {
    /// Compute the hash of this transaction.
    pub fn compute_hash(tx: &Transaction, sig: &Signature) -> B256 {
        let mut data = serde_json::to_vec(tx).unwrap_or_default();
        data.extend_from_slice(&serde_json::to_vec(sig).unwrap_or_default());
        B256::from_slice(&Keccak256::digest(&data))
    }

    /// Create a new signed transaction (manually providing signature).
    pub fn new(transaction: Transaction, signature: Signature) -> Self {
        let hash = Self::compute_hash(&transaction, &signature);
        Self {
            transaction,
            signature,
            hash,
        }
    }

    /// Recover the sender address from the ECDSA signature.
    pub fn sender(&self) -> Result<Address, PrimitivesError> {
        let signing_hash = self.transaction.signing_hash();
        let r_bytes: [u8; 32] = self.signature.r.to_be_bytes();
        let s_bytes: [u8; 32] = self.signature.s.to_be_bytes();
        crypto::recover_signer(&signing_hash, self.signature.v, &r_bytes, &s_bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_recover_sender() {
        let kp = Keypair::random();
        let tx = Transaction::new_game_action(
            27181,
            0,
            GameAction::Move {
                x: 1000,
                y: 2000,
                z: 3000,
            },
        );

        let signed = tx.sign(&kp).unwrap();
        let recovered_sender = signed.sender().unwrap();
        assert_eq!(recovered_sender, kp.address());
    }

    #[test]
    fn test_game_action_tx() {
        let kp = Keypair::random();
        let tx = Transaction::new_game_action(27181, 0, GameAction::Respawn);
        let signed = tx.sign(&kp).unwrap();

        assert!(signed.is_game_action());
        assert_eq!(signed.game_action(), Some(&GameAction::Respawn));
    }
}
