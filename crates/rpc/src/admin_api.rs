//! Admin and network info JSON-RPC API.
//!
//! Provides node status, peer information, and health check endpoints
//! for monitoring and administration.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use velochain_game_engine::GameWorld;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

/// Node information response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfo {
    /// Node software name and version.
    pub name: String,
    /// Chain ID.
    pub chain_id: u64,
    /// Current block number.
    pub block_number: u64,
    /// Current game tick.
    pub game_tick: u64,
    /// Number of entities in the game world.
    pub entity_count: usize,
    /// Number of players in the game world.
    pub player_count: usize,
    /// Number of pending transactions.
    pub pending_tx_count: usize,
    /// Node uptime in seconds.
    pub uptime_secs: u64,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    /// Whether the node is healthy.
    pub healthy: bool,
    /// Current block number.
    pub block_number: u64,
    /// Current game tick.
    pub game_tick: u64,
    /// Pending transaction count.
    pub pending_tx_count: usize,
    /// Database accessible.
    pub db_ok: bool,
}

/// Admin JSON-RPC API.
#[rpc(server, namespace = "admin")]
pub trait AdminApi {
    /// Returns detailed node information.
    #[method(name = "nodeInfo")]
    async fn node_info(&self) -> RpcResult<NodeInfo>;

    /// Returns a health check status.
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<HealthStatus>;

    /// Returns the current peer count (placeholder for future P2P integration).
    #[method(name = "peerCount")]
    async fn peer_count(&self) -> RpcResult<usize>;
}

/// Admin API implementation.
pub struct AdminApiImpl {
    chain_id: u64,
    db: Arc<Database>,
    game_world: Arc<GameWorld>,
    txpool: Arc<TransactionPool>,
    start_time: Instant,
}

impl AdminApiImpl {
    pub fn new(
        chain_id: u64,
        db: Arc<Database>,
        game_world: Arc<GameWorld>,
        txpool: Arc<TransactionPool>,
    ) -> Self {
        Self {
            chain_id,
            db,
            game_world,
            txpool,
            start_time: Instant::now(),
        }
    }
}

#[jsonrpsee::core::async_trait]
impl AdminApiServer for AdminApiImpl {
    async fn node_info(&self) -> RpcResult<NodeInfo> {
        let block_number = self
            .db
            .get_latest_block_number()
            .map_err(|e| internal_err(format!("Storage error: {e}")))?
            .unwrap_or(0);

        Ok(NodeInfo {
            name: format!("VeloChain/v{}", env!("CARGO_PKG_VERSION")),
            chain_id: self.chain_id,
            block_number,
            game_tick: self.game_world.current_tick(),
            entity_count: self.game_world.entity_count(),
            player_count: self.game_world.player_count(),
            pending_tx_count: self.txpool.pending_count(),
            uptime_secs: self.start_time.elapsed().as_secs(),
        })
    }

    async fn health(&self) -> RpcResult<HealthStatus> {
        let block_number = self
            .db
            .get_latest_block_number()
            .unwrap_or(Some(0))
            .unwrap_or(0);

        // Simple DB health check: try reading metadata
        let db_ok = self.db.get_meta("latest_block_number").is_ok();

        Ok(HealthStatus {
            healthy: db_ok,
            block_number,
            game_tick: self.game_world.current_tick(),
            pending_tx_count: self.txpool.pending_count(),
            db_ok,
        })
    }

    async fn peer_count(&self) -> RpcResult<usize> {
        // Placeholder: peer count tracking will be integrated with network layer
        Ok(0)
    }
}

fn internal_err(msg: String) -> jsonrpsee::types::ErrorObjectOwned {
    jsonrpsee::types::ErrorObjectOwned::owned(-32000, msg, None::<()>)
}
