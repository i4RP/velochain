//! Network layer for connecting to VeloChain node.
//!
//! Handles RPC calls and WebSocket subscriptions for real-time
//! game state synchronization. Uses a polling model compatible
//! with both native and WASM targets.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Plugin for network communication.
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NetworkState::default())
            .insert_resource(ServerWorldState::default())
            .add_systems(Update, process_server_updates);
    }
}

/// Network configuration.
#[derive(Resource, Clone)]
pub struct NetworkConfig {
    /// RPC/WebSocket endpoint URL.
    pub rpc_url: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            rpc_url: "ws://127.0.0.1:9545".to_string(),
        }
    }
}

/// Current network connection state.
#[derive(Resource, Default)]
pub struct NetworkState {
    /// Whether we're connected to the server.
    pub connected: bool,
    /// Current server tick.
    pub server_tick: u64,
    /// Pending actions to send to server.
    pub pending_actions: Vec<ClientAction>,
    /// Received entity updates from server.
    pub entity_updates: Vec<EntityUpdate>,
    /// Received chat messages from server.
    pub chat_messages: Vec<ServerChatMessage>,
}

/// Server world state snapshot (received periodically).
#[derive(Resource, Default, Clone)]
pub struct ServerWorldState {
    /// All known player positions and data.
    pub players: Vec<ServerPlayer>,
    /// All known NPC positions and data.
    pub npcs: Vec<ServerNpc>,
    /// Ground items.
    pub ground_items: Vec<ServerGroundItem>,
    /// Day/night state.
    pub is_day: bool,
    /// Current weather.
    pub weather: String,
}

/// Player data from server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerPlayer {
    pub entity_id: u64,
    pub address: String,
    pub position: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    pub level: u32,
    pub is_alive: bool,
}

/// NPC data from server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerNpc {
    pub entity_id: u64,
    pub npc_type: String,
    pub position: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    pub is_alive: bool,
}

/// Ground item data from server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerGroundItem {
    pub drop_id: u64,
    pub item_id: u32,
    pub quantity: u32,
    pub position: [f32; 3],
}

/// Chat message from server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerChatMessage {
    pub sender: String,
    pub text: String,
    pub tick: u64,
}

/// Client action to send to server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientAction {
    /// Move to position.
    Move { x: f32, y: f32, z: f32 },
    /// Attack an entity.
    Attack { target_entity_id: u64 },
    /// Send chat message.
    Chat { message: String },
    /// Pick up a ground item.
    Pickup { drop_id: u64 },
    /// Respawn after death.
    Respawn,
}

/// Entity update received from server.
#[derive(Clone, Debug)]
pub struct EntityUpdate {
    pub entity_id: u64,
    pub entity_type: String,
    pub position: [f32; 3],
    pub health: f32,
    pub max_health: f32,
    pub is_alive: bool,
}

/// Queue a move action based on player input.
pub fn queue_move_action(net_state: &mut NetworkState, x: f32, y: f32, z: f32) {
    net_state
        .pending_actions
        .push(ClientAction::Move { x, y, z });
}

/// Queue an attack action.
pub fn queue_attack_action(net_state: &mut NetworkState, target_id: u64) {
    net_state.pending_actions.push(ClientAction::Attack {
        target_entity_id: target_id,
    });
}

/// Queue a chat message.
pub fn queue_chat_action(net_state: &mut NetworkState, message: String) {
    net_state
        .pending_actions
        .push(ClientAction::Chat { message });
}

/// Process incoming server updates and apply them to the game world.
fn process_server_updates(
    mut net_state: ResMut<NetworkState>,
    mut chat_log: ResMut<crate::ui::ChatLog>,
) {
    // Process chat messages
    let messages: Vec<ServerChatMessage> = net_state.chat_messages.drain(..).collect();
    for msg in messages {
        chat_log.push(msg.sender, msg.text);
    }

    // Entity updates are consumed by the renderer system
    // They stay in net_state.entity_updates until consumed
}

/// Convert server position to client world position.
pub fn server_to_client_pos(server_pos: &[f32; 3]) -> Vec3 {
    Vec3::new(
        server_pos[0] * crate::terrain_render::TILE_PIXEL_SIZE,
        server_pos[1] * crate::terrain_render::TILE_PIXEL_SIZE,
        1.0, // Above terrain
    )
}

/// Convert client world position to server position.
pub fn client_to_server_pos(client_pos: &Vec3) -> [f32; 3] {
    [
        client_pos.x / crate::terrain_render::TILE_PIXEL_SIZE,
        client_pos.y / crate::terrain_render::TILE_PIXEL_SIZE,
        0.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_state_default() {
        let state = NetworkState::default();
        assert!(!state.connected);
        assert_eq!(state.server_tick, 0);
        assert!(state.pending_actions.is_empty());
    }

    #[test]
    fn test_queue_actions() {
        let mut state = NetworkState::default();
        queue_move_action(&mut state, 1.0, 2.0, 0.0);
        queue_attack_action(&mut state, 42);
        queue_chat_action(&mut state, "Hello!".to_string());
        assert_eq!(state.pending_actions.len(), 3);
    }

    #[test]
    fn test_position_conversion_roundtrip() {
        let server_pos = [10.0, 20.0, 0.0];
        let client_pos = server_to_client_pos(&server_pos);
        let back = client_to_server_pos(&client_pos);
        assert!((back[0] - server_pos[0]).abs() < 0.01);
        assert!((back[1] - server_pos[1]).abs() < 0.01);
    }

    #[test]
    fn test_server_world_state_default() {
        let state = ServerWorldState::default();
        assert!(state.players.is_empty());
        assert!(state.npcs.is_empty());
        assert!(state.ground_items.is_empty());
    }

    #[test]
    fn test_client_action_variants() {
        let actions = vec![
            ClientAction::Move {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            },
            ClientAction::Attack {
                target_entity_id: 5,
            },
            ClientAction::Chat {
                message: "test".into(),
            },
            ClientAction::Pickup { drop_id: 10 },
            ClientAction::Respawn,
        ];
        assert_eq!(actions.len(), 5);
    }
}
