//! Game event system for periodic events and entity interactions.
//!
//! Events are triggered by tick-based conditions and processed
//! deterministically during each game tick.

use crate::ecs::EntityId;
use serde::{Deserialize, Serialize};

/// Game event types that can occur during a tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    /// An entity was spawned.
    EntitySpawned {
        entity_id: EntityId,
        entity_type: String,
        position: [f32; 3],
    },
    /// An entity died.
    EntityDied {
        entity_id: EntityId,
        entity_type: String,
        killer_id: Option<EntityId>,
        position: [f32; 3],
    },
    /// An entity was despawned (removed from world).
    EntityDespawned {
        entity_id: EntityId,
        entity_type: String,
    },
    /// A player joined the game.
    PlayerJoined {
        entity_id: EntityId,
        address: String,
    },
    /// A player leveled up.
    PlayerLevelUp {
        entity_id: EntityId,
        address: String,
        new_level: u32,
    },
    /// An item was dropped on the ground.
    ItemDropped {
        position: [f32; 3],
        item_id: u32,
        quantity: u32,
        drop_id: u64,
    },
    /// An item was picked up by a player.
    ItemPickedUp {
        entity_id: EntityId,
        item_id: u32,
        quantity: u32,
    },
    /// Combat event: damage dealt.
    CombatHit {
        attacker_id: EntityId,
        target_id: EntityId,
        damage: f32,
        is_critical: bool,
    },
    /// A chat message was sent.
    ChatMessage {
        sender_id: EntityId,
        sender_address: String,
        message: String,
    },
    /// A periodic world event started.
    WorldEvent {
        event_type: WorldEventType,
        tick: u64,
    },
}

/// Types of periodic world events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum WorldEventType {
    /// Day/night cycle change.
    DayNightTransition { is_day: bool },
    /// Weather change.
    WeatherChange { weather: WeatherType },
    /// Spawn wave of enemies.
    EnemyWave { intensity: u32 },
    /// Resource respawn across the world.
    ResourceRespawn,
    /// World boss spawned.
    WorldBoss,
}

/// Weather types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum WeatherType {
    Clear,
    Rain,
    Storm,
    Fog,
    Snow,
}

/// Ground item that can be picked up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundItem {
    /// Unique ID for this drop instance.
    pub drop_id: u64,
    /// Item definition ID.
    pub item_id: u32,
    /// Quantity.
    pub quantity: u32,
    /// World position.
    pub position: [f32; 3],
    /// Tick when this item was dropped.
    pub dropped_at: u64,
    /// Ticks until despawn (0 = never).
    pub despawn_ticks: u32,
}

/// Event manager processes and stores game events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventManager {
    /// Events emitted during the current tick.
    current_events: Vec<GameEvent>,
    /// Ground items waiting to be picked up.
    ground_items: Vec<GroundItem>,
    /// Next drop ID.
    next_drop_id: u64,
    /// Current day/night state (true = day).
    pub is_day: bool,
    /// Current weather.
    pub weather: WeatherType,
    /// Day/night cycle length in ticks.
    pub day_cycle_ticks: u64,
    /// Weather change interval in ticks.
    pub weather_interval_ticks: u64,
}

impl EventManager {
    /// Create a new event manager.
    pub fn new() -> Self {
        Self {
            current_events: Vec::new(),
            ground_items: Vec::new(),
            next_drop_id: 1,
            is_day: true,
            weather: WeatherType::Clear,
            day_cycle_ticks: 6000,        // ~20 minutes at 5 ticks/sec
            weather_interval_ticks: 3000, // ~10 minutes
        }
    }

    /// Emit an event.
    pub fn emit(&mut self, event: GameEvent) {
        self.current_events.push(event);
    }

