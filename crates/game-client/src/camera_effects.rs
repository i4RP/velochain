//! Enhanced camera system with screen shake, smooth zoom transitions,
//! camera bounds, focus targets, and cinematic mode.
//!
//! Extends the base camera module with game-feel enhancements.

use crate::camera::{CameraConfig, LocalPlayer, MainCamera};
use bevy::prelude::*;

/// Plugin for camera effects.
pub struct CameraEffectsPlugin;

impl Plugin for CameraEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScreenShake::default())
            .insert_resource(ZoomTransition::default())
            .insert_resource(CameraBounds::default())
            .insert_resource(CameraMode::default())
            .add_event::<CameraEvent>()
            .add_systems(
                Update,
                (
                    handle_camera_events,
                    apply_screen_shake,
                    apply_zoom_transition,
                    apply_camera_bounds,
                    handle_cinematic_mode,
                )
                    .chain(),
            );
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Camera effect events.
#[derive(Event, Clone, Debug)]
pub enum CameraEvent {
    /// Trigger screen shake.
    Shake { intensity: f32, duration: f32 },
    /// Smoothly zoom to a target level.
    ZoomTo { target: f32, speed: f32 },
    /// Focus camera on a world position.
    FocusOn { position: Vec2, duration: f32 },
    /// Return camera to follow player.
    ReturnToPlayer,
    /// Toggle cinematic mode (free camera).
    ToggleCinematic,
    /// Set camera bounds.
    SetBounds { min: Vec2, max: Vec2 },
    /// Clear camera bounds.
    ClearBounds,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Screen shake state.
#[derive(Resource)]
pub struct ScreenShake {
    /// Current shake intensity.
    pub intensity: f32,
    /// Remaining shake duration.
    pub remaining: f32,
    /// Shake decay rate.
    pub decay: f32,
    /// Current offset applied to camera.
    pub offset: Vec2,
    /// Pseudo-random seed for shake pattern.
    pub seed: f32,
}

impl Default for ScreenShake {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            remaining: 0.0,
            decay: 5.0,
            offset: Vec2::ZERO,
            seed: 0.0,
        }
    }
}

impl ScreenShake {
    /// Trigger a new shake. Stacks with existing shake.
    pub fn trigger(&mut self, intensity: f32, duration: f32) {
        self.intensity = (self.intensity + intensity).min(20.0);
        self.remaining = self.remaining.max(duration);
    }

    /// Whether the camera is currently shaking.
    pub fn is_active(&self) -> bool {
        self.remaining > 0.0
    }
}

/// Smooth zoom transition state.
#[derive(Resource)]
pub struct ZoomTransition {
    /// Target zoom level.
    pub target: f32,
    /// Interpolation speed.
    pub speed: f32,
    /// Whether a transition is active.
    pub active: bool,
}

impl Default for ZoomTransition {
    fn default() -> Self {
        Self {
            target: 1.0,
            speed: 2.0,
            active: false,
        }
    }
}

/// Camera movement bounds.
#[derive(Resource, Default)]
pub struct CameraBounds {
    /// Minimum position (bottom-left).
    pub min: Option<Vec2>,
    /// Maximum position (top-right).
    pub max: Option<Vec2>,
}

impl CameraBounds {
    /// Whether bounds are active.
    pub fn is_active(&self) -> bool {
        self.min.is_some() && self.max.is_some()
    }

    /// Clamp a position to within bounds.
    pub fn clamp(&self, pos: Vec2) -> Vec2 {
        match (self.min, self.max) {
            (Some(min), Some(max)) => {
                Vec2::new(pos.x.clamp(min.x, max.x), pos.y.clamp(min.y, max.y))
            }
            _ => pos,
        }
    }
}

/// Camera mode state.
#[derive(Resource)]
pub struct CameraMode {
    /// Current mode.
    pub mode: CameraModeType,
    /// Focus target position (for FocusOn mode).
    pub focus_target: Option<Vec2>,
    /// Time remaining for focus.
    pub focus_remaining: f32,
    /// Cinematic camera speed.
    pub cinematic_speed: f32,
}

