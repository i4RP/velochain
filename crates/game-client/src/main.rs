//! VeloChain Game Client - main entry point.
//!
//! Launches the Bevy-based 2D game client that connects to
//! a VeloChain node and renders the on-chain game world.

use bevy::prelude::*;
use velochain_game_client::VeloChainClientPlugin;

fn main() {
    let rpc_url = std::env::var("VELOCHAIN_RPC_URL")
        .unwrap_or_else(|_| "ws://127.0.0.1:9545".to_string());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "VeloChain".to_string(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(VeloChainClientPlugin { rpc_url })
        .run();
}
