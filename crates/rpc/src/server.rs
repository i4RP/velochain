//! JSON-RPC server setup and configuration.
//!
//! Supports both HTTP and WebSocket connections on the same port.
//! WebSocket clients can use subscription endpoints for real-time
//! game state streaming.

use crate::admin_api::{AdminApiImpl, AdminApiServer};
use crate::error::RpcError;
use crate::eth_api::{EthApiImpl, EthApiServer};
use crate::game_api::{GameApiImpl, GameApiServer};
use crate::session::{SessionApiImpl, SessionApiServer, SessionManager};
use crate::subscriptions::{
    EventSender, EthSubscriptionApiImpl, EthSubscriptionApiServer, SubscriptionApiImpl,
    SubscriptionApiServer,
};
use jsonrpsee::server::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use velochain_game_engine::GameWorld;
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

/// RPC server configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Address to bind the RPC server to.
    pub addr: SocketAddr,
    /// Chain ID for eth_chainId responses.
    pub chain_id: u64,
    /// Maximum number of WebSocket connections.
    pub max_ws_connections: u32,
    /// Whether to enable admin API endpoints.
    pub enable_admin: bool,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8545".parse().unwrap(),
            chain_id: velochain_primitives::DEFAULT_CHAIN_ID,
            max_ws_connections: 100,
            enable_admin: false,
        }
    }
}

/// The JSON-RPC server (HTTP + WebSocket).
pub struct RpcServer;

impl RpcServer {
    /// Start the RPC server with references to all subsystems.
    /// Supports both HTTP and WebSocket on the same port.
    pub async fn start(
        config: RpcConfig,
        db: Arc<Database>,
        state: Arc<WorldState>,
        game_world: Arc<GameWorld>,
        txpool: Arc<TransactionPool>,
        event_tx: Option<EventSender>,
        session_manager: Option<Arc<SessionManager>>,
    ) -> Result<SocketAddr, RpcError> {
        let server = Server::builder()
            .max_connections(config.max_ws_connections)
            .build(config.addr)
            .await
            .map_err(|e| RpcError::Server(e.to_string()))?;

        let eth_api = EthApiImpl::new(config.chain_id, db.clone(), state, txpool.clone());
        let game_api = GameApiImpl::new(game_world.clone(), txpool.clone(), config.chain_id);

        let mut module = jsonrpsee::RpcModule::new(());
        module
            .merge(eth_api.into_rpc())
            .map_err(|e| RpcError::Server(e.to_string()))?;
        module
            .merge(game_api.into_rpc())
            .map_err(|e| RpcError::Server(e.to_string()))?;

        // Register session API if session manager is provided
        if let Some(sm) = session_manager {
            let session_api = SessionApiImpl::new(sm);
            module
                .merge(session_api.into_rpc())
                .map_err(|e| RpcError::Server(e.to_string()))?;
            info!("Session management API endpoints registered");
        }

        // Register admin API if enabled
        if config.enable_admin {
            let admin_api = AdminApiImpl::new(
                config.chain_id,
                db.clone(),
                game_world.clone(),
                txpool.clone(),
            );
            module
                .merge(admin_api.into_rpc())
                .map_err(|e| RpcError::Server(e.to_string()))?;
            info!("Admin API endpoints registered");
        }

        // Register subscription APIs if event channel is provided
        if let Some(event_tx) = event_tx {
            let sub_api =
                SubscriptionApiImpl::new(event_tx.clone(), game_world.clone(), db.clone());
            module
                .merge(sub_api.into_rpc())
                .map_err(|e| RpcError::Server(e.to_string()))?;

            let eth_sub_api = EthSubscriptionApiImpl::new(event_tx, game_world, db);
            module
                .merge(eth_sub_api.into_rpc())
                .map_err(|e| RpcError::Server(e.to_string()))?;

            info!("WebSocket subscription endpoints registered");
        }

        let addr = server
            .local_addr()
            .map_err(|e| RpcError::Server(e.to_string()))?;
        let handle = server.start(module);

        info!("JSON-RPC server started on {} (HTTP + WebSocket)", addr);

        // Keep server running in background
        tokio::spawn(async move {
            handle.stopped().await;
        });

        Ok(addr)
    }
}
