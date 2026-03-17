//! Proof-of-Authority (Clique-style) consensus implementation.
//!
//! Validators take turns producing blocks in round-robin fashion.
//! Each block corresponds to one game tick.

use alloy_primitives::{Address, B256, B64, Bloom, U256};
use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::{debug, info, warn};
use velochain_primitives::{BlockHeader, Block, Keypair, DEFAULT_BLOCK_GAS_LIMIT};

use crate::{ConsensusEngine, ConsensusError};

/// PoA consensus configuration.
#[derive(Debug, Clone)]
pub struct PoaConfig {
    /// Block period in seconds.
    pub period: u64,
    /// Epoch length for checkpoints.
    pub epoch: u64,
    /// Chain ID.
    pub chain_id: u64,
}

impl Default for PoaConfig {
    fn default() -> Self {
        Self {
            period: 1,
            epoch: 30000,
            chain_id: velochain_primitives::DEFAULT_CHAIN_ID,
        }
    }
}

/// Proof-of-Authority consensus engine.
pub struct PoaConsensus {
    /// Validator keypair for signing blocks (None if not a validator).
    keypair: Option<Keypair>,
    /// Our validator address (derived from keypair, or None).
    local_address: Option<Address>,
    /// Current validator set.
    validators: RwLock<Vec<Address>>,
    /// Configuration.
    config: PoaConfig,
    /// Current game tick counter.
    game_tick: RwLock<u64>,
}

impl PoaConsensus {
    /// Create a new PoA consensus engine with a validator keypair.
    pub fn new_with_keypair(
        keypair: Keypair,
        validators: Vec<Address>,
        config: PoaConfig,
    ) -> Self {
        let address = keypair.address();
        info!("PoA consensus initialized with validator address: {:?}", address);
        Self {
            keypair: Some(keypair),
            local_address: Some(address),
            validators: RwLock::new(validators),
            config,
            game_tick: RwLock::new(0),
        }
    }

    /// Create a new PoA consensus engine without a validator key (read-only node).
    pub fn new_readonly(validators: Vec<Address>, config: PoaConfig) -> Self {
        Self {
            keypair: None,
            local_address: None,
            validators: RwLock::new(validators),
            config,
            game_tick: RwLock::new(0),
        }
    }

    /// Determine which validator should produce the block at the given height.
    pub fn proposer_for_height(&self, height: u64) -> Option<Address> {
        let validators = self.validators.read();
        if validators.is_empty() {
            return None;
        }
        let index = (height as usize) % validators.len();
        Some(validators[index])
    }

    /// Check if we are the proposer for the given height.
    pub fn is_our_turn(&self, height: u64) -> bool {
        match (self.local_address, self.proposer_for_height(height)) {
            (Some(local), Some(proposer)) => local == proposer,
            _ => false,
        }
    }

    /// Get the current game tick.
    pub fn current_tick(&self) -> u64 {
        *self.game_tick.read()
    }

    /// Advance the game tick.
    pub fn advance_tick(&self) -> u64 {
        let mut tick = self.game_tick.write();
        *tick += 1;
        *tick
    }
}

#[async_trait]
impl ConsensusEngine for PoaConsensus {
    fn verify_header(
        &self,
        header: &BlockHeader,
        parent: &BlockHeader,
    ) -> Result<(), ConsensusError> {
        // Check block number is sequential
        if header.number != parent.number + 1 {
            return Err(ConsensusError::InvalidHeader(format!(
                "Block number mismatch: expected {}, got {}",
                parent.number + 1,
                header.number
            )));
        }

        // Check parent hash
        if header.parent_hash != parent.hash() {
            return Err(ConsensusError::InvalidHeader(
                "Parent hash mismatch".to_string(),
            ));
        }

        // Check timestamp is not before parent
        if header.timestamp <= parent.timestamp {
            return Err(ConsensusError::InvalidHeader(
                "Timestamp must be after parent".to_string(),
            ));
        }

        // Check game tick is sequential
        if header.game_tick != parent.game_tick + 1 {
            return Err(ConsensusError::InvalidHeader(format!(
                "Game tick mismatch: expected {}, got {}",
                parent.game_tick + 1,
                header.game_tick
            )));
        }

        // Check difficulty is 1 (PoA)
        if header.difficulty != U256::from(1) {
            return Err(ConsensusError::InvalidHeader(
                "PoA difficulty must be 1".to_string(),
            ));
        }

        debug!(
            "Header verified: block={}, tick={}, validator={}",
            header.number, header.game_tick, header.beneficiary
        );

        Ok(())
    }

    fn is_validator(&self) -> bool {
        match self.local_address {
            Some(addr) => self.validators.read().contains(&addr),
            None => false,
        }
    }

    fn prepare_header(&self, parent: &BlockHeader) -> Result<BlockHeader, ConsensusError> {
        let local_address = self
            .local_address
            .ok_or(ConsensusError::NotValidator)?;

        if !self.is_validator() {
            return Err(ConsensusError::NotValidator);
        }

        let next_number = parent.number + 1;
        let next_tick = self.advance_tick();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Ensure timestamp is at least parent + period
        let timestamp = std::cmp::max(now, parent.timestamp + self.config.period);

        if !self.is_our_turn(next_number) {
            warn!(
                "Not our turn for block {}, expected {:?}",
                next_number,
                self.proposer_for_height(next_number)
            );
        }

        Ok(BlockHeader {
            parent_hash: parent.hash(),
            ommers_hash: B256::ZERO,
            beneficiary: local_address,
            state_root: B256::ZERO, // Will be filled after execution
            transactions_root: B256::ZERO,
            receipts_root: B256::ZERO,
            game_state_root: B256::ZERO, // Will be filled after game tick
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(1),
            number: next_number,
            gas_limit: DEFAULT_BLOCK_GAS_LIMIT,
            gas_used: 0,
            timestamp,
            game_tick: next_tick,
            extra_data: Vec::new(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: parent.base_fee_per_gas,
        })
    }

    fn seal_block(&self, block: &mut Block) -> Result<(), ConsensusError> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or(ConsensusError::NotValidator)?;

        // Sign the block hash with our validator private key.
        // The signature is stored in extra_data as: [32 bytes R | 32 bytes S | 1 byte V]
        let block_hash = block.header.hash();
        let (sig, recid) = keypair
            .sign_hash(&block_hash)
            .map_err(|e| ConsensusError::SealError(format!("Signing failed: {e}")))?;

        let sig_bytes = sig.to_bytes();
        let mut extra_data = Vec::with_capacity(65);
        extra_data.extend_from_slice(&sig_bytes); // 64 bytes (R + S)
        extra_data.push(recid.to_byte()); // 1 byte (V)
        block.header.extra_data = extra_data;

        debug!(
            "Block sealed: number={}, hash={}, signer={:?}",
            block.number(),
            block.hash(),
            keypair.address()
        );
        Ok(())
    }

    fn validators(&self) -> Vec<Address> {
        self.validators.read().clone()
    }
}
