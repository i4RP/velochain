//! Particle effects and animation system.
//!
//! Provides configurable particle emitters for combat effects,
//! level-up celebrations, item pickups, and damage numbers.
//! All particles are 2D sprites with lifetime, velocity, and fade.

use bevy::prelude::*;

/// Plugin for particle effects.
pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ParticleConfig::default())
            .add_event::<ParticleEvent>()
            .add_systems(
                Update,
                (
                    handle_particle_events,
                    update_particles,
                    update_damage_numbers,
                    update_sprite_animations,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Global particle configuration.
#[derive(Resource)]
pub struct ParticleConfig {
    /// Maximum simultaneous particles.
    pub max_particles: usize,
    /// Global particle scale multiplier.
    pub scale: f32,
    /// Whether particles are enabled.
    pub enabled: bool,
}

impl Default for ParticleConfig {
    fn default() -> Self {
        Self {
            max_particles: 500,
            scale: 1.0,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Event to request particle emission.
#[derive(Event, Clone, Debug)]
pub enum ParticleEvent {
    /// Damage number floating up from a position.
    DamageNumber {
        position: Vec3,
        amount: i32,
        is_critical: bool,
    },
    /// Heal number floating up.
    HealNumber { position: Vec3, amount: i32 },
    /// Attack spark burst at contact point.
    AttackSpark { position: Vec3 },
    /// Level-up celebration around an entity.
    LevelUp { position: Vec3 },
    /// Item pickup sparkle.
    ItemPickup { position: Vec3 },
    /// Death poof effect.
    DeathPoof { position: Vec3 },
    /// Spawn/respawn glow.
    SpawnGlow { position: Vec3 },
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// A single particle with physics and lifetime.
#[derive(Component)]
pub struct Particle {
    /// Velocity in pixels/sec.
    pub velocity: Vec2,
    /// Acceleration (e.g. gravity).
    pub acceleration: Vec2,
    /// Total lifetime in seconds.
    pub lifetime: f32,
    /// Elapsed time.
    pub elapsed: f32,
    /// Starting alpha.
    pub start_alpha: f32,
    /// Whether to fade out over lifetime.
    pub fade_out: bool,
    /// Scale change per second.
    pub scale_velocity: f32,
}

/// Floating damage/heal number.
#[derive(Component)]
pub struct DamageNumber {
    /// Float speed (pixels/sec upward).
    pub float_speed: f32,
    /// Total display time.
    pub lifetime: f32,
    /// Elapsed time.
    pub elapsed: f32,
}

/// Sprite animation controller for entity state transitions.
#[derive(Component)]
pub struct SpriteAnimation {
    /// Current animation state.
    pub state: AnimationState,
    /// Time spent in current state.
    pub timer: f32,
    /// Animation speed multiplier.
    pub speed: f32,
    /// Base color to return to after flash.
    pub base_color: Color,
}

/// Animation states for game entities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationState {
    #[default]
    Idle,
    Walking,
    Attacking,
    Hurt,
    Dying,
}

// ---------------------------------------------------------------------------
// Particle presets
// ---------------------------------------------------------------------------

/// Preset configurations for common particle types.
pub struct ParticlePreset;

impl ParticlePreset {
    /// Small spark particle.
    pub fn spark(base_angle: f32, spread: f32, speed: f32) -> Particle {
        let angle = base_angle + spread;
        Particle {
            velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            acceleration: Vec2::new(0.0, -50.0),
            lifetime: 0.4,
            elapsed: 0.0,
            start_alpha: 1.0,
            fade_out: true,
            scale_velocity: -1.0,
        }
    }

    /// Upward floating particle (for level-up, glow effects).
    pub fn float_up(offset_x: f32, speed: f32) -> Particle {
        Particle {
            velocity: Vec2::new(offset_x, speed),
            acceleration: Vec2::ZERO,
            lifetime: 1.0,
            elapsed: 0.0,
            start_alpha: 0.9,
            fade_out: true,
            scale_velocity: -0.3,
        }
    }

    /// Expanding ring particle.
    pub fn ring_expand(angle: f32, speed: f32) -> Particle {
        Particle {
            velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            acceleration: Vec2::ZERO,
            lifetime: 0.6,
            elapsed: 0.0,
            start_alpha: 0.8,
            fade_out: true,
            scale_velocity: 0.5,
        }
    }

    /// Poof particle (death effect).
    pub fn poof(angle: f32, speed: f32) -> Particle {
        Particle {
            velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            acceleration: Vec2::new(0.0, 20.0),
            lifetime: 0.5,
            elapsed: 0.0,
            start_alpha: 0.7,
            fade_out: true,
            scale_velocity: 0.8,
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn handle_particle_events(
    mut commands: Commands,
    config: Res<ParticleConfig>,
    mut events: EventReader<ParticleEvent>,
    existing_particles: Query<&Particle>,
) {
    if !config.enabled {
        events.clear();
        return;
    }

    let current_count = existing_particles.iter().count();
    let mut spawned = 0;

    for event in events.read() {
        if current_count + spawned >= config.max_particles {
            break;
        }

        match event {
            ParticleEvent::DamageNumber {
                position,
                amount,
                is_critical,
            } => {
                let color = if *is_critical {
                    Color::srgb(1.0, 0.3, 0.0)
                } else {
                    Color::srgb(1.0, 0.2, 0.2)
                };
                let font_size = if *is_critical { 22.0 } else { 16.0 };

                commands.spawn((
                    Text::new(format!("-{}", amount)),
                    TextFont {
                        font_size,
                        ..default()
                    },
                    TextColor(color),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(position.x),
                        top: Val::Px(position.y),
                        ..default()
                    },
                    DamageNumber {
                        float_speed: 40.0,
                        lifetime: 1.2,
                        elapsed: 0.0,
                    },
                ));
                spawned += 1;
            }
            ParticleEvent::HealNumber { position, amount } => {
                commands.spawn((
                    Text::new(format!("+{}", amount)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.2, 1.0, 0.3)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(position.x),
                        top: Val::Px(position.y),
                        ..default()
                    },
                    DamageNumber {
                        float_speed: 30.0,
                        lifetime: 1.0,
                        elapsed: 0.0,
                    },
                ));
                spawned += 1;
            }
            ParticleEvent::AttackSpark { position } => {
                for i in 0..6 {
                    let angle = (i as f32 / 6.0) * std::f32::consts::TAU;
                    let p = ParticlePreset::spark(angle, 0.2, 80.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgb(1.0, 0.8, 0.2),
                        3.0 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
            }
            ParticleEvent::LevelUp { position } => {
                for i in 0..12 {
                    let angle = (i as f32 / 12.0) * std::f32::consts::TAU;
                    let p = ParticlePreset::ring_expand(angle, 60.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgb(1.0, 0.9, 0.3),
                        4.0 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
                // Upward sparkles
                for i in 0..8 {
                    let ox = (i as f32 - 4.0) * 5.0;
                    let p = ParticlePreset::float_up(ox, 50.0 + (i as f32) * 5.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgb(0.9, 0.8, 0.1),
                        2.5 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
            }
            ParticleEvent::ItemPickup { position } => {
                for i in 0..4 {
                    let ox = (i as f32 - 2.0) * 8.0;
                    let p = ParticlePreset::float_up(ox, 40.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgb(0.3, 1.0, 0.5),
                        2.0 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
            }
            ParticleEvent::DeathPoof { position } => {
                for i in 0..8 {
                    let angle = (i as f32 / 8.0) * std::f32::consts::TAU;
                    let p = ParticlePreset::poof(angle, 30.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgba(0.5, 0.5, 0.5, 0.7),
                        5.0 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
            }
            ParticleEvent::SpawnGlow { position } => {
                for i in 0..10 {
                    let angle = (i as f32 / 10.0) * std::f32::consts::TAU;
                    let p = ParticlePreset::ring_expand(angle, 40.0);
                    spawn_particle_sprite(
                        &mut commands,
                        *position,
                        Color::srgba(0.4, 0.7, 1.0, 0.8),
                        3.0 * config.scale,
                        p,
                    );
                    spawned += 1;
                }
            }
        }
    }
}

fn spawn_particle_sprite(
    commands: &mut Commands,
    position: Vec3,
    color: Color,
    size: f32,
    particle: Particle,
) {
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(size, size)),
            ..default()
        },
        Transform::from_translation(position + Vec3::Z * 10.0),
        particle,
    ));
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    for (entity, mut p, mut transform, mut sprite) in query.iter_mut() {
        p.elapsed += dt;

        if p.elapsed >= p.lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Update position
        let accel = p.acceleration * dt;
        p.velocity += accel;
        transform.translation.x += p.velocity.x * dt;
        transform.translation.y += p.velocity.y * dt;

        // Update scale
        let current_scale = transform.scale.x + p.scale_velocity * dt;
        let clamped = current_scale.max(0.01);
        transform.scale = Vec3::splat(clamped);

        // Fade out
        if p.fade_out {
            let progress = p.elapsed / p.lifetime;
            let alpha = p.start_alpha * (1.0 - progress);
            sprite.color = sprite.color.with_alpha(alpha.max(0.0));
        }
    }
}

fn update_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DamageNumber, &mut Node, &mut TextColor)>,
) {
    let dt = time.delta_secs();

    for (entity, mut dmg, mut node, mut color) in query.iter_mut() {
        dmg.elapsed += dt;

        if dmg.elapsed >= dmg.lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Float upward
        if let Val::Px(ref mut top) = node.top {
            *top -= dmg.float_speed * dt;
        }

        // Fade out in last 40%
        let progress = dmg.elapsed / dmg.lifetime;
        if progress > 0.6 {
            let fade = 1.0 - ((progress - 0.6) / 0.4);
            color.0 = color.0.with_alpha(fade.max(0.0));
        }
    }
}

fn update_sprite_animations(
    time: Res<Time>,
    mut query: Query<(&mut SpriteAnimation, &mut Sprite)>,
) {
    let dt = time.delta_secs();

    for (mut anim, mut sprite) in query.iter_mut() {
        anim.timer += dt * anim.speed;

        match anim.state {
            AnimationState::Idle => {
                // Gentle breathing pulse
                let pulse = (anim.timer * 2.0).sin() * 0.02 + 1.0;
                sprite.custom_size = sprite.custom_size.map(|s| s * pulse);
            }
            AnimationState::Walking => {
                // Slight bobbing
                let _bob = (anim.timer * 8.0).sin() * 0.03;
                // Applied via transform in a real implementation
            }
            AnimationState::Attacking => {
                // Flash white briefly
                if anim.timer < 0.15 {
                    sprite.color = Color::srgb(1.0, 1.0, 1.0);
                } else {
                    sprite.color = anim.base_color;
                    anim.state = AnimationState::Idle;
                    anim.timer = 0.0;
                }
            }
            AnimationState::Hurt => {
                // Flash red
                if anim.timer < 0.2 {
                    let flash = ((anim.timer * 30.0).sin() + 1.0) / 2.0;
                    sprite.color = Color::srgb(1.0, flash, flash);
                } else {
                    sprite.color = anim.base_color;
                    anim.state = AnimationState::Idle;
                    anim.timer = 0.0;
                }
            }
            AnimationState::Dying => {
                // Fade and shrink
                let progress = (anim.timer / 0.8).min(1.0);
                sprite.color = sprite.color.with_alpha(1.0 - progress);
                if let Some(ref mut size) = sprite.custom_size {
                    *size *= 1.0 - progress * 0.3;
                }
            }
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
    fn test_particle_config_defaults() {
        let config = ParticleConfig::default();
        assert_eq!(config.max_particles, 500);
        assert_eq!(config.scale, 1.0);
        assert!(config.enabled);
    }

    #[test]
    fn test_particle_preset_spark() {
        let p = ParticlePreset::spark(0.0, 0.1, 100.0);
        assert!(p.velocity.length() > 0.0);
        assert_eq!(p.lifetime, 0.4);
        assert!(p.fade_out);
    }

    #[test]
    fn test_particle_preset_float_up() {
        let p = ParticlePreset::float_up(5.0, 50.0);
        assert!(p.velocity.y > 0.0);
        assert_eq!(p.lifetime, 1.0);
    }

    #[test]
    fn test_particle_preset_ring_expand() {
        let p = ParticlePreset::ring_expand(0.0, 60.0);
        assert!(p.velocity.x > 0.0);
        assert_eq!(p.lifetime, 0.6);
    }

    #[test]
    fn test_particle_preset_poof() {
        let p = ParticlePreset::poof(std::f32::consts::FRAC_PI_2, 30.0);
        assert!(p.velocity.y > 0.0);
        assert_eq!(p.lifetime, 0.5);
    }

    #[test]
    fn test_animation_state_default() {
        let state = AnimationState::default();
        assert_eq!(state, AnimationState::Idle);
    }

    #[test]
    fn test_particle_lifetime_check() {
        let mut p = ParticlePreset::spark(0.0, 0.0, 50.0);
        assert!(p.elapsed < p.lifetime);
        p.elapsed = p.lifetime + 0.1;
        assert!(p.elapsed >= p.lifetime);
    }

    #[test]
    fn test_all_particle_events() {
        let pos = Vec3::new(100.0, 200.0, 0.0);
        let events = vec![
            ParticleEvent::DamageNumber {
                position: pos,
                amount: 25,
                is_critical: false,
            },
            ParticleEvent::DamageNumber {
                position: pos,
                amount: 50,
                is_critical: true,
            },
            ParticleEvent::HealNumber {
                position: pos,
                amount: 10,
            },
            ParticleEvent::AttackSpark { position: pos },
            ParticleEvent::LevelUp { position: pos },
            ParticleEvent::ItemPickup { position: pos },
            ParticleEvent::DeathPoof { position: pos },
            ParticleEvent::SpawnGlow { position: pos },
        ];
        assert_eq!(events.len(), 8);
    }

    #[test]
    fn test_sprite_animation_states() {
        let states = [
            AnimationState::Idle,
            AnimationState::Walking,
            AnimationState::Attacking,
            AnimationState::Hurt,
            AnimationState::Dying,
        ];
        assert_eq!(states.len(), 5);
        assert_ne!(AnimationState::Attacking, AnimationState::Idle);
    }
}
