//! Game-specific JSON-RPC API for querying and interacting with the game world.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use velochain_game_engine::GameWorld;
use velochain_primitives::transaction::{GameAction, Transaction};
use velochain_primitives::Keypair;
use velochain_txpool::TransactionPool;

/// Re-export PlayerInfo from game engine for RPC responses.
pub use velochain_game_engine::PlayerInfo;

/// World information response.
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

/// Game API implementation backed by actual game world.
pub struct GameApiImpl {
    game_world: Arc<GameWorld>,
    txpool: Arc<TransactionPool>,
    chain_id: u64,
}

impl GameApiImpl {
    pub fn new(game_world: Arc<GameWorld>, txpool: Arc<TransactionPool>, chain_id: u64) -> Self {
        Self {
            game_world,
            txpool,
            chain_id,
        }
    }
}

#[jsonrpsee::core::async_trait]
impl GameApiServer for GameApiImpl {
    async fn get_world_info(&self) -> RpcResult<WorldInfo> {
        Ok(WorldInfo {
            current_tick: self.game_world.current_tick(),
            entity_count: self.game_world.entity_count(),
            player_count: self.game_world.player_count(),
            seed: self.game_world.seed(),
        })
    }

    async fn get_player(&self, address: String) -> RpcResult<Option<PlayerInfo>> {
        Ok(self.game_world.get_player_info(&address))
    }

    async fn get_current_tick(&self) -> RpcResult<u64> {
        Ok(self.game_world.current_tick())
    }

    async fn get_entity_count(&self) -> RpcResult<usize> {
        Ok(self.game_world.entity_count())
    }

    async fn submit_action(
        &self,
        action_type: String,
        params: serde_json::Value,
    ) -> RpcResult<String> {
        let action = parse_game_action(&action_type, params)?;

        // Create a temporary keypair for unsigned game actions submitted via RPC.
        // In production, the client should sign and submit via eth_sendRawTransaction.
        let keypair = Keypair::random();
        let tx = Transaction::new_game_action(self.chain_id, 0, action);
        let signed = tx.sign(&keypair).map_err(|e| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to sign action: {e}"),
                None::<()>,
            )
        })?;

        let hash = signed.hash;
        self.txpool.add_transaction(signed).map_err(|e| {
            jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("TxPool error: {e}"),
                None::<()>,
            )
        })?;

        Ok(format!("{:?}", hash))
    }
}

fn parse_game_action(
    action_type: &str,
    params: serde_json::Value,
) -> RpcResult<GameAction> {
    match action_type {
        "move" => {
            let x = params.get("x").and_then(|v| v.as_i64()).unwrap_or(0);
            let y = params.get("y").and_then(|v| v.as_i64()).unwrap_or(0);
            let z = params.get("z").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok(GameAction::Move { x, y, z })
        }
        "attack" => {
            let target = params
                .get("target_entity_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    jsonrpsee::types::ErrorObjectOwned::owned(
                        -32602,
                        "Missing target_entity_id",
                        None::<()>,
                    )
                })?;
            Ok(GameAction::Attack {
                target_entity_id: target,
            })
        }
        "chat" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(GameAction::Chat { message })
        }
        "respawn" => Ok(GameAction::Respawn),
        _ => Err(jsonrpsee::types::ErrorObjectOwned::owned(
            -32602,
            format!("Unknown action type: {action_type}"),
            None::<()>,
        )),
    }
}
