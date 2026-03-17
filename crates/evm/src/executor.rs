//! EVM executor using revm.
//!
//! Executes EVM transactions against the current world state.
//! Used for asset management operations (transfers, NFT minting, etc.)

use crate::error::EvmError;
use alloy_primitives::{Address, B256, U256};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{AccountInfo, Bytecode, ExecutionResult, Output, TransactTo},
    Evm,
};
use tracing::debug;
use velochain_primitives::SignedTransaction;
use velochain_state::WorldState;

/// Result of executing a transaction through the EVM.
#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    /// Whether execution was successful.
    pub success: bool,
    /// Gas used by the execution.
    pub gas_used: u64,
    /// Return data from the execution.
    pub output: Vec<u8>,
    /// Logs emitted during execution.
    pub logs: Vec<Log>,
    /// Contract address if this was a creation.
    pub contract_address: Option<Address>,
}

/// An EVM log entry.
#[derive(Debug, Clone)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Vec<u8>,
}

/// EVM executor that processes transactions.
pub struct EvmExecutor {
    /// Chain ID.
    chain_id: u64,
    /// In-memory state database for the current block.
    db: CacheDB<EmptyDB>,
}

impl EvmExecutor {
    /// Create a new EVM executor.
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            db: CacheDB::new(EmptyDB::default()),
        }
    }

    /// Set an account's balance in the EVM state.
    pub fn set_balance(&mut self, address: Address, balance: U256) {
        let info = AccountInfo {
            balance,
            nonce: 0,
            code_hash: B256::ZERO,
            code: None,
        };
        self.db.insert_account_info(address, info);
    }

    /// Deploy contract code at an address.
    pub fn set_code(&mut self, address: Address, code: Vec<u8>) {
        let bytecode = Bytecode::new_raw(code.into());
        let code_hash = B256::from_slice(&{
            use sha3::{Digest, Keccak256};
            Keccak256::digest(bytecode.bytes_slice())
        });
        let info = AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            code: Some(bytecode),
        };
        self.db.insert_account_info(address, info);
    }

    /// Execute an EVM transaction.
    pub fn execute_tx(&mut self, tx: &SignedTransaction) -> Result<ExecutionOutcome, EvmError> {
        // Only execute non-game-action transactions through the EVM
        if tx.is_game_action() {
            return Err(EvmError::InvalidTransaction(
                "Game actions are not executed through EVM".to_string(),
            ));
        }

        // Recover sender from ECDSA signature
        let sender = tx
            .sender()
            .map_err(|e| EvmError::InvalidTransaction(format!("Failed to recover sender: {e}")))?;

        let tx_data = &tx.transaction;

        let mut evm = Evm::builder()
            .with_db(&mut self.db)
            .modify_tx_env(|tx_env| {
                tx_env.caller = sender;
                tx_env.transact_to = match tx_data.to {
                    Some(to) => TransactTo::Call(to),
                    None => TransactTo::Create,
                };
                tx_env.value = tx_data.value;
                tx_env.data = tx_data.input.clone().into();
                tx_env.gas_limit = tx_data.gas_limit;
                tx_env.gas_price = U256::from(tx_data.gas_price.unwrap_or(0));
                tx_env.nonce = Some(tx_data.nonce);
                tx_env.chain_id = Some(tx_data.chain_id);
            })
            .modify_cfg_env(|cfg| {
                cfg.chain_id = self.chain_id;
            })
            .build();

        let result = evm
            .transact_commit()
            .map_err(|e| EvmError::Internal(format!("{:?}", e)))?;

        match result {
            ExecutionResult::Success {
                gas_used,
                output,
                logs,
                ..
            } => {
                let (output_data, contract_address) = match output {
                    Output::Call(data) => (data.to_vec(), None),
                    Output::Create(data, addr) => (data.to_vec(), addr),
                };

                let converted_logs: Vec<Log> = logs
                    .into_iter()
                    .map(|log| Log {
                        address: log.address,
                        topics: log.topics().to_vec(),
                        data: log.data.data.to_vec(),
                    })
                    .collect();

                debug!("EVM execution success: gas_used={}", gas_used);

                Ok(ExecutionOutcome {
                    success: true,
                    gas_used,
                    output: output_data,
                    logs: converted_logs,
                    contract_address,
                })
            }
            ExecutionResult::Revert { gas_used, output } => Ok(ExecutionOutcome {
                success: false,
                gas_used,
                output: output.to_vec(),
                logs: vec![],
                contract_address: None,
            }),
            ExecutionResult::Halt { reason, gas_used } => Err(EvmError::Halt(format!(
                "{:?}, gas_used={}",
                reason, gas_used
            ))),
        }
    }

    /// Load an account from WorldState into the EVM's in-memory DB.
    /// Call this before executing transactions to ensure the EVM has
    /// the correct account state.
    pub fn load_account(&mut self, address: Address, world_state: &WorldState) {
        if let Ok(Some(account)) = world_state.get_account(&address) {
            let info = AccountInfo {
                balance: account.balance,
                nonce: account.nonce,
                code_hash: B256::ZERO,
                code: None,
            };
            self.db.insert_account_info(address, info);
        }
    }

    /// Flush EVM state changes back to WorldState after block execution.
    /// This syncs balances and nonces modified by EVM transactions.
    pub fn flush_to_state(&self, world_state: &WorldState) -> Result<(), EvmError> {
        for (address, db_account) in &self.db.accounts {
            let info = &db_account.info;
            let mut account = world_state
                .get_or_create_account(address)
                .map_err(|e| EvmError::Internal(format!("State read error: {e}")))?;
            account.balance = info.balance;
            account.nonce = info.nonce;
            world_state
                .put_account(address, &account)
                .map_err(|e| EvmError::Internal(format!("State write error: {e}")))?;
        }
        Ok(())
    }

    /// Reset the in-memory EVM database for a new block.
    pub fn reset(&mut self) {
        self.db = CacheDB::new(EmptyDB::default());
    }

    /// Get an account's balance from the EVM state.
    pub fn get_balance(&self, address: Address) -> U256 {
        self.db
            .accounts
            .get(&address)
            .map(|a| a.info.balance)
            .unwrap_or(U256::ZERO)
    }

    /// Get an account's nonce from the EVM state.
    pub fn get_nonce(&self, address: Address) -> u64 {
        self.db
            .accounts
            .get(&address)
            .map(|a| a.info.nonce)
            .unwrap_or(0)
    }

    /// Get an account's code from the EVM state.
    pub fn get_code(&self, address: Address) -> Vec<u8> {
        self.db
            .accounts
            .get(&address)
            .and_then(|a| a.info.code.as_ref())
            .map(|c| c.bytes_slice().to_vec())
            .unwrap_or_default()
    }

    /// Simulate a call without committing state changes (eth_call).
    /// Returns the output bytes or an error.
    pub fn simulate_call(
        &self,
        from: Address,
        to: Option<Address>,
        value: U256,
        data: Vec<u8>,
        gas_limit: u64,
    ) -> Result<ExecutionOutcome, EvmError> {
        // Clone the DB so we don't mutate the original
        let mut db = self.db.clone();

        let mut evm = Evm::builder()
            .with_db(&mut db)
            .modify_tx_env(|tx_env| {
                tx_env.caller = from;
                tx_env.transact_to = match to {
                    Some(addr) => TransactTo::Call(addr),
                    None => TransactTo::Create,
                };
                tx_env.value = value;
                tx_env.data = data.into();
                tx_env.gas_limit = gas_limit;
                tx_env.gas_price = U256::ZERO;
                tx_env.nonce = None; // Skip nonce check for calls
                tx_env.chain_id = Some(self.chain_id);
            })
            .modify_cfg_env(|cfg| {
                cfg.chain_id = self.chain_id;
            })
            .build();

        let result = evm
            .transact()
            .map_err(|e| EvmError::Internal(format!("{:?}", e)))?;

        match result.result {
            ExecutionResult::Success {
                gas_used,
                output,
                logs,
                ..
            } => {
                let (output_data, contract_address) = match output {
                    Output::Call(data) => (data.to_vec(), None),
                    Output::Create(data, addr) => (data.to_vec(), addr),
                };
                let converted_logs: Vec<Log> = logs
                    .into_iter()
                    .map(|log| Log {
                        address: log.address,
                        topics: log.topics().to_vec(),
                        data: log.data.data.to_vec(),
                    })
                    .collect();
                Ok(ExecutionOutcome {
                    success: true,
                    gas_used,
                    output: output_data,
                    logs: converted_logs,
                    contract_address,
                })
            }
            ExecutionResult::Revert { gas_used, output } => Ok(ExecutionOutcome {
                success: false,
                gas_used,
                output: output.to_vec(),
                logs: vec![],
                contract_address: None,
            }),
            ExecutionResult::Halt { reason, gas_used } => Err(EvmError::Halt(format!(
                "{:?}, gas_used={}",
                reason, gas_used
            ))),
        }
    }

    /// Estimate the gas needed for a transaction (eth_estimateGas).
    pub fn estimate_gas(
        &self,
        from: Address,
        to: Option<Address>,
        value: U256,
        data: Vec<u8>,
    ) -> Result<u64, EvmError> {
        // Run with high gas limit to see actual usage
        let result = self.simulate_call(from, to, value, data, 30_000_000)?;
        // Add 20% buffer to the gas used
        let estimated = result.gas_used + result.gas_used / 5;
        Ok(estimated.max(21_000)) // Minimum 21k gas
    }
}
