//! Sound system for BGM, sound effects, and environment audio.
//!
//! Provides a volume-controlled audio manager with event-driven
//! sound playback. Compatible with both native and WASM targets.
//! Uses a queue-based approach since actual audio playback depends
//! on the platform's audio backend.

use bevy::prelude::*;

/// Plugin for sound management.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SoundConfig::default())
            .insert_resource(SoundState::default())
            .add_event::<SoundEvent>()
            .add_systems(Update, (handle_sound_events, update_bgm_state));
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Sound volume and playback configuration.
#[derive(Resource, Clone)]
pub struct SoundConfig {
    /// Master volume (0.0 - 1.0).
    pub master_volume: f32,
    /// Background music volume.
    pub bgm_volume: f32,
    /// Sound effects volume.
    pub sfx_volume: f32,
    /// Environment sounds volume.
    pub env_volume: f32,
    /// Whether sound is globally muted.
    pub muted: bool,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            bgm_volume: 0.6,
            sfx_volume: 0.8,
            env_volume: 0.5,
            muted: false,
        }
    }
}

impl SoundConfig {
    /// Effective BGM volume accounting for master and mute.
    pub fn effective_bgm(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.bgm_volume
        }
    }

    /// Effective SFX volume.
    pub fn effective_sfx(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.sfx_volume
        }
    }

    /// Effective environment volume.
    pub fn effective_env(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.master_volume * self.env_volume
        }
    }
}

// ---------------------------------------------------------------------------
// Sound events
// ---------------------------------------------------------------------------

/// Sound effect event to be played.
#[derive(Event, Clone, Debug)]
pub enum SoundEvent {
    // --- Combat ---
    /// Melee attack swing.
    AttackSwing,
    /// Attack hit impact.
    AttackHit,
    /// Ranged attack / projectile.
    RangedAttack,
    /// Taking damage.
    TakeDamage,
    /// Healing effect.
    Heal,
    /// Entity death.
    Death,
    /// Critical hit.
    CriticalHit,

    // --- Player actions ---
    /// Item pickup from ground.
    ItemPickup,
    /// Item drop.
    ItemDrop,
    /// Level up celebration.
    LevelUp,
    /// Crafting start.
    CraftStart,
    /// Crafting complete.
    CraftComplete,
    /// Quest accepted.
    QuestAccept,
    /// Quest complete.
    QuestComplete,
    /// Skill learned.
    SkillLearn,

    // --- UI ---
    /// UI button click.
    UiClick,
    /// Panel open.
    PanelOpen,
    /// Panel close.
    PanelClose,
    /// Error/deny sound.
    UiError,
    /// Chat message received.
    ChatMessage,

    // --- BGM control ---
    /// Change background music track.
    ChangeBgm { track: BgmTrack },
    /// Stop BGM.
    StopBgm,

    // --- Environment ---
    /// Change ambient environment sound.
    ChangeAmbient { ambient: AmbientSound },
    /// Stop ambient sound.
    StopAmbient,
}

/// Background music tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BgmTrack {
    /// Peaceful exploration.
    Overworld,
    /// Combat encounter.
    Combat,
    /// Town/village.
    Town,
    /// Dungeon/cave.
    Dungeon,
    /// Night time.
    Night,
    /// Boss fight.
    Boss,
    /// Menu/title screen.
    Menu,
}

/// Ambient environment sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AmbientSound {
    /// Forest birds and wind.
    Forest,
    /// Ocean waves.
    Ocean,
    /// Rain falling.
    Rain,
    /// Heavy wind.
    Wind,
    /// Cave drips.
    Cave,
    /// Town bustle.
    TownBustle,
    /// Silence.
    None,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Current sound system state.
#[derive(Resource)]
pub struct SoundState {
    /// Currently playing BGM track.
    pub current_bgm: Option<BgmTrack>,
    /// Target BGM (for crossfade transitions).
    pub target_bgm: Option<BgmTrack>,
    /// Current ambient sound.
    pub current_ambient: AmbientSound,
    /// BGM crossfade progress (0.0 = old track, 1.0 = new track).
    pub crossfade_progress: f32,
    /// Whether crossfade is in progress.
    pub crossfading: bool,
    /// Recent sound effect log (for deduplication).
    pub recent_sfx: Vec<(SfxId, f32)>,
    /// Minimum interval between same SFX (seconds).
    pub sfx_cooldown: f32,
    /// Queued sound effects for playback.
    pub sfx_queue: Vec<QueuedSfx>,
}

