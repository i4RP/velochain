use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Genesis block configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genesis {
    /// Chain configuration.
    pub config: ChainConfig,
    /// Initial account allocations.
    pub alloc: HashMap<Address, GenesisAccount>,
    /// Genesis timestamp.
    pub timestamp: u64,
    /// Genesis extra data.
    pub extra_data: Vec<u8>,
    /// Genesis gas limit.
    pub gas_limit: u64,
    /// Genesis difficulty.
    pub difficulty: u64,
}

/// Chain configuration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Chain ID.
    pub chain_id: u64,
    /// Block time in seconds.
    pub block_time: u64,
    /// Game tick interval in milliseconds.
    pub tick_interval_ms: u64,
    /// Consensus configuration.
    pub consensus: ConsensusConfig,
    /// World configuration.
    pub world: WorldConfig,
}

/// Consensus configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Consensus type (currently only "poa" supported).
    pub engine: String,
    /// PoA: block period in seconds.
    pub period: u64,
    /// PoA: epoch length for checkpoints.
    pub epoch: u64,
    /// Initial validators.
    pub validators: Vec<Address>,
}

/// World configuration for the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    /// World seed for deterministic generation.
    pub seed: u64,
    /// World size in chunks (width x depth).
    pub size_chunks: [u32; 2],
    /// Maximum world height in blocks.
    pub max_height: u32,
    /// Spawn point coordinates.
    pub spawn_point: [f32; 3],
}

/// A genesis account allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAccount {
    /// Initial balance.
    pub balance: U256,
    /// Contract code (if any).
    #[serde(default)]
    pub code: Vec<u8>,
    /// Storage entries (if any).
    #[serde(default)]
    pub storage: HashMap<U256, U256>,
    /// Initial nonce.
    #[serde(default)]
    pub nonce: u64,
}

impl Default for Genesis {
    fn default() -> Self {
        let mut alloc = HashMap::new();
        // Default allocation: give the zero address some tokens for testing
        alloc.insert(
            Address::ZERO,
            GenesisAccount {
                balance: U256::from(1_000_000_000_000_000_000_000_000u128), // 1M tokens
                code: Vec::new(),
                storage: HashMap::new(),
                nonce: 0,
            },
        );

        Self {
            config: ChainConfig {
                chain_id: crate::DEFAULT_CHAIN_ID,
                block_time: crate::DEFAULT_BLOCK_TIME_SECS,
                tick_interval_ms: crate::DEFAULT_TICK_INTERVAL_MS,
                consensus: ConsensusConfig {
                    engine: "poa".to_string(),
                    period: 1,
                    epoch: 30000,
                    validators: vec![],
                },
                world: WorldConfig {
                    seed: 42,
                    size_chunks: [256, 256],
                    max_height: 256,
                    spawn_point: [0.0, 0.0, 64.0],
                },
            },
            alloc,
            timestamp: 0,
            extra_data: Vec::new(),
            gas_limit: crate::DEFAULT_BLOCK_GAS_LIMIT,
            difficulty: 1,
        }
    }
}
