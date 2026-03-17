//! Ethereum-compatible JSON-RPC API implementation.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

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

/// Ethereum API implementation.
pub struct EthApiImpl {
    chain_id: u64,
}

impl EthApiImpl {
    pub fn new(chain_id: u64) -> Self {
        Self { chain_id }
    }
}

#[jsonrpsee::core::async_trait]
impl EthApiServer for EthApiImpl {
    async fn chain_id(&self) -> RpcResult<String> {
        Ok(format!("0x{:x}", self.chain_id))
    }

    async fn block_number(&self) -> RpcResult<String> {
        // TODO: Return actual block number from state
        Ok("0x0".to_string())
    }

    async fn get_balance(&self, _address: String, _block: Option<String>) -> RpcResult<String> {
        // TODO: Query state for balance
        Ok("0x0".to_string())
    }

    async fn get_transaction_count(
        &self,
        _address: String,
        _block: Option<String>,
    ) -> RpcResult<String> {
        // TODO: Query state for nonce
        Ok("0x0".to_string())
    }

    async fn send_raw_transaction(&self, _data: String) -> RpcResult<String> {
        // TODO: Decode, validate, and add to tx pool
        Err(jsonrpsee::types::ErrorObjectOwned::owned(
            -32000,
            "Not yet implemented",
            None::<()>,
        ))
    }

    async fn gas_price(&self) -> RpcResult<String> {
        Ok("0x3b9aca00".to_string()) // 1 gwei
    }

    async fn net_version(&self) -> RpcResult<String> {
        Ok(self.chain_id.to_string())
    }
}
