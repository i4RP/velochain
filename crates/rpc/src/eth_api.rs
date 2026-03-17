//! Ethereum-compatible JSON-RPC API implementation.

use alloy_primitives::{Address, U256};
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use velochain_evm::EvmExecutor;
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

/// Ethereum-compatible JSON-RPC API.
#[rpc(server, namespace = "eth")]
pub trait EthApi {
    /// Returns the chain ID.
    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<String>;

    /// Returns the current block number.
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<String>;

    /// Returns the balance of an account.
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: String, block: Option<String>) -> RpcResult<String>;

    /// Returns the nonce of an account.
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: String,
        block: Option<String>,
    ) -> RpcResult<String>;

    /// Sends a signed transaction.
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, data: String) -> RpcResult<String>;

    /// Returns the gas price.
    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> RpcResult<String>;

    /// Returns the current network version.
    #[method(name = "net_version")]
    async fn net_version(&self) -> RpcResult<String>;

    /// Returns block information by number.
    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        number: String,
        full_txs: bool,
    ) -> RpcResult<Option<RpcBlock>>;

    /// Returns block information by hash.
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        hash: String,
        full_txs: bool,
    ) -> RpcResult<Option<RpcBlock>>;

    /// Returns a transaction receipt by transaction hash.
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(
        &self,
        hash: String,
    ) -> RpcResult<Option<RpcReceipt>>;

    /// Executes a call without creating a transaction (read-only).
    #[method(name = "call")]
    async fn call(&self, tx: CallRequest, block: Option<String>) -> RpcResult<String>;

    /// Estimates the gas needed for a transaction.
    #[method(name = "estimateGas")]
    async fn estimate_gas(&self, tx: CallRequest, block: Option<String>) -> RpcResult<String>;

    /// Returns the code at a given address.
    #[method(name = "getCode")]
    async fn get_code(&self, address: String, block: Option<String>) -> RpcResult<String>;

    /// Returns logs matching a filter.
    #[method(name = "getLogs")]
    async fn get_logs(&self, filter: LogFilter) -> RpcResult<Vec<RpcLog>>;
}

/// Block information returned by RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlock {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub beneficiary: String,
    pub state_root: String,
    pub transactions_root: String,
    pub receipts_root: String,
    pub game_tick: u64,
    pub game_state_root: String,
    pub difficulty: String,
    pub transactions: Vec<String>,
}

impl RpcBlock {
    pub fn from_block(
        header: &velochain_primitives::BlockHeader,
        body: &velochain_primitives::BlockBody,
    ) -> Self {
        let tx_hashes: Vec<String> = body
            .transactions
            .iter()
            .map(|tx| format!("0x{}", hex::encode(tx.hash.as_slice())))
            .collect();
        Self {
            number: format!("0x{:x}", header.number),
            hash: format!("0x{}", hex::encode(header.hash().as_slice())),
            parent_hash: format!("0x{}", hex::encode(header.parent_hash.as_slice())),
            timestamp: format!("0x{:x}", header.timestamp),
            gas_limit: format!("0x{:x}", header.gas_limit),
            gas_used: format!("0x{:x}", header.gas_used),
            beneficiary: format!("0x{}", hex::encode(header.beneficiary.as_slice())),
            state_root: format!("0x{}", hex::encode(header.state_root.as_slice())),
            transactions_root: format!("0x{}", hex::encode(header.transactions_root.as_slice())),
            receipts_root: format!("0x{}", hex::encode(header.receipts_root.as_slice())),
            game_tick: header.game_tick,
            game_state_root: format!("0x{}", hex::encode(header.game_state_root.as_slice())),
            difficulty: format!("0x{:x}", header.difficulty),
            transactions: tx_hashes,
        }
    }
}

/// Transaction receipt returned by RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcReceipt {
    pub transaction_hash: String,
    pub block_number: String,
    pub block_hash: String,
    pub transaction_index: String,
    pub success: bool,
    pub gas_used: String,
    pub cumulative_gas_used: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<String>,
    pub logs: Vec<serde_json::Value>,
}

/// Call request for eth_call / eth_estimateGas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub gas: Option<String>,
}

/// Log filter for eth_getLogs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilter {
    #[serde(default)]
    pub from_block: Option<String>,
    #[serde(default)]
    pub to_block: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub topics: Option<Vec<Option<String>>>,
}

