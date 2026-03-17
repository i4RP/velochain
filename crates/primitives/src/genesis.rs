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
    /// EVM configuration.
    #[serde(default)]
    pub evm: EvmConfig,
    /// Fork schedule: block numbers at which upgrades activate.
    #[serde(default)]
    pub forks: ForkConfig,
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

/// EVM execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmConfig {
    /// Block gas limit (overrides Genesis.gas_limit for dynamic adjustment).
    #[serde(default = "default_block_gas_limit")]
    pub block_gas_limit: u64,
    /// Minimum gas price accepted by the node.
    #[serde(default)]
    pub min_gas_price: u64,
    /// Maximum contract code size in bytes (default: 24576 = 24KB per EIP-170).
    #[serde(default = "default_max_code_size")]
    pub max_code_size: usize,
    /// Whether to enable EIP-1559 style fee market.
    #[serde(default)]
    pub eip1559: bool,
    /// List of enabled precompile address ranges (start, end inclusive).
    #[serde(default = "default_precompiles")]
    pub precompile_range: [u64; 2],
}

impl Default for EvmConfig {
    fn default() -> Self {
        Self {
            block_gas_limit: crate::DEFAULT_BLOCK_GAS_LIMIT,
            min_gas_price: 0,
            max_code_size: 24576,
            eip1559: false,
            precompile_range: default_precompiles(),
        }
    }
}

fn default_block_gas_limit() -> u64 {
    crate::DEFAULT_BLOCK_GAS_LIMIT
}

fn default_max_code_size() -> usize {
    24576
}

fn default_precompiles() -> [u64; 2] {
    [1, 9] // Standard Ethereum precompiles 0x01..0x09
}

/// Fork scheduling configuration.
/// Each field is an optional block number at which the fork activates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForkConfig {
    /// Block number to activate EIP-1559 fee market.
    #[serde(default)]
    pub eip1559_block: Option<u64>,
    /// Block number to activate contract code size limit increase.
    #[serde(default)]
    pub code_size_increase_block: Option<u64>,
    /// Block number to activate custom game precompiles.
    #[serde(default)]
    pub game_precompiles_block: Option<u64>,
}

impl ForkConfig {
    /// Check if a fork is active at the given block number.
    pub fn is_eip1559_active(&self, block: u64) -> bool {
        self.eip1559_block.is_some_and(|b| block >= b)
    }

    /// Check if code size increase is active at the given block number.
    pub fn is_code_size_increase_active(&self, block: u64) -> bool {
        self.code_size_increase_block.is_some_and(|b| block >= b)
    }

    /// Check if game precompiles are active at the given block number.
    pub fn is_game_precompiles_active(&self, block: u64) -> bool {
        self.game_precompiles_block.is_some_and(|b| block >= b)
    }
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
                evm: EvmConfig::default(),
                forks: ForkConfig::default(),
            },
            alloc,
            timestamp: 0,
            extra_data: Vec::new(),
            gas_limit: crate::DEFAULT_BLOCK_GAS_LIMIT,
            difficulty: 1,
        }
    }
}
