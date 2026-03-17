//! Touch and mobile input support.
//!
//! Provides virtual joystick, touch-tap interaction, pinch zoom,
//! swipe gestures, and adaptive UI scaling for mobile/tablet play.
//! Works alongside keyboard input — both can be active simultaneously.

use bevy::prelude::*;

/// Plugin for touch/mobile input.
pub struct TouchInputPlugin;

impl Plugin for TouchInputPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TouchConfig::default())
            .insert_resource(VirtualJoystick::default())
            .insert_resource(GestureState::default())
            .insert_resource(TouchUiScale::default())
            .add_event::<TouchGameAction>()
            .add_systems(Update, (
                process_touch_input,
                update_virtual_joystick,
                detect_gestures,
            ));
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Touch input configuration.
#[derive(Resource)]
pub struct TouchConfig {
    /// Whether touch input is enabled.
    pub enabled: bool,
    /// Dead zone radius for virtual joystick (pixels).
    pub joystick_dead_zone: f32,
    /// Maximum joystick displacement (pixels).
    pub joystick_max_radius: f32,
    /// Tap detection threshold (seconds).
    pub tap_threshold: f32,
    /// Swipe minimum distance (pixels).
    pub swipe_min_distance: f32,
    /// Pinch zoom sensitivity.
    pub pinch_sensitivity: f32,
    /// Double-tap detection window (seconds).
    pub double_tap_window: f32,
    /// Long-press threshold (seconds).
    pub long_press_threshold: f32,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            joystick_dead_zone: 10.0,
            joystick_max_radius: 60.0,
            tap_threshold: 0.25,
            swipe_min_distance: 50.0,
            pinch_sensitivity: 1.0,
            double_tap_window: 0.3,
            long_press_threshold: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Virtual joystick
// ---------------------------------------------------------------------------

/// Virtual joystick state for movement.
#[derive(Resource)]
pub struct VirtualJoystick {
    /// Whether the joystick is currently active (finger down).
    pub active: bool,
    /// Center position where touch started.
    pub origin: Vec2,
    /// Current touch position.
    pub current: Vec2,
    /// Normalized direction (-1 to 1 on each axis).
    pub direction: Vec2,
    /// Displacement magnitude (0 to 1).
    pub magnitude: f32,
    /// Which side of screen the joystick is on.
    pub side: JoystickSide,
}

impl Default for VirtualJoystick {
    fn default() -> Self {
        Self {
            active: false,
            origin: Vec2::ZERO,
            current: Vec2::ZERO,
            direction: Vec2::ZERO,
            magnitude: 0.0,
            side: JoystickSide::Left,
        }
    }
}

impl VirtualJoystick {
    /// Reset joystick to idle.
    pub fn reset(&mut self) {
        self.active = false;
        self.direction = Vec2::ZERO;
        self.magnitude = 0.0;
    }

    /// Update joystick from touch positions.
    pub fn update(&mut self, origin: Vec2, current: Vec2, dead_zone: f32, max_radius: f32) {
        self.origin = origin;
        self.current = current;

        let delta = current - origin;
        let distance = delta.length();

        if distance < dead_zone {
            self.direction = Vec2::ZERO;
            self.magnitude = 0.0;
        } else {
            self.direction = delta.normalize();
            self.magnitude = ((distance - dead_zone) / (max_radius - dead_zone)).min(1.0);
        }
    }

    /// Get the movement vector (direction * magnitude).
    pub fn movement(&self) -> Vec2 {
        self.direction * self.magnitude
    }
}

/// Which side of the screen the joystick appears on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoystickSide {
    Left,
    Right,
}

// ---------------------------------------------------------------------------
// Gesture detection
// ---------------------------------------------------------------------------

/// Gesture recognition state.
#[derive(Resource)]
pub struct GestureState {
    /// Active touch points.
    pub touches: Vec<TouchPoint>,
    /// Last tap time for double-tap detection.
    pub last_tap_time: f32,
    /// Last tap position for double-tap detection.
    pub last_tap_pos: Vec2,
    /// Previous pinch distance (for delta calculation).
    pub prev_pinch_distance: Option<f32>,
    /// Current recognized gesture.
    pub current_gesture: Option<Gesture>,
}

impl Default for GestureState {
    fn default() -> Self {
        Self {
            touches: Vec::new(),
            last_tap_time: -1.0,
            last_tap_pos: Vec2::ZERO,
            prev_pinch_distance: None,
            current_gesture: None,
        }
    }
}

/// A tracked touch point.
#[derive(Clone, Debug)]
pub struct TouchPoint {
    /// Touch ID.
    pub id: u64,
    /// Start position.
    pub start_pos: Vec2,
    /// Current position.
    pub current_pos: Vec2,
    /// Time touch started.
    pub start_time: f32,
    /// Whether this touch has been consumed by a gesture.
    pub consumed: bool,
}

