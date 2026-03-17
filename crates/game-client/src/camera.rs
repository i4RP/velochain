//! Camera system for the 2D game view.
//!
//! Provides a 2D camera that follows the player and supports
//! zoom and pan controls.

use bevy::prelude::*;

/// Plugin for camera management.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera)
            .add_systems(Update, (camera_follow_player, camera_zoom));
    }
}

/// Marker component for the main game camera.
#[derive(Component)]
pub struct MainCamera;

/// Camera configuration resource.
#[derive(Resource)]
pub struct CameraConfig {
    /// Zoom level (1.0 = default, smaller = zoomed in).
    pub zoom: f32,
    /// Minimum zoom level.
    pub min_zoom: f32,
    /// Maximum zoom level.
    pub max_zoom: f32,
    /// Camera follow smoothing factor (0-1, higher = snappier).
    pub follow_speed: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            min_zoom: 0.25,
            max_zoom: 4.0,
            follow_speed: 0.1,
        }
    }
}

/// Marker component for the local player entity.
#[derive(Component)]
pub struct LocalPlayer;

fn setup_camera(mut commands: Commands) {
    commands.insert_resource(CameraConfig::default());

    commands.spawn((Camera2d, MainCamera, Transform::from_xyz(0.0, 0.0, 999.0)));
}

fn camera_follow_player(
    config: Res<CameraConfig>,
    player_query: Query<&Transform, (With<LocalPlayer>, Without<MainCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut OrthographicProjection), With<MainCamera>>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let Ok((mut cam_transform, mut projection)) = camera_query.get_single_mut() else {
        return;
    };

    // Smooth follow
    let target = player_transform.translation.truncate();
    let current = cam_transform.translation.truncate();
    let new_pos = current.lerp(target, config.follow_speed);
    cam_transform.translation.x = new_pos.x;
    cam_transform.translation.y = new_pos.y;

    // Apply zoom
    projection.scale = config.zoom;
}

fn camera_zoom(mut config: ResMut<CameraConfig>, keyboard: Res<ButtonInput<KeyCode>>) {
    let zoom_speed = 0.05;

    if keyboard.pressed(KeyCode::Equal) || keyboard.pressed(KeyCode::NumpadAdd) {
        config.zoom = (config.zoom - zoom_speed).max(config.min_zoom);
    }
    if keyboard.pressed(KeyCode::Minus) || keyboard.pressed(KeyCode::NumpadSubtract) {
        config.zoom = (config.zoom + zoom_speed).min(config.max_zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_config_defaults() {
        let config = CameraConfig::default();
        assert_eq!(config.zoom, 1.0);
        assert!(config.min_zoom < config.max_zoom);
        assert!(config.follow_speed > 0.0 && config.follow_speed <= 1.0);
    }
}
