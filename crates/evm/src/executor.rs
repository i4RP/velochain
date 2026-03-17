//! EVM executor using revm.
//!
//! Executes EVM transactions against the current world state.
//! Used for asset management operations (transfers, NFT minting, etc.)

use crate::error::EvmError;
use alloy_primitives::{Address, B256, U256};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, Bytecode, ExecutionResult, Output, TransactTo,
    },
    Evm,
};
use tracing::debug;
use velochain_primitives::SignedTransaction;

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

        let tx_data = &tx.transaction;

        let mut evm = Evm::builder()
            .with_db(&mut self.db)
            .modify_tx_env(|tx_env| {
                // For now use a placeholder caller since signature recovery isn't implemented yet
                tx_env.caller = Address::ZERO;
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
            ExecutionResult::Revert { gas_used, output } => {
                Ok(ExecutionOutcome {
                    success: false,
                    gas_used,
                    output: output.to_vec(),
                    logs: vec![],
                    contract_address: None,
                })
            }
            ExecutionResult::Halt { reason, gas_used } => {
                Err(EvmError::Halt(format!("{:?}, gas_used={}", reason, gas_used)))
            }
        }
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
}
