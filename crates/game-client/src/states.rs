//! Game client state machine.

use bevy::prelude::*;

/// Top-level game client states.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameClientState {
    /// Loading assets and connecting to server.
    #[default]
    Loading,
    /// Main gameplay state.
    Playing,
    /// Inventory/menu overlay.
    Menu,
}
