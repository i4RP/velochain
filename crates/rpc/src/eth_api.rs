//! Ethereum-compatible JSON-RPC API implementation.

use alloy_primitives::Address;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use std::sync::Arc;
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
}

/// Ethereum API implementation backed by actual chain state.
pub struct EthApiImpl {
    chain_id: u64,
    db: Arc<Database>,
    state: Arc<WorldState>,
    txpool: Arc<TransactionPool>,
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