/// Recognized gesture types.
#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    /// Single tap at position.
    Tap(Vec2),
    /// Double tap at position.
    DoubleTap(Vec2),
    /// Long press at position.
    LongPress(Vec2),
    /// Swipe in a direction.
    Swipe {
        start: Vec2,
        end: Vec2,
        direction: SwipeDirection,
    },
    /// Pinch zoom (delta factor: >1 zoom in, <1 zoom out).
    Pinch {
        center: Vec2,
        delta: f32,
    },
}

/// Swipe direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

impl SwipeDirection {
    /// Determine swipe direction from a displacement vector.
    pub fn from_delta(delta: Vec2) -> Self {
        if delta.x.abs() > delta.y.abs() {
            if delta.x > 0.0 { Self::Right } else { Self::Left }
        } else if delta.y > 0.0 {
            Self::Up
        } else {
            Self::Down
        }
    }
}

// ---------------------------------------------------------------------------
// Touch game actions
// ---------------------------------------------------------------------------

/// Game actions triggered by touch input.
#[derive(Event, Clone, Debug)]
pub enum TouchGameAction {
    /// Move in a direction (from joystick).
    Move { direction: Vec2, magnitude: f32 },
    /// Tap to interact/attack at screen position.
    Interact { screen_pos: Vec2 },
    /// Double tap to pick up item.
    QuickPickup { screen_pos: Vec2 },
    /// Long press for context menu.
    ContextMenu { screen_pos: Vec2 },
    /// Swipe to open a panel.
    SwipeMenu { direction: SwipeDirection },
    /// Pinch zoom change.
    Zoom { delta: f32 },
}

// ---------------------------------------------------------------------------
// UI scaling
// ---------------------------------------------------------------------------

/// Adaptive UI scaling for different screen sizes.
#[derive(Resource)]
pub struct TouchUiScale {
    /// Current scale factor (1.0 = normal, 1.5 = 150%).
    pub scale: f32,
    /// Whether to auto-detect from screen size.
    pub auto_scale: bool,
    /// Minimum button/interactive element size (pixels).
    pub min_touch_target: f32,
    /// Reference resolution width for scaling calculation.
    pub reference_width: f32,
}

impl Default for TouchUiScale {
    fn default() -> Self {
        Self {
            scale: 1.0,
            auto_scale: true,
            min_touch_target: 44.0,
            reference_width: 1280.0,
        }
    }
}

impl TouchUiScale {
    /// Calculate scale factor based on screen width.
    pub fn calculate_scale(&self, screen_width: f32) -> f32 {
        if !self.auto_scale {
            return self.scale;
        }
        // On small screens, scale up UI; on large screens, keep normal
        let ratio = self.reference_width / screen_width;
        ratio.clamp(0.8, 2.0)
    }

