//! Game world - the main entry point for game state management.
//!
//! The GameWorld wraps the ECS and provides high-level operations
//! for processing player actions and running game ticks.

use crate::ecs::*;
use crate::error::GameEngineError;
use crate::systems;
use alloy_primitives::B256;
use parking_lot::RwLock;
use tracing::{debug, info};
use velochain_primitives::transaction::GameAction;

/// The game world state.
pub struct GameWorld {
    /// ECS world containing all entities and components.
    ecs: RwLock<EcsWorld>,
    /// Current game tick.
    current_tick: RwLock<u64>,
    /// World seed for deterministic generation.
    seed: u64,
}

impl GameWorld {
    /// Create a new game world with the given seed.
    pub fn new(seed: u64) -> Self {
        let mut ecs = EcsWorld::new();

        // Spawn some initial NPCs
        ecs.spawn(vec![
            Component::Position(PositionComponent {
                x: 10.0,
                y: 10.0,
                z: 0.0,
            }),
            Component::Health(HealthComponent {
                current: 100.0,
                maximum: 100.0,
                is_dead: false,
            }),
            Component::Npc(NpcComponent {
                npc_type: "merchant".to_string(),
                behavior: NpcBehavior::Idle,
                home_position: PositionComponent {
                    x: 10.0,
                    y: 10.0,
                    z: 0.0,
                },
            }),
        ]);

        ecs.spawn(vec![
            Component::Position(PositionComponent {
                x: 50.0,
                y: 50.0,
                z: 0.0,
            }),
            Component::Health(HealthComponent {
                current: 50.0,
                maximum: 50.0,
                is_dead: false,
            }),
            Component::Npc(NpcComponent {
                npc_type: "guard".to_string(),
                behavior: NpcBehavior::Patrol,
                home_position: PositionComponent {
                    x: 50.0,
                    y: 50.0,
                    z: 0.0,
                },
            }),
            Component::Physics(PhysicsComponent {
                velocity_x: 0.0,
                velocity_y: 0.0,
                velocity_z: 0.0,
                mass: 80.0,
                is_grounded: true,
            }),
        ]);

        info!("Game world initialized with seed={}, entities={}", seed, ecs.entity_count());

        Self {
            ecs: RwLock::new(ecs),
            current_tick: RwLock::new(0),
            seed,
        }
    }

    /// Load a game world from serialized state.
    pub fn from_state(data: &[u8], seed: u64) -> Result<Self, GameEngineError> {
        let ecs = EcsWorld::deserialize(data)
            .map_err(GameEngineError::Serialization)?;
        let tick = 0; // Will be updated from chain state
        Ok(Self {
            ecs: RwLock::new(ecs),
            current_tick: RwLock::new(tick),
            seed,
        })
    }

    /// Run one game tick, processing the given player actions.
    ///
    /// This is the core function called once per block. It must be
    /// completely deterministic: given the same state and actions,
    /// it must always produce the same result.
    pub fn tick(&self, actions: &[(String, GameAction)]) -> Result<B256, GameEngineError> {
        let mut ecs = self.ecs.write();
        let mut tick = self.current_tick.write();
        *tick += 1;
        let current_tick = *tick;

        // 1. Process player actions
        for (player_address, action) in actions {
            self.process_action(&mut ecs, player_address, action, current_tick)?;
        }

        // 2. Run game systems (physics, AI, combat, etc.)
        systems::run_tick(&mut ecs, current_tick);

        // 3. Compute the new game state hash
        let state_hash = ecs.state_hash();
        let root = B256::from_slice(&state_hash);

        debug!("Game tick {} complete: state_root={}", current_tick, root);
        Ok(root)
    }