/// A log entry returned by eth_getLogs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub log_index: String,
}

/// Ethereum API implementation backed by actual chain state.
pub struct EthApiImpl {
    chain_id: u64,
    db: Arc<Database>,
    state: Arc<WorldState>,
    txpool: Arc<TransactionPool>,
    evm: Mutex<EvmExecutor>,
}

impl EthApiImpl {
    pub fn new(
        chain_id: u64,
        db: Arc<Database>,
        state: Arc<WorldState>,
        txpool: Arc<TransactionPool>,
    ) -> Self {
        Self {
            chain_id,
            db,
            state,
            txpool,
            evm: Mutex::new(EvmExecutor::new(chain_id)),
        }
    }
}

#[jsonrpsee::core::async_trait]
impl EthApiServer for EthApiImpl {
    async fn chain_id(&self) -> RpcResult<String> {
        Ok(format!("0x{:x}", self.chain_id))
    }

    async fn block_number(&self) -> RpcResult<String> {
        let number = self
            .db
            .get_latest_block_number()
            .map_err(|e| internal_err(format!("Storage error: {e}")))?
            .unwrap_or(0);
        Ok(format!("0x{:x}", number))
    }

    async fn get_balance(&self, address: String, _block: Option<String>) -> RpcResult<String> {
        let addr = parse_address(&address)?;
        let balance = self
            .state
            .get_balance(&addr)
            .map_err(|e| internal_err(format!("State error: {e}")))?;
        Ok(format!("0x{:x}", balance))
    }

    async fn get_transaction_count(
        &self,
        address: String,
        _block: Option<String>,
    ) -> RpcResult<String> {
        let addr = parse_address(&address)?;
        let nonce = self
            .state
            .get_nonce(&addr)
            .map_err(|e| internal_err(format!("State error: {e}")))?;
        Ok(format!("0x{:x}", nonce))
    }

    async fn send_raw_transaction(&self, data: String) -> RpcResult<String> {
        // Decode the JSON-encoded signed transaction
        let hex_data = data.strip_prefix("0x").unwrap_or(&data);
        let tx_bytes = hex::decode(hex_data).map_err(|e| {
            invalid_params(format!("Invalid hex data: {e}"))
        })?;
        let signed_tx: velochain_primitives::SignedTransaction =
            serde_json::from_slice(&tx_bytes).map_err(|e| {
                invalid_params(format!("Invalid transaction encoding: {e}"))
            })?;

        // Verify signature by recovering sender
        signed_tx.sender().map_err(|e| {
            invalid_params(format!("Invalid signature: {e}"))
        })?;

        let hash = signed_tx.hash;
        self.txpool.add_transaction(signed_tx).map_err(|e| {
            internal_err(format!("TxPool error: {e}"))
        })?;

        Ok(format!("{:?}", hash))
    }

    async fn gas_price(&self) -> RpcResult<String> {
        Ok("0x3b9aca00".to_string()) // 1 gwei
    }

    async fn net_version(&self) -> RpcResult<String> {
        Ok(self.chain_id.to_string())
    }

    async fn get_block_by_number(
        &self,
        number: String,
        _full_txs: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let block_num = if number == "latest" {
            self.db
                .get_latest_block_number()
                .map_err(|e| internal_err(format!("Storage error: {e}")))?
                .unwrap_or(0)
        } else {
            let s = number.strip_prefix("0x").unwrap_or(&number);
            u64::from_str_radix(s, 16).map_err(|e| invalid_params(format!("Invalid block number: {e}")))?
        };

        let hash = match self.db.get_block_hash_by_number(block_num)
            .map_err(|e| internal_err(format!("Storage error: {e}")))? {
            Some(h) => h,
            None => return Ok(None),
        };

        let header = match self.db.get_header(&hash)
            .map_err(|e| internal_err(format!("Storage error: {e}")))? {
            Some(h) => h,
            None => return Ok(None),
        };

        let body = self.db.get_body(&hash)
            .map_err(|e| internal_err(format!("Storage error: {e}")))?
            .unwrap_or_else(|| velochain_primitives::BlockBody { transactions: vec![] });

        Ok(Some(RpcBlock::from_block(&header, &body)))
    }