impl Default for CameraMode {
    fn default() -> Self {
        Self {
            mode: CameraModeType::FollowPlayer,
            focus_target: None,
            focus_remaining: 0.0,
            cinematic_speed: 200.0,
        }
    }
}

/// Camera behavior modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraModeType {
    /// Follow the local player (default).
    FollowPlayer,
    /// Focus on a specific position.
    FocusTarget,
    /// Free camera controlled by keyboard.
    Cinematic,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn handle_camera_events(
    mut shake: ResMut<ScreenShake>,
    mut zoom: ResMut<ZoomTransition>,
    mut bounds: ResMut<CameraBounds>,
    mut mode: ResMut<CameraMode>,
    mut events: EventReader<CameraEvent>,
) {
    for event in events.read() {
        match event {
            CameraEvent::Shake {
                intensity,
                duration,
            } => {
                shake.trigger(*intensity, *duration);
            }
            CameraEvent::ZoomTo { target, speed } => {
                zoom.target = *target;
                zoom.speed = *speed;
                zoom.active = true;
            }
            CameraEvent::FocusOn { position, duration } => {
                mode.mode = CameraModeType::FocusTarget;
                mode.focus_target = Some(*position);
                mode.focus_remaining = *duration;
            }
            CameraEvent::ReturnToPlayer => {
                mode.mode = CameraModeType::FollowPlayer;
                mode.focus_target = None;
                mode.focus_remaining = 0.0;
            }
            CameraEvent::ToggleCinematic => {
                mode.mode = if mode.mode == CameraModeType::Cinematic {
                    CameraModeType::FollowPlayer
                } else {
                    CameraModeType::Cinematic
                };
            }
            CameraEvent::SetBounds { min, max } => {
                bounds.min = Some(*min);
                bounds.max = Some(*max);
            }
            CameraEvent::ClearBounds => {
                bounds.min = None;
                bounds.max = None;
            }
        }
    }
}

fn apply_screen_shake(
    time: Res<Time>,
    mut shake: ResMut<ScreenShake>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    if !shake.is_active() {
        if shake.offset != Vec2::ZERO {
            // Remove leftover shake offset
            if let Ok(mut transform) = camera_query.get_single_mut() {
                transform.translation.x -= shake.offset.x;
                transform.translation.y -= shake.offset.y;
                shake.offset = Vec2::ZERO;
            }
        }
        return;
    }

    let dt = time.delta_secs();
    shake.remaining -= dt;
    shake.seed += dt * 50.0;

    // Decay intensity
    let decay_factor = (-shake.decay * dt).exp();
    shake.intensity *= decay_factor;

    // Generate shake offset using simple pseudo-random from seed
    let new_offset = Vec2::new(
        (shake.seed * 12.9898).sin() * shake.intensity,
        (shake.seed * 78.233).sin() * shake.intensity,
    );

    if let Ok(mut transform) = camera_query.get_single_mut() {
        // Remove old offset, apply new
        transform.translation.x += new_offset.x - shake.offset.x;
        transform.translation.y += new_offset.y - shake.offset.y;
    }

    shake.offset = new_offset;

    if shake.remaining <= 0.0 {
        shake.remaining = 0.0;
        shake.intensity = 0.0;
    }
}

fn apply_zoom_transition(
    time: Res<Time>,
    mut zoom: ResMut<ZoomTransition>,
    mut config: ResMut<CameraConfig>,
) {
    if !zoom.active {
        return;
    }

    let dt = time.delta_secs();
    let diff = zoom.target - config.zoom;

    if diff.abs() < 0.01 {
        config.zoom = zoom.target;
        zoom.active = false;
        return;
    }

    config.zoom += diff * zoom.speed * dt;
    config.zoom = config.zoom.clamp(config.min_zoom, config.max_zoom);
}

fn apply_camera_bounds(
    bounds: Res<CameraBounds>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    if !bounds.is_active() {
        return;
    }

    if let Ok(mut transform) = camera_query.get_single_mut() {
        let clamped = bounds.clamp(transform.translation.truncate());
        transform.translation.x = clamped.x;
        transform.translation.y = clamped.y;
    }
}

