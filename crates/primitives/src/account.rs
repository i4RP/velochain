use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// An Ethereum-compatible account with game extensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    /// Account nonce (transaction count).
    pub nonce: u64,
    /// Account balance in wei.
    pub balance: U256,
    /// Hash of the account's contract code (keccak256 of empty for EOAs).
    pub code_hash: B256,
    /// Root hash of the account's storage trie.
    pub storage_root: B256,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            nonce: 0,
            balance: U256::ZERO,
            code_hash: B256::ZERO,
            storage_root: B256::ZERO,
        }
    }
}

impl Account {
    /// Create a new account with the given balance.
    pub fn with_balance(balance: U256) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }

    /// Check if this account is empty (zero nonce, zero balance, no code).
    pub fn is_empty(&self) -> bool {
        self.nonce == 0 && self.balance == U256::ZERO && self.code_hash == B256::ZERO
    }
}

/// Game-specific player data associated with an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    /// Player's address (links to Account).
    pub address: Address,
    /// Player's in-game entity ID.
    pub entity_id: u64,
    /// Player's position in the world.
    pub position: Position,
    /// Player's health points.
    pub health: f32,
    /// Player's maximum health points.
    pub max_health: f32,
    /// Player's level.
    pub level: u32,
    /// Player's experience points.
    pub experience: u64,
    /// Whether the player is currently alive.
    pub is_alive: bool,
    /// Last game tick this player was active.
    pub last_active_tick: u64,
}

/// A 3D position in the game world.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            address: Address::ZERO,
            entity_id: 0,
            position: Position::zero(),
            health: 100.0,
            max_health: 100.0,
            level: 1,
            experience: 0,
            is_alive: true,
            last_active_tick: 0,
        }
    }
}