    /// Take all events from the current tick (clears the buffer).
    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.current_events)
    }

    /// Get current events without draining.
    pub fn current_events(&self) -> &[GameEvent] {
        &self.current_events
    }

    /// Drop an item on the ground.
    pub fn drop_item(&mut self, item_id: u32, quantity: u32, position: [f32; 3], tick: u64) -> u64 {
        let drop_id = self.next_drop_id;
        self.next_drop_id += 1;

        self.ground_items.push(GroundItem {
            drop_id,
            item_id,
            quantity,
            position,
            dropped_at: tick,
            despawn_ticks: 600, // 2 minutes at 5 ticks/sec
        });

        self.emit(GameEvent::ItemDropped {
            position,
            item_id,
            quantity,
            drop_id,
        });

        drop_id
    }

    /// Try to pick up a ground item near a position.
    /// Returns (item_id, quantity) if successful.
    pub fn try_pickup(
        &mut self,
        entity_id: EntityId,
        position: [f32; 3],
        pickup_range: f32,
    ) -> Option<(u32, u32)> {
        let range_sq = pickup_range * pickup_range;
        let idx = self.ground_items.iter().position(|item| {
            let dx = item.position[0] - position[0];
            let dy = item.position[1] - position[1];
            dx * dx + dy * dy <= range_sq
        });

        if let Some(idx) = idx {
            let item = self.ground_items.remove(idx);
            self.emit(GameEvent::ItemPickedUp {
                entity_id,
                item_id: item.item_id,
                quantity: item.quantity,
            });
            Some((item.item_id, item.quantity))
        } else {
            None
        }
    }

    /// Process periodic events based on tick number.
    pub fn tick_periodic_events(&mut self, tick: u64, seed: u64) {
        // Day/night cycle
        if self.day_cycle_ticks > 0 && tick.is_multiple_of(self.day_cycle_ticks) {
            self.is_day = !self.is_day;
            self.emit(GameEvent::WorldEvent {
                event_type: WorldEventType::DayNightTransition {
                    is_day: self.is_day,
                },
                tick,
            });
        }

        // Weather changes
        if self.weather_interval_ticks > 0 && tick.is_multiple_of(self.weather_interval_ticks) {
            let new_weather = Self::deterministic_weather(tick, seed);
            if new_weather != self.weather {
                self.weather = new_weather;
                self.emit(GameEvent::WorldEvent {
                    event_type: WorldEventType::WeatherChange {
                        weather: self.weather,
                    },
                    tick,
                });
            }
        }

        // Resource respawn every 1000 ticks (~3.3 minutes)
        if tick.is_multiple_of(1000) && tick > 0 {
            self.emit(GameEvent::WorldEvent {
                event_type: WorldEventType::ResourceRespawn,
                tick,
            });
        }

        // Enemy wave every 5000 ticks (~16.6 minutes)
        if tick.is_multiple_of(5000) && tick > 0 {
            let intensity = ((tick / 5000) as u32).min(5);
            self.emit(GameEvent::WorldEvent {
                event_type: WorldEventType::EnemyWave { intensity },
                tick,
            });
        }

        // Clean up expired ground items
        self.ground_items.retain(|item| {
            if item.despawn_ticks == 0 {
                return true;
            }
            tick.saturating_sub(item.dropped_at) < item.despawn_ticks as u64
        });
    }

    /// Get all ground items.
    pub fn ground_items(&self) -> &[GroundItem] {
        &self.ground_items
    }

    /// Ground item count.
    pub fn ground_item_count(&self) -> usize {
        self.ground_items.len()
    }

    /// Deterministic weather selection.
    fn deterministic_weather(tick: u64, seed: u64) -> WeatherType {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(seed.to_le_bytes());
        hasher.update(tick.to_le_bytes());
        hasher.update(b"weather");
        let result = hasher.finalize();
        match result[0] % 5 {
            0 => WeatherType::Clear,
            1 => WeatherType::Rain,
            2 => WeatherType::Storm,
            3 => WeatherType::Fog,
            _ => WeatherType::Snow,
        }
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_and_drain() {
        let mut em = EventManager::new();
        em.emit(GameEvent::EntitySpawned {
            entity_id: 1,
            entity_type: "npc:wolf".into(),
            position: [0.0, 0.0, 0.0],
        });
        assert_eq!(em.current_events().len(), 1);
        let events = em.drain_events();
        assert_eq!(events.len(), 1);
        assert!(em.current_events().is_empty());
    }

    #[test]
    fn test_drop_and_pickup() {
        let mut em = EventManager::new();
        let drop_id = em.drop_item(20, 2, [5.0, 5.0, 0.0], 100);
        assert!(drop_id > 0);
        assert_eq!(em.ground_item_count(), 1);

        // Pickup in range
        let result = em.try_pickup(1, [5.5, 5.0, 0.0], 2.0);
        assert!(result.is_some());
        let (item_id, qty) = result.unwrap();
        assert_eq!(item_id, 20);
        assert_eq!(qty, 2);
        assert_eq!(em.ground_item_count(), 0);
    }

    #[test]
    fn test_pickup_out_of_range() {
        let mut em = EventManager::new();
        em.drop_item(20, 1, [5.0, 5.0, 0.0], 100);

        let result = em.try_pickup(1, [100.0, 100.0, 0.0], 2.0);
        assert!(result.is_none());
        assert_eq!(em.ground_item_count(), 1);
    }

    #[test]
    fn test_day_night_cycle() {
        let mut em = EventManager::new();
        assert!(em.is_day);

        em.tick_periodic_events(em.day_cycle_ticks, 42);
        assert!(!em.is_day);

        em.drain_events();
        em.tick_periodic_events(em.day_cycle_ticks * 2, 42);
        assert!(em.is_day);
    }

    #[test]
    fn test_ground_item_despawn() {
        let mut em = EventManager::new();
        em.drop_item(20, 1, [0.0, 0.0, 0.0], 0);
        assert_eq!(em.ground_item_count(), 1);

        // After despawn time, item should be removed
        em.tick_periodic_events(700, 42);
        assert_eq!(em.ground_item_count(), 0);
    }

    #[test]
    fn test_enemy_wave_event() {
        let mut em = EventManager::new();
        em.tick_periodic_events(5000, 42);
        let events = em.drain_events();
        let has_wave = events.iter().any(|e| {
            matches!(
                e,
                GameEvent::WorldEvent {
                    event_type: WorldEventType::EnemyWave { .. },
                    ..
                }
            )
        });
        assert!(has_wave);
    }

    #[test]
    fn test_weather_deterministic() {
        let w1 = EventManager::deterministic_weather(3000, 42);
        let w2 = EventManager::deterministic_weather(3000, 42);
        assert_eq!(w1, w2);
    }

    #[test]
    fn test_resource_respawn_event() {
        let mut em = EventManager::new();
        em.tick_periodic_events(1000, 42);
        let events = em.drain_events();
        let has_respawn = events.iter().any(|e| {
            matches!(
                e,
                GameEvent::WorldEvent {
                    event_type: WorldEventType::ResourceRespawn,
                    ..
                }
            )
        });
        assert!(has_respawn);
    }
}
