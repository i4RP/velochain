//! VeloChain Game Client - Bevy-based 2D game client with WASM support.
//!
//! Renders the on-chain game world using Bevy's 2D sprite system.
//! Connects to a VeloChain node via WebSocket/RPC for real-time updates.

pub mod camera;
pub mod network;
pub mod renderer;
pub mod states;
pub mod terrain_render;
pub mod ui;

use bevy::prelude::*;
use states::GameClientState;

/// Main plugin that bundles all game client systems.
pub struct VeloChainClientPlugin {
    /// RPC endpoint URL (e.g., "ws://localhost:9545").
    pub rpc_url: String,
}

impl Plugin for VeloChainClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(network::NetworkConfig {
            rpc_url: self.rpc_url.clone(),
        })
        .init_state::<GameClientState>()
        .add_plugins((
            camera::CameraPlugin,
            renderer::RendererPlugin,
            terrain_render::TerrainPlugin,
            ui::UiPlugin,
            network::NetworkPlugin,
        ));
    }
}
