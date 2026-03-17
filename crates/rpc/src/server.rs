//! JSON-RPC server setup and configuration.

use crate::error::RpcError;
use crate::eth_api::{EthApiImpl, EthApiServer};
use crate::game_api::{GameApiImpl, GameApiServer};
use jsonrpsee::server::Server;
use std::net::SocketAddr;
use tracing::info;

/// RPC server configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Address to bind the RPC server to.
    pub addr: SocketAddr,
    /// Chain ID for eth_chainId responses.
    pub chain_id: u64,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8545".parse().unwrap(),
            chain_id: velochain_primitives::DEFAULT_CHAIN_ID,
        }
    }
}

/// The JSON-RPC server.
pub struct RpcServer;

impl RpcServer {
    /// Start the RPC server.
    pub async fn start(config: RpcConfig) -> Result<SocketAddr, RpcError> {
        let server = Server::builder()
            .build(config.addr)
            .await
            .map_err(|e| RpcError::Server(e.to_string()))?;

        let eth_api = EthApiImpl::new(config.chain_id);
        let game_api = GameApiImpl::new();

        let mut module = jsonrpsee::RpcModule::new(());
        module
            .merge(eth_api.into_rpc())
            .map_err(|e| RpcError::Server(e.to_string()))?;
        module
            .merge(game_api.into_rpc())
            .map_err(|e| RpcError::Server(e.to_string()))?;

        let addr = server.local_addr().map_err(|e| RpcError::Server(e.to_string()))?;
        let handle = server.start(module);

        info!("JSON-RPC server started on {}", addr);

        // Keep server running in background
        tokio::spawn(async move {
            handle.stopped().await;
        });

        Ok(addr)
    }
}