impl Default for SoundState {
    fn default() -> Self {
        Self {
            current_bgm: None,
            target_bgm: None,
            current_ambient: AmbientSound::None,
            crossfade_progress: 0.0,
            crossfading: false,
            recent_sfx: Vec::new(),
            sfx_cooldown: 0.05,
            sfx_queue: Vec::new(),
        }
    }
}

/// Identifier for sound deduplication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SfxId {
    AttackSwing,
    AttackHit,
    RangedAttack,
    TakeDamage,
    Heal,
    Death,
    CriticalHit,
    ItemPickup,
    ItemDrop,
    LevelUp,
    CraftStart,
    CraftComplete,
    QuestAccept,
    QuestComplete,
    SkillLearn,
    UiClick,
    PanelOpen,
    PanelClose,
    UiError,
    ChatMessage,
}

/// A queued sound effect ready for playback.
#[derive(Clone, Debug)]
pub struct QueuedSfx {
    /// Sound identifier.
    pub id: SfxId,
    /// Volume multiplier (0.0 - 1.0).
    pub volume: f32,
    /// Pitch variation.
    pub pitch: f32,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn handle_sound_events(
    time: Res<Time>,
    config: Res<SoundConfig>,
    mut state: ResMut<SoundState>,
    mut events: EventReader<SoundEvent>,
) {
    let now = time.elapsed_secs();

    // Clean up old recent sfx entries
    let cooldown = state.sfx_cooldown;
    state.recent_sfx.retain(|(_, t)| now - t < cooldown);

    for event in events.read() {
        match event {
            SoundEvent::ChangeBgm { track } => {
                if state.current_bgm != Some(*track) {
                    state.target_bgm = Some(*track);
                    state.crossfading = true;
                    state.crossfade_progress = 0.0;
                }
            }
            SoundEvent::StopBgm => {
                state.target_bgm = None;
                state.crossfading = true;
                state.crossfade_progress = 0.0;
            }
            SoundEvent::ChangeAmbient { ambient } => {
                state.current_ambient = *ambient;
            }
            SoundEvent::StopAmbient => {
                state.current_ambient = AmbientSound::None;
            }
            _ => {
                // Map event to SfxId and queue
                if let Some(sfx_id) = event_to_sfx_id(event) {
                    // Deduplication check
                    if !state.recent_sfx.iter().any(|(id, _)| *id == sfx_id) {
                        let volume = config.effective_sfx();
                        let pitch = 1.0 + ((now * 100.0) % 10.0 - 5.0) * 0.01;
                        state.sfx_queue.push(QueuedSfx {
                            id: sfx_id,
                            volume,
                            pitch,
                        });
                        state.recent_sfx.push((sfx_id, now));
                    }
                }
            }
        }
    }
}

fn update_bgm_state(time: Res<Time>, mut state: ResMut<SoundState>) {
    if !state.crossfading {
        return;
    }

    let dt = time.delta_secs();
    state.crossfade_progress = (state.crossfade_progress + dt * 0.5).min(1.0);

    if state.crossfade_progress >= 1.0 {
        state.current_bgm = state.target_bgm;
        state.crossfading = false;
        state.crossfade_progress = 0.0;
    }
}

fn event_to_sfx_id(event: &SoundEvent) -> Option<SfxId> {
    match event {
        SoundEvent::AttackSwing => Some(SfxId::AttackSwing),
        SoundEvent::AttackHit => Some(SfxId::AttackHit),
        SoundEvent::RangedAttack => Some(SfxId::RangedAttack),
        SoundEvent::TakeDamage => Some(SfxId::TakeDamage),
        SoundEvent::Heal => Some(SfxId::Heal),
        SoundEvent::Death => Some(SfxId::Death),
        SoundEvent::CriticalHit => Some(SfxId::CriticalHit),
        SoundEvent::ItemPickup => Some(SfxId::ItemPickup),
        SoundEvent::ItemDrop => Some(SfxId::ItemDrop),
        SoundEvent::LevelUp => Some(SfxId::LevelUp),
        SoundEvent::CraftStart => Some(SfxId::CraftStart),
        SoundEvent::CraftComplete => Some(SfxId::CraftComplete),
        SoundEvent::QuestAccept => Some(SfxId::QuestAccept),
        SoundEvent::QuestComplete => Some(SfxId::QuestComplete),
        SoundEvent::SkillLearn => Some(SfxId::SkillLearn),
        SoundEvent::UiClick => Some(SfxId::UiClick),
        SoundEvent::PanelOpen => Some(SfxId::PanelOpen),
        SoundEvent::PanelClose => Some(SfxId::PanelClose),
        SoundEvent::UiError => Some(SfxId::UiError),
        SoundEvent::ChatMessage => Some(SfxId::ChatMessage),
        SoundEvent::ChangeBgm { .. }
        | SoundEvent::StopBgm
        | SoundEvent::ChangeAmbient { .. }
        | SoundEvent::StopAmbient => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_config_defaults() {
        let config = SoundConfig::default();
        assert_eq!(config.master_volume, 0.8);
        assert!(!config.muted);
    }

    #[test]
    fn test_effective_volumes() {
        let config = SoundConfig::default();
        let bgm = config.effective_bgm();
        assert!((bgm - 0.48).abs() < 0.01); // 0.8 * 0.6

        let sfx = config.effective_sfx();
        assert!((sfx - 0.64).abs() < 0.01); // 0.8 * 0.8

        let env = config.effective_env();
        assert!((env - 0.40).abs() < 0.01); // 0.8 * 0.5
    }

    #[test]
    fn test_muted_volumes() {
        let mut config = SoundConfig::default();
        config.muted = true;
        assert_eq!(config.effective_bgm(), 0.0);
        assert_eq!(config.effective_sfx(), 0.0);
        assert_eq!(config.effective_env(), 0.0);
    }

    #[test]
    fn test_sound_state_default() {
        let state = SoundState::default();
        assert!(state.current_bgm.is_none());
        assert_eq!(state.current_ambient, AmbientSound::None);
        assert!(!state.crossfading);
        assert!(state.sfx_queue.is_empty());
    }

    #[test]
    fn test_event_to_sfx_mapping() {
        assert_eq!(
            event_to_sfx_id(&SoundEvent::AttackSwing),
            Some(SfxId::AttackSwing)
        );
        assert_eq!(event_to_sfx_id(&SoundEvent::LevelUp), Some(SfxId::LevelUp));
        assert_eq!(event_to_sfx_id(&SoundEvent::UiClick), Some(SfxId::UiClick));
        assert_eq!(
            event_to_sfx_id(&SoundEvent::ChangeBgm {
                track: BgmTrack::Combat
            }),
            None
        );
        assert_eq!(event_to_sfx_id(&SoundEvent::StopBgm), None);
        assert_eq!(
            event_to_sfx_id(&SoundEvent::ChangeAmbient {
                ambient: AmbientSound::Rain
            }),
            None
        );
    }

    #[test]
    fn test_bgm_tracks_all_variants() {
        let tracks = [
            BgmTrack::Overworld,
            BgmTrack::Combat,
            BgmTrack::Town,
            BgmTrack::Dungeon,
            BgmTrack::Night,
            BgmTrack::Boss,
            BgmTrack::Menu,
        ];
        assert_eq!(tracks.len(), 7);
    }

    #[test]
    fn test_ambient_sounds_all_variants() {
        let sounds = [
            AmbientSound::Forest,
            AmbientSound::Ocean,
            AmbientSound::Rain,
            AmbientSound::Wind,
            AmbientSound::Cave,
            AmbientSound::TownBustle,
            AmbientSound::None,
        ];
        assert_eq!(sounds.len(), 7);
    }

    #[test]
    fn test_queued_sfx() {
        let sfx = QueuedSfx {
            id: SfxId::AttackHit,
            volume: 0.8,
            pitch: 1.05,
        };
        assert_eq!(sfx.id, SfxId::AttackHit);
        assert!(sfx.pitch > 1.0);
    }

    #[test]
    fn test_sfx_id_equality() {
        assert_eq!(SfxId::LevelUp, SfxId::LevelUp);
        assert_ne!(SfxId::LevelUp, SfxId::Death);
    }
}