    async fn get_block_by_hash(
        &self,
        hash: String,
        _full_txs: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let hash_hex = hash.strip_prefix("0x").unwrap_or(&hash);
        let hash_bytes = hex::decode(hash_hex)
            .map_err(|e| invalid_params(format!("Invalid hash: {e}")))?;
        if hash_bytes.len() != 32 {
            return Err(invalid_params("Hash must be 32 bytes".to_string()));
        }
        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(&hash_bytes);

        let header = match self.db.get_header(&hash_arr)
            .map_err(|e| internal_err(format!("Storage error: {e}")))? {
            Some(h) => h,
            None => return Ok(None),
        };

        let body = self.db.get_body(&hash_arr)
            .map_err(|e| internal_err(format!("Storage error: {e}")))?
            .unwrap_or_else(|| velochain_primitives::BlockBody { transactions: vec![] });

        Ok(Some(RpcBlock::from_block(&header, &body)))
    }

    async fn get_transaction_receipt(
        &self,
        hash: String,
    ) -> RpcResult<Option<RpcReceipt>> {
        let hash_hex = hash.strip_prefix("0x").unwrap_or(&hash);
        let hash_bytes = hex::decode(hash_hex)
            .map_err(|e| invalid_params(format!("Invalid hash: {e}")))?;
        if hash_bytes.len() != 32 {
            return Err(invalid_params("Hash must be 32 bytes".to_string()));
        }
        let mut hash_arr = [0u8; 32];
        hash_arr.copy_from_slice(&hash_bytes);

        match self.db.get_receipt(&hash_arr)
            .map_err(|e| internal_err(format!("Storage error: {e}")))? {
            Some(data) => {
                // Parse the stored receipt (node's TransactionReceipt format)
                let stored: serde_json::Value = serde_json::from_slice(&data)
                    .map_err(|e| internal_err(format!("Receipt decode: {e}")))?;
                let receipt = RpcReceipt {
                    transaction_hash: format!("0x{}", stored["tx_hash"].as_str().unwrap_or("").trim_start_matches("0x")),
                    block_number: format!("0x{:x}", stored["block_number"].as_u64().unwrap_or(0)),
                    block_hash: format!("0x{}", stored["block_hash"].as_str().unwrap_or("").trim_start_matches("0x")),
                    transaction_index: format!("0x{:x}", stored["index"].as_u64().unwrap_or(0)),
                    success: stored["success"].as_bool().unwrap_or(false),
                    gas_used: format!("0x{:x}", stored["gas_used"].as_u64().unwrap_or(0)),
                    cumulative_gas_used: format!("0x{:x}", stored["cumulative_gas_used"].as_u64().unwrap_or(0)),
                    contract_address: stored["contract_address"].as_str().map(|s| s.to_string()),
                    logs: stored["logs"].as_array().cloned().unwrap_or_default(),
                };
                Ok(Some(receipt))
            }
            None => Ok(None),
        }
    }

    async fn call(&self, tx: CallRequest, _block: Option<String>) -> RpcResult<String> {
        let from = match &tx.from {
            Some(addr) => parse_address(addr)?,
            None => Address::ZERO,
        };
        let to = match &tx.to {
            Some(addr) => Some(parse_address(addr)?),
            None => None,
        };
        let value = parse_u256_hex(tx.value.as_deref().unwrap_or("0x0"))?;
        let data = parse_hex_data(tx.data.as_deref().unwrap_or("0x"))?;
        let gas_limit = parse_u64_hex(tx.gas.as_deref().unwrap_or("0x1c9c380"))?; // 30M default

        let evm = self.evm.lock();
        let result = evm.simulate_call(from, to, value, data, gas_limit)
            .map_err(|e| internal_err(format!("EVM call failed: {e}")))?;

        Ok(format!("0x{}", hex::encode(&result.output)))
    }

    async fn estimate_gas(&self, tx: CallRequest, _block: Option<String>) -> RpcResult<String> {
        let from = match &tx.from {
            Some(addr) => parse_address(addr)?,
            None => Address::ZERO,
        };
        let to = match &tx.to {
            Some(addr) => Some(parse_address(addr)?),
            None => None,
        };
        let value = parse_u256_hex(tx.value.as_deref().unwrap_or("0x0"))?;
        let data = parse_hex_data(tx.data.as_deref().unwrap_or("0x"))?;

        let evm = self.evm.lock();
        let gas = evm.estimate_gas(from, to, value, data)
            .map_err(|e| internal_err(format!("Gas estimation failed: {e}")))?;

        Ok(format!("0x{:x}", gas))
    }

