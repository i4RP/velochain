//! Game-specific JSON-RPC API for querying and interacting with the game world.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use serde::{Deserialize, Serialize};

/// Game world query response types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub entity_id: u64,
    pub address: String,
    pub position: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    pub level: u32,
    pub is_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldInfo {
    pub current_tick: u64,
    pub entity_count: usize,
    pub player_count: usize,
    pub seed: u64,
}

/// Game-specific JSON-RPC API.
#[rpc(server, namespace = "game")]
pub trait GameApi {
    /// Get current world information.
    #[method(name = "getWorldInfo")]
    async fn get_world_info(&self) -> RpcResult<WorldInfo>;

    /// Get player information by address.
    #[method(name = "getPlayer")]
    async fn get_player(&self, address: String) -> RpcResult<Option<PlayerInfo>>;

    /// Get current game tick.
    #[method(name = "getCurrentTick")]
    async fn get_current_tick(&self) -> RpcResult<u64>;

    /// Get entity count.
    #[method(name = "getEntityCount")]
    async fn get_entity_count(&self) -> RpcResult<usize>;

    /// Submit a game action (move, attack, etc.).
    #[method(name = "submitAction")]
    async fn submit_action(&self, action_type: String, params: serde_json::Value) -> RpcResult<String>;
}

/// Game API implementation.
pub struct GameApiImpl;

impl GameApiImpl {
    pub fn new() -> Self {
        Self
    }
}

#[jsonrpsee::core::async_trait]
impl GameApiServer for GameApiImpl {
    async fn get_world_info(&self) -> RpcResult<WorldInfo> {
        // TODO: Query actual game world
        Ok(WorldInfo {
            current_tick: 0,
            entity_count: 0,
            player_count: 0,
            seed: 42,
        })
    }

    async fn get_player(&self, _address: String) -> RpcResult<Option<PlayerInfo>> {
        // TODO: Query actual game world
        Ok(None)
    }

    async fn get_current_tick(&self) -> RpcResult<u64> {
        // TODO: Return actual tick
        Ok(0)
    }

    async fn get_entity_count(&self) -> RpcResult<usize> {
        // TODO: Return actual count
        Ok(0)
    }

    async fn submit_action(
        &self,
        _action_type: String,
        _params: serde_json::Value,
    ) -> RpcResult<String> {
        // TODO: Parse action, create tx, add to pool
        Err(jsonrpsee::types::ErrorObjectOwned::owned(
            -32000,
            "Not yet implemented",
            None::<()>,
        ))
    }
}
