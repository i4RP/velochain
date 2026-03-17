//! UI/HUD system for the game client.
//!
//! Provides health bar, inventory panel, minimap, chat, and
//! debug overlay using Bevy's built-in UI system.

use bevy::prelude::*;
use crate::camera::LocalPlayer;
use crate::renderer::GameEntity;
use crate::network::NetworkState;

/// Plugin for all UI elements.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChatLog::default())
            .insert_resource(InventoryUiState::default())
            .add_systems(Startup, setup_ui)
            .add_systems(Update, (
                update_health_bar,
                update_debug_overlay,
                update_chat_display,
                toggle_inventory,
            ));
    }
}

/// Chat log resource.
#[derive(Resource, Default)]
pub struct ChatLog {
    pub messages: Vec<ChatMessage>,
    pub max_display: usize,
}

impl ChatLog {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_display: 8,
        }
    }

    pub fn push(&mut self, sender: String, text: String) {
        self.messages.push(ChatMessage { sender, text });
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
}

/// A single chat message.
pub struct ChatMessage {
    pub sender: String,
    pub text: String,
}

/// Inventory UI state.
#[derive(Resource, Default)]
pub struct InventoryUiState {
    pub is_open: bool,
}

// --- UI Marker Components ---

#[derive(Component)]
struct HealthBarFill;

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct DebugOverlayText;

#[derive(Component)]
struct ChatDisplayText;

#[derive(Component)]
struct InventoryPanel;

fn setup_ui(mut commands: Commands) {
    // Health bar background
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(16.0),
                width: Val::Px(204.0),
                height: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|parent| {
            // Health bar fill
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    margin: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.8, 0.1, 0.1)),
                HealthBarFill,
            ));
        });

    // Health text
    commands.spawn((
        Text::new("HP: 100/100"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(18.0),
            ..default()
        },
        HealthText,
    ));

    // Debug overlay (top-right)
    commands.spawn((
        Text::new("FPS: --\nPos: 0, 0\nEntities: 0\nChunk: 0, 0"),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        DebugOverlayText,
    ));

    // Chat display (bottom-left)
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            bottom: Val::Px(16.0),
            max_width: Val::Px(400.0),
            ..default()
        },
        ChatDisplayText,
    ));

    // Inventory panel (hidden by default)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(80.0),
                width: Val::Px(250.0),
                height: Val::Px(350.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.85)),
            InventoryPanel,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Inventory"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.8, 0.3)),
            ));

            parent.spawn((
                Text::new("(Empty)"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(0.8, 0.8, 0.8, 0.7)),
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
            ));
        });
}

fn update_health_bar(
    player_query: Query<&GameEntity, With<LocalPlayer>>,
    mut bar_query: Query<&mut Node, With<HealthBarFill>>,
    mut text_query: Query<&mut Text, With<HealthText>>,
) {
    let Ok(entity) = player_query.get_single() else {
        return;
    };

    let pct = if entity.max_health > 0.0 {
        (entity.health / entity.max_health * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    if let Ok(mut node) = bar_query.get_single_mut() {
        node.width = Val::Percent(pct);
    }

    if let Ok(mut text) = text_query.get_single_mut() {
        **text = format!("HP: {}/{}", entity.health as i32, entity.max_health as i32);
    }
}

fn update_debug_overlay(
    time: Res<Time>,
    player_query: Query<&Transform, With<LocalPlayer>>,
    entity_query: Query<&GameEntity>,
    net_state: Res<NetworkState>,
    mut text_query: Query<&mut Text, With<DebugOverlayText>>,
) {
    let Ok(mut text) = text_query.get_single_mut() else {
        return;
    };

    let fps = (1.0 / time.delta_secs()).round() as i32;
    let (px, py) = if let Ok(t) = player_query.get_single() {
        (t.translation.x as i32, t.translation.y as i32)
    } else {
        (0, 0)
    };
    let entity_count = entity_query.iter().count();
    let connected = if net_state.connected { "Yes" } else { "No" };

    **text = format!(
        "FPS: {}\nPos: {}, {}\nEntities: {}\nConnected: {}\nTick: {}",
        fps, px, py, entity_count, connected, net_state.server_tick
    );
}

fn update_chat_display(
    chat_log: Res<ChatLog>,
    mut text_query: Query<&mut Text, With<ChatDisplayText>>,
) {
    if !chat_log.is_changed() {
        return;
    }

    let Ok(mut text) = text_query.get_single_mut() else {
        return;
    };

    let start = chat_log.messages.len().saturating_sub(chat_log.max_display);
    let visible: Vec<String> = chat_log.messages[start..]
        .iter()
        .map(|m| format!("[{}] {}", m.sender, m.text))
        .collect();

    **text = visible.join("\n");
}

fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inv_state: ResMut<InventoryUiState>,
    mut panel_query: Query<&mut Node, With<InventoryPanel>>,
) {
    if keyboard.just_pressed(KeyCode::KeyI) || keyboard.just_pressed(KeyCode::Tab) {
        inv_state.is_open = !inv_state.is_open;
        if let Ok(mut node) = panel_query.get_single_mut() {
            node.display = if inv_state.is_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_log() {
        let mut log = ChatLog::new();
        log.push("Alice".into(), "Hello!".into());
        log.push("Bob".into(), "Hi!".into());
        assert_eq!(log.messages.len(), 2);
    }

    #[test]
    fn test_chat_log_overflow() {
        let mut log = ChatLog::new();
        for i in 0..110 {
            log.push(format!("User{}", i), format!("Msg {}", i));
        }
        assert!(log.messages.len() <= 100);
    }

    #[test]
    fn test_inventory_ui_state_default() {
        let state = InventoryUiState::default();
        assert!(!state.is_open);
    }
}