    /// Process a single player action.
    fn process_action(
        &self,
        ecs: &mut EcsWorld,
        player_address: &str,
        action: &GameAction,
        _tick: u64,
    ) -> Result<(), GameEngineError> {
        match action {
            GameAction::Move { x, y, z } => {
                // Convert from milliunits (i64) to f32
                let fx = *x as f32 / 1000.0;
                let fy = *y as f32 / 1000.0;
                let fz = *z as f32 / 1000.0;
                // Find the player entity
                let entity = self.find_player_entity(ecs, player_address);
                match entity {
                    Some(entity_id) => {
                        if let Some(components) = ecs.get_components_mut(entity_id) {
                            for component in components.iter_mut() {
                                if let Component::Position(pos) = component {
                                    pos.x = fx;
                                    pos.y = fy;
                                    pos.z = fz;
                                }
                            }
                        }
                        Ok(())
                    }
                    None => {
                        // Auto-spawn player on first action
                        self.spawn_player(ecs, player_address, fx, fy, fz);
                        Ok(())
                    }
                }
            }
            GameAction::Chat { message: _ } => {
                // Chat messages are stored in block data but don't affect game state
                Ok(())
            }
            GameAction::Respawn => {
                if let Some(entity_id) = self.find_player_entity(ecs, player_address) {
                    if let Some(components) = ecs.get_components_mut(entity_id) {
                        for component in components.iter_mut() {
                            match component {
                                Component::Health(health) => {
                                    health.current = health.maximum;
                                    health.is_dead = false;
                                }
                                Component::Position(pos) => {
                                    pos.x = 0.0;
                                    pos.y = 0.0;
                                    pos.z = 64.0;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(())
            }
            GameAction::Attack { target_entity_id } => {
                // Apply damage to target
                if let Some(components) = ecs.get_components_mut(*target_entity_id) {
                    for component in components.iter_mut() {
                        if let Component::Health(health) = component {
                            health.current -= 10.0; // Base damage
                            if health.current <= 0.0 {
                                health.current = 0.0;
                                health.is_dead = true;
                            }
                        }
                    }
                    Ok(())
                } else {
                    Err(GameEngineError::EntityNotFound(*target_entity_id))
                }
            }
            _ => {
                debug!("Unhandled game action: {:?}", action);
                Ok(())
            }
        }
    }

    /// Find a player entity by their on-chain address.
    fn find_player_entity(&self, ecs: &EcsWorld, address: &str) -> Option<EntityId> {
        let players = ecs.player_entities();
        for entity_id in players {
            if let Some(components) = ecs.get_components(entity_id) {
                for component in components {
                    if let Component::Player(player) = component {
                        if player.address == address {
                            return Some(entity_id);
                        }
                    }
                }
            }
        }
        None
    }

    /// Spawn a new player entity.
    fn spawn_player(&self, ecs: &mut EcsWorld, address: &str, x: f32, y: f32, z: f32) -> EntityId {
        let entity = ecs.spawn(vec![
            Component::Position(PositionComponent { x, y, z }),
            Component::Health(HealthComponent {
                current: 100.0,
                maximum: 100.0,
                is_dead: false,
            }),
            Component::Player(PlayerComponent {
                address: address.to_string(),
                name: format!("Player_{}", &address[..8.min(address.len())]),
                level: 1,
                experience: 0,
            }),
            Component::Physics(PhysicsComponent {
                velocity_x: 0.0,
                velocity_y: 0.0,
                velocity_z: 0.0,
                mass: 70.0,
                is_grounded: true,
            }),
            Component::Inventory(InventoryComponent {
                items: vec![],
                max_slots: 20,
            }),
        ]);

        info!("Player spawned: address={}, entity_id={}", address, entity);
        entity
    }

    /// Get the current tick number.
    pub fn current_tick(&self) -> u64 {
        *self.current_tick.read()
    }

    /// Get the entity count.
    pub fn entity_count(&self) -> usize {
        self.ecs.read().entity_count()
    }

    /// Serialize the game world state.
    pub fn serialize_state(&self) -> Result<Vec<u8>, GameEngineError> {
        self.ecs
            .read()
            .serialize()
            .map_err(GameEngineError::Serialization)
    }

    /// Compute the game state root hash.
    pub fn state_root(&self) -> B256 {
        let hash = self.ecs.read().state_hash();
        B256::from_slice(&hash)
    }

    /// Get the world seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Get the number of player entities.
    pub fn player_count(&self) -> usize {
        self.ecs.read().player_entities().len()
    }

    /// Get player info by on-chain address (for RPC queries).
    pub fn get_player_info(&self, address: &str) -> Option<crate::game_api_types::PlayerInfo> {
        let ecs = self.ecs.read();
        let players = ecs.player_entities();
        for entity_id in players {
            if let Some(components) = ecs.get_components(entity_id) {
                let mut is_match = false;
                let mut info = crate::game_api_types::PlayerInfo {
                    entity_id,
                    address: String::new(),
                    position: [0.0, 0.0, 0.0],
                    health: 0.0,
                    max_health: 0.0,
                    level: 0,
                    is_alive: true,
                };
                for component in components {
                    match component {
                        Component::Player(player) => {
                            if player.address == address {
                                is_match = true;
                                info.address = player.address.clone();
                                info.level = player.level;
                            }
                        }
                        Component::Position(pos) => {
                            info.position = [pos.x, pos.y, pos.z];
                        }
                        Component::Health(health) => {
                            info.health = health.current;
                            info.max_health = health.maximum;
                            info.is_alive = !health.is_dead;
                        }
                        _ => {}
                    }
                }
                if is_match {
                    return Some(info);
                }
            }
        }
        None
    }
}