    async fn get_code(&self, address: String, _block: Option<String>) -> RpcResult<String> {
        let _addr = parse_address(&address)?;
        // For now, return empty code as contract storage is in the EVM cache
        // which is transient per block. Persistent code storage can be added later.
        Ok("0x".to_string())
    }

    async fn get_logs(&self, filter: LogFilter) -> RpcResult<Vec<RpcLog>> {
        let from_block = parse_u64_hex(filter.from_block.as_deref().unwrap_or("0x0"))?;
        let latest = self.db
            .get_latest_block_number()
            .map_err(|e| internal_err(format!("Storage error: {e}")))?
            .unwrap_or(0);
        let to_block = parse_u64_hex(filter.to_block.as_deref().unwrap_or("latest"))
            .unwrap_or(latest);

        let filter_addr = filter.address.as_ref().map(|a| parse_address(a)).transpose()?;

        let mut logs = Vec::new();
        for block_num in from_block..=to_block.min(from_block + 1000) {
            let block_hash = match self.db.get_block_hash_by_number(block_num)
                .map_err(|e| internal_err(format!("Storage error: {e}")))? {
                Some(h) => h,
                None => continue,
            };

            let body = match self.db.get_body(&block_hash)
                .map_err(|e| internal_err(format!("Storage error: {e}")))? {
                Some(b) => b,
                None => continue,
            };

            for (tx_idx, tx) in body.transactions.iter().enumerate() {
                if let Some(receipt_data) = self.db.get_receipt(tx.hash.as_ref())
                    .map_err(|e| internal_err(format!("Storage error: {e}")))? {
                    if let Ok(stored) = serde_json::from_slice::<serde_json::Value>(&receipt_data) {
                        if let Some(stored_logs) = stored["logs"].as_array() {
                            for (log_idx, log) in stored_logs.iter().enumerate() {
                                let log_addr = log["address"].as_str().unwrap_or("");
                                if let Some(ref fa) = filter_addr {
                                    if log_addr != format!("{:?}", fa) {
                                        continue;
                                    }
                                }
                                let topics: Vec<String> = log["topics"]
                                    .as_array()
                                    .map(|t| t.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                    .unwrap_or_default();
                                logs.push(RpcLog {
                                    address: log_addr.to_string(),
                                    topics,
                                    data: log["data"].as_str().unwrap_or("0x").to_string(),
                                    block_number: format!("0x{:x}", block_num),
                                    block_hash: format!("0x{}", hex::encode(block_hash)),
                                    transaction_hash: format!("{:?}", tx.hash),
                                    transaction_index: format!("0x{:x}", tx_idx),
                                    log_index: format!("0x{:x}", log_idx),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(logs)
    }
}

fn parse_address(s: &str) -> RpcResult<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|e| invalid_params(format!("Invalid address hex: {e}")))?;
    if bytes.len() != 20 {
        return Err(invalid_params(format!(
            "Address must be 20 bytes, got {}",
            bytes.len()
        )));
    }
    Ok(Address::from_slice(&bytes))
}

fn internal_err(msg: String) -> jsonrpsee::types::ErrorObjectOwned {
    jsonrpsee::types::ErrorObjectOwned::owned(-32000, msg, None::<()>)
}

fn invalid_params(msg: String) -> jsonrpsee::types::ErrorObjectOwned {
    jsonrpsee::types::ErrorObjectOwned::owned(-32602, msg, None::<()>)
}

fn parse_u256_hex(s: &str) -> RpcResult<U256> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.is_empty() || s == "0" {
        return Ok(U256::ZERO);
    }
    U256::from_str_radix(s, 16)
        .map_err(|e| invalid_params(format!("Invalid U256 hex: {e}")))
}

fn parse_hex_data(s: &str) -> RpcResult<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(s).map_err(|e| invalid_params(format!("Invalid hex data: {e}")))
}

fn parse_u64_hex(s: &str) -> RpcResult<u64> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.is_empty() || s == "0" {
        return Ok(0);
    }
    u64::from_str_radix(s, 16)
        .map_err(|e| invalid_params(format!("Invalid u64 hex: {e}")))
}