    /// Get the effective minimum touch target size.
    pub fn effective_touch_target(&self) -> f32 {
        self.min_touch_target * self.scale
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn process_touch_input(
    config: Res<TouchConfig>,
    touches: Res<Touches>,
    time: Res<Time>,
    mut gesture: ResMut<GestureState>,
) {
    if !config.enabled {
        return;
    }

    let now = time.elapsed_secs();

    // Track new touches
    for touch in touches.iter_just_pressed() {
        gesture.touches.push(TouchPoint {
            id: touch.id(),
            start_pos: touch.position(),
            current_pos: touch.position(),
            start_time: now,
            consumed: false,
        });
    }

    // Update existing touches
    for touch in touches.iter() {
        if let Some(tp) = gesture.touches.iter_mut().find(|t| t.id == touch.id()) {
            tp.current_pos = touch.position();
        }
    }

    // Handle released touches
    for touch in touches.iter_just_released() {
        // Remove the released touch
        gesture.touches.retain(|t| t.id != touch.id());
    }
}

fn update_virtual_joystick(
    config: Res<TouchConfig>,
    touches: Res<Touches>,
    mut joystick: ResMut<VirtualJoystick>,
    mut actions: EventWriter<TouchGameAction>,
    windows: Query<&Window>,
) {
    if !config.enabled {
        return;
    }

    let screen_width = windows
        .get_single()
        .map(|w| w.width())
        .unwrap_or(1280.0);

    let half_width = screen_width / 2.0;

    // Find left-side touch for joystick
    let mut found_joystick_touch = false;
    for touch in touches.iter() {
        let pos = touch.position();
        let is_left = pos.x < half_width;

        if is_left && joystick.side == JoystickSide::Left {
            if !joystick.active {
                joystick.active = true;
                joystick.origin = pos;
            }
            let origin = joystick.origin;
            joystick.update(
                origin,
                pos,
                config.joystick_dead_zone,
                config.joystick_max_radius,
            );
            found_joystick_touch = true;

            if joystick.magnitude > 0.0 {
                actions.send(TouchGameAction::Move {
                    direction: joystick.direction,
                    magnitude: joystick.magnitude,
                });
            }
            break;
        }
    }

    if !found_joystick_touch && joystick.active {
        joystick.reset();
    }
}

fn detect_gestures(
    config: Res<TouchConfig>,
    time: Res<Time>,
    touches: Res<Touches>,
    mut gesture: ResMut<GestureState>,
    mut actions: EventWriter<TouchGameAction>,
) {
    if !config.enabled {
        return;
    }

    let now = time.elapsed_secs();

    // Detect tap on release
    for touch in touches.iter_just_released() {
        let pos = touch.position();

        // Find matching touch point
        let was_quick = gesture.touches.iter().any(|tp| {
            tp.id == touch.id()
                && (now - tp.start_time) < config.tap_threshold
                && (tp.current_pos - tp.start_pos).length() < config.swipe_min_distance
        });

        if was_quick {
            // Check for double tap
            let is_double = (now - gesture.last_tap_time) < config.double_tap_window
                && (pos - gesture.last_tap_pos).length() < 30.0;

            if is_double {
                gesture.current_gesture = Some(Gesture::DoubleTap(pos));
                actions.send(TouchGameAction::QuickPickup { screen_pos: pos });
                gesture.last_tap_time = -1.0;
            } else {
                gesture.current_gesture = Some(Gesture::Tap(pos));
                actions.send(TouchGameAction::Interact { screen_pos: pos });
                gesture.last_tap_time = now;
                gesture.last_tap_pos = pos;
            }
        }

        // Detect swipe on release
        if let Some(tp) = gesture.touches.iter().find(|t| t.id == touch.id()) {
            let delta = tp.current_pos - tp.start_pos;
            if delta.length() >= config.swipe_min_distance
                && (now - tp.start_time) < 0.5
            {
                let direction = SwipeDirection::from_delta(delta);
                gesture.current_gesture = Some(Gesture::Swipe {
                    start: tp.start_pos,
                    end: tp.current_pos,
                    direction,
                });
                actions.send(TouchGameAction::SwipeMenu { direction });
            }
        }
    }

    // Detect long press
    let dead_zone = config.joystick_dead_zone;
    let long_threshold = config.long_press_threshold;
    let mut long_press_pos: Option<Vec2> = None;
    for tp in gesture.touches.iter_mut() {
        if !tp.consumed
            && (now - tp.start_time) >= long_threshold
            && (tp.current_pos - tp.start_pos).length() < dead_zone
        {
            tp.consumed = true;
            long_press_pos = Some(tp.current_pos);
        }
    }
    if let Some(pos) = long_press_pos {
        gesture.current_gesture = Some(Gesture::LongPress(pos));
        actions.send(TouchGameAction::ContextMenu { screen_pos: pos });
    }

    // Detect pinch zoom (two active touches)
    let active_touches: Vec<&TouchPoint> = gesture.touches.iter().collect();
    if active_touches.len() == 2 {
        let dist = (active_touches[0].current_pos - active_touches[1].current_pos).length();
        let center = (active_touches[0].current_pos + active_touches[1].current_pos) / 2.0;

        if let Some(prev_dist) = gesture.prev_pinch_distance {
            let delta = (dist / prev_dist) * config.pinch_sensitivity;
            if (delta - 1.0).abs() > 0.01 {
                gesture.current_gesture = Some(Gesture::Pinch { center, delta });
                actions.send(TouchGameAction::Zoom { delta });
            }
        }
        gesture.prev_pinch_distance = Some(dist);
    } else {
        gesture.prev_pinch_distance = None;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_config_defaults() {
        let config = TouchConfig::default();
        assert!(config.enabled);
        assert!(config.joystick_dead_zone > 0.0);
        assert!(config.joystick_max_radius > config.joystick_dead_zone);
        assert!(config.tap_threshold > 0.0);
    }

    #[test]
    fn test_virtual_joystick_default() {
        let js = VirtualJoystick::default();
        assert!(!js.active);
        assert_eq!(js.direction, Vec2::ZERO);
        assert_eq!(js.magnitude, 0.0);
        assert_eq!(js.movement(), Vec2::ZERO);
    }

    #[test]
    fn test_virtual_joystick_update() {
        let mut js = VirtualJoystick::default();
        js.active = true;
        js.update(
            Vec2::new(100.0, 100.0),
            Vec2::new(100.0, 160.0),
            10.0,
            60.0,
        );
        assert!(js.magnitude > 0.0);
        assert!(js.direction.y > 0.0);
    }

    #[test]
    fn test_virtual_joystick_dead_zone() {
        let mut js = VirtualJoystick::default();
        js.update(
            Vec2::new(100.0, 100.0),
            Vec2::new(105.0, 100.0),
            10.0,
            60.0,
        );
        assert_eq!(js.magnitude, 0.0);
        assert_eq!(js.direction, Vec2::ZERO);
    }

    #[test]
    fn test_virtual_joystick_max_magnitude() {
        let mut js = VirtualJoystick::default();
        js.update(
            Vec2::new(100.0, 100.0),
            Vec2::new(300.0, 100.0),
            10.0,
            60.0,
        );
        assert_eq!(js.magnitude, 1.0);
    }

    #[test]
    fn test_virtual_joystick_reset() {
        let mut js = VirtualJoystick::default();
        js.active = true;
        js.magnitude = 0.8;
        js.direction = Vec2::new(1.0, 0.0);
        js.reset();
        assert!(!js.active);
        assert_eq!(js.magnitude, 0.0);
    }

    #[test]
    fn test_swipe_direction_from_delta() {
        assert_eq!(SwipeDirection::from_delta(Vec2::new(50.0, 0.0)), SwipeDirection::Right);
        assert_eq!(SwipeDirection::from_delta(Vec2::new(-50.0, 0.0)), SwipeDirection::Left);
        assert_eq!(SwipeDirection::from_delta(Vec2::new(0.0, 50.0)), SwipeDirection::Up);
        assert_eq!(SwipeDirection::from_delta(Vec2::new(0.0, -50.0)), SwipeDirection::Down);
    }

    #[test]
    fn test_swipe_direction_diagonal() {
        // Diagonal favors the larger axis
        assert_eq!(SwipeDirection::from_delta(Vec2::new(60.0, 40.0)), SwipeDirection::Right);
        assert_eq!(SwipeDirection::from_delta(Vec2::new(30.0, -50.0)), SwipeDirection::Down);
    }

    #[test]
    fn test_gesture_state_default() {
        let state = GestureState::default();
        assert!(state.touches.is_empty());
        assert!(state.current_gesture.is_none());
        assert!(state.prev_pinch_distance.is_none());
    }

    #[test]
    fn test_touch_ui_scale_default() {
        let scale = TouchUiScale::default();
        assert_eq!(scale.scale, 1.0);
        assert!(scale.auto_scale);
        assert_eq!(scale.min_touch_target, 44.0);
    }

    #[test]
    fn test_touch_ui_scale_calculate() {
        let scale = TouchUiScale::default();
        // At reference resolution
        let s = scale.calculate_scale(1280.0);
        assert!((s - 1.0).abs() < 0.01);

        // Small screen should scale up
        let s_small = scale.calculate_scale(640.0);
        assert!(s_small > 1.0);

        // Large screen stays close to 1.0
        let s_large = scale.calculate_scale(2560.0);
        assert!(s_large < 1.0);
    }

    #[test]
    fn test_touch_ui_scale_manual() {
        let mut scale = TouchUiScale::default();
        scale.auto_scale = false;
        scale.scale = 1.5;
        assert_eq!(scale.calculate_scale(640.0), 1.5);
    }

    #[test]
    fn test_effective_touch_target() {
        let mut scale = TouchUiScale::default();
        scale.scale = 1.5;
        assert_eq!(scale.effective_touch_target(), 66.0);
    }

    #[test]
    fn test_gesture_variants() {
        let gestures = vec![
            Gesture::Tap(Vec2::ZERO),
            Gesture::DoubleTap(Vec2::new(10.0, 10.0)),
            Gesture::LongPress(Vec2::new(20.0, 20.0)),
            Gesture::Swipe {
                start: Vec2::ZERO,
                end: Vec2::new(100.0, 0.0),
                direction: SwipeDirection::Right,
            },
            Gesture::Pinch { center: Vec2::ZERO, delta: 1.5 },
        ];
        assert_eq!(gestures.len(), 5);
    }

    #[test]
    fn test_touch_game_actions() {
        let actions = vec![
            TouchGameAction::Move { direction: Vec2::X, magnitude: 0.5 },
            TouchGameAction::Interact { screen_pos: Vec2::ZERO },
            TouchGameAction::QuickPickup { screen_pos: Vec2::ZERO },
            TouchGameAction::ContextMenu { screen_pos: Vec2::ZERO },
            TouchGameAction::SwipeMenu { direction: SwipeDirection::Up },
            TouchGameAction::Zoom { delta: 1.2 },
        ];
        assert_eq!(actions.len(), 6);
    }
}