fn handle_cinematic_mode(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<CameraMode>,
    _player_query: Query<&Transform, (With<LocalPlayer>, Without<MainCamera>)>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs();

    match mode.mode {
        CameraModeType::Cinematic => {
            let Ok(mut cam) = camera_query.get_single_mut() else {
                return;
            };
            let speed = mode.cinematic_speed * dt;

            if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
                cam.translation.y += speed;
            }
            if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
                cam.translation.y -= speed;
            }
            if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
                cam.translation.x -= speed;
            }
            if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
                cam.translation.x += speed;
            }
        }
        CameraModeType::FocusTarget => {
            if let Some(target) = mode.focus_target {
                if let Ok(mut cam) = camera_query.get_single_mut() {
                    let current = cam.translation.truncate();
                    let new_pos = current.lerp(target, 3.0 * dt);
                    cam.translation.x = new_pos.x;
                    cam.translation.y = new_pos.y;
                }
            }

            mode.focus_remaining -= dt;
            if mode.focus_remaining <= 0.0 {
                mode.mode = CameraModeType::FollowPlayer;
                mode.focus_target = None;
            }
        }
        CameraModeType::FollowPlayer => {
            // Handled by base camera module
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_shake_trigger() {
        let mut shake = ScreenShake::default();
        assert!(!shake.is_active());

        shake.trigger(5.0, 0.5);
        assert!(shake.is_active());
        assert_eq!(shake.intensity, 5.0);
        assert_eq!(shake.remaining, 0.5);
    }

    #[test]
    fn test_screen_shake_stacking() {
        let mut shake = ScreenShake::default();
        shake.trigger(3.0, 0.3);
        shake.trigger(4.0, 0.5);
        assert_eq!(shake.intensity, 7.0);
        assert_eq!(shake.remaining, 0.5);
    }

    #[test]
    fn test_screen_shake_max_cap() {
        let mut shake = ScreenShake::default();
        shake.trigger(15.0, 1.0);
        shake.trigger(10.0, 1.0);
        assert_eq!(shake.intensity, 20.0); // Capped at 20
    }

    #[test]
    fn test_zoom_transition_default() {
        let zoom = ZoomTransition::default();
        assert_eq!(zoom.target, 1.0);
        assert!(!zoom.active);
    }

    #[test]
    fn test_camera_bounds_inactive() {
        let bounds = CameraBounds::default();
        assert!(!bounds.is_active());

        let pos = Vec2::new(100.0, 200.0);
        assert_eq!(bounds.clamp(pos), pos);
    }

    #[test]
    fn test_camera_bounds_clamping() {
        let bounds = CameraBounds {
            min: Some(Vec2::new(-50.0, -50.0)),
            max: Some(Vec2::new(50.0, 50.0)),
        };
        assert!(bounds.is_active());

        assert_eq!(bounds.clamp(Vec2::new(0.0, 0.0)), Vec2::new(0.0, 0.0));
        assert_eq!(bounds.clamp(Vec2::new(100.0, 100.0)), Vec2::new(50.0, 50.0));
        assert_eq!(
            bounds.clamp(Vec2::new(-100.0, -100.0)),
            Vec2::new(-50.0, -50.0)
        );
    }

    #[test]
    fn test_camera_mode_default() {
        let mode = CameraMode::default();
        assert_eq!(mode.mode, CameraModeType::FollowPlayer);
        assert!(mode.focus_target.is_none());
    }

    #[test]
    fn test_camera_mode_types() {
        assert_ne!(CameraModeType::FollowPlayer, CameraModeType::Cinematic);
        assert_ne!(CameraModeType::FocusTarget, CameraModeType::FollowPlayer);
    }

    #[test]
    fn test_camera_events_all_variants() {
        let events = vec![
            CameraEvent::Shake {
                intensity: 5.0,
                duration: 0.5,
            },
            CameraEvent::ZoomTo {
                target: 2.0,
                speed: 3.0,
            },
            CameraEvent::FocusOn {
                position: Vec2::new(10.0, 20.0),
                duration: 2.0,
            },
            CameraEvent::ReturnToPlayer,
            CameraEvent::ToggleCinematic,
            CameraEvent::SetBounds {
                min: Vec2::ZERO,
                max: Vec2::new(100.0, 100.0),
            },
            CameraEvent::ClearBounds,
        ];
        assert_eq!(events.len(), 7);
    }
}
