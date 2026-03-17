//! Game world - the main entry point for game state management.
//!
//! The GameWorld wraps the ECS and provides high-level operations
//! for processing player actions and running game ticks.

use crate::ecs::*;
use crate::error::GameEngineError;
use crate::events::EventManager;
use crate::items::ItemRegistry;
use crate::npc_ai::SpawnManager;
use crate::systems;
use crate::terrain::WorldTerrain;
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
    /// Terrain data with chunk caching.
    terrain: RwLock<WorldTerrain>,
    /// NPC spawn manager.
    spawn_manager: RwLock<SpawnManager>,
    /// Game event manager.
    event_manager: RwLock<EventManager>,
    /// Item registry.
    item_registry: ItemRegistry,
}

impl GameWorld {
    /// Create a new game world with the given seed.
    pub fn new(seed: u64) -> Self {
        let mut ecs = EcsWorld::new();
        let mut terrain = WorldTerrain::new(seed);
        let mut spawn_manager = SpawnManager::new();
        let event_manager = EventManager::new();
        let item_registry = ItemRegistry::default_registry();

        // Generate terrain around spawn
        terrain.generate_spawn_area();

        // Generate NPC spawn points
        spawn_manager.generate_spawn_points(seed);

        // Spawn initial NPCs from spawn manager
        let initial_spawns = spawn_manager.tick_spawns();
        for (npc_type, position, waypoints) in initial_spawns {
            if let Some(archetype) = spawn_manager.get_archetype(&npc_type) {
                let mut components = vec![
                    Component::Position(PositionComponent {
                        x: position[0],
                        y: position[1],
                        z: position[2],
                    }),
                    Component::Health(HealthComponent {
                        current: archetype.max_health,
                        maximum: archetype.max_health,
                        is_dead: false,
                    }),
                    Component::Npc(NpcComponent {
                        npc_type: npc_type.clone(),
                        behavior: match archetype.behavior_pattern {
                            crate::npc_ai::BehaviorPattern::Passive => NpcBehavior::Idle,
                            crate::npc_ai::BehaviorPattern::PatrolPassive
                            | crate::npc_ai::BehaviorPattern::PatrolAggressive => NpcBehavior::Patrol,
                            crate::npc_ai::BehaviorPattern::Guardian => NpcBehavior::Aggressive,
                            crate::npc_ai::BehaviorPattern::Timid => NpcBehavior::Idle,
                            crate::npc_ai::BehaviorPattern::Predator => NpcBehavior::Aggressive,
                        },
                        home_position: PositionComponent {
                            x: position[0],
                            y: position[1],
                            z: position[2],
                        },
                    }),
                    Component::Physics(PhysicsComponent {
                        velocity_x: 0.0,
                        velocity_y: 0.0,
                        velocity_z: 0.0,
                        mass: 80.0,
                        is_grounded: true,
                    }),
                    Component::Combat(CombatComponent {
                        attack: archetype.attack_damage,
                        defense: archetype.attack_damage * 0.3,
                        attack_range: archetype.attack_range,
                        attack_cooldown: archetype.attack_cooldown,
                        cooldown_remaining: 0,
                        crit_chance: 0.0,
                        crit_multiplier: 1.5,
                    }),
                    Component::AiState(AiStateComponent {
                        state: NpcAiState::Idle { ticks_remaining: 10 },
                        waypoints,
                        leash_range: archetype.leash_range,
                        aggro_range: archetype.aggro_range,
                        move_speed: archetype.move_speed,
                        experience_reward: archetype.experience_reward,
                    }),
                ];

                // Merchants don't need combat stats
                if archetype.attack_damage == 0.0 {
                    components.retain(|c| !matches!(c, Component::Combat(_)));
                }

                ecs.spawn(components);
            }
        }

        info!(
            "Game world initialized with seed={}, entities={}, chunks={}",
            seed,
            ecs.entity_count(),
            terrain.cached_chunk_count()
        );

        Self {
            ecs: RwLock::new(ecs),
            current_tick: RwLock::new(0),
            seed,
            terrain: RwLock::new(terrain),
            spawn_manager: RwLock::new(spawn_manager),
            event_manager: RwLock::new(event_manager),
            item_registry,
        }
    }

    /// Load a game world from serialized state.
    pub fn from_state(data: &[u8], seed: u64) -> Result<Self, GameEngineError> {
        let ecs = EcsWorld::deserialize(data)
            .map_err(GameEngineError::Serialization)?;
        let mut terrain = WorldTerrain::new(seed);
        terrain.generate_spawn_area();
        let mut spawn_manager = SpawnManager::new();
        spawn_manager.generate_spawn_points(seed);
        let tick = 0; // Will be updated from chain state
        Ok(Self {
            ecs: RwLock::new(ecs),
            current_tick: RwLock::new(tick),
            seed,
            terrain: RwLock::new(terrain),
            spawn_manager: RwLock::new(spawn_manager),
            event_manager: RwLock::new(EventManager::new()),
            item_registry: ItemRegistry::default_registry(),
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

        // 3. Process periodic events (day/night, weather, spawns)
        {
            let mut event_mgr = self.event_manager.write();
            event_mgr.tick_periodic_events(current_tick, self.seed);
        }

        // 4. Tick NPC spawn manager
        {
            let mut spawn_mgr = self.spawn_manager.write();
            let new_spawns = spawn_mgr.tick_spawns();
            for (npc_type, position, waypoints) in new_spawns {
                if let Some(archetype) = spawn_mgr.get_archetype(&npc_type) {
                    let components = vec![
                        Component::Position(PositionComponent {
                            x: position[0],
                            y: position[1],
                            z: position[2],
                        }),
                        Component::Health(HealthComponent {
                            current: archetype.max_health,
                            maximum: archetype.max_health,
                            is_dead: false,
                        }),
                        Component::Npc(NpcComponent {
                            npc_type: npc_type.clone(),
                            behavior: match archetype.behavior_pattern {
                                crate::npc_ai::BehaviorPattern::Passive => NpcBehavior::Idle,
                                crate::npc_ai::BehaviorPattern::PatrolPassive
                                | crate::npc_ai::BehaviorPattern::PatrolAggressive => NpcBehavior::Patrol,
                                crate::npc_ai::BehaviorPattern::Guardian => NpcBehavior::Aggressive,
                                crate::npc_ai::BehaviorPattern::Timid => NpcBehavior::Idle,
                                crate::npc_ai::BehaviorPattern::Predator => NpcBehavior::Aggressive,
                            },
                            home_position: PositionComponent {
                                x: position[0],
                                y: position[1],
                                z: position[2],
                            },
                        }),
                        Component::Physics(PhysicsComponent {
                            velocity_x: 0.0,
                            velocity_y: 0.0,
                            velocity_z: 0.0,
                            mass: 80.0,
                            is_grounded: true,
                        }),
                        Component::Combat(CombatComponent {
                            attack: archetype.attack_damage,
                            defense: archetype.attack_damage * 0.3,
                            attack_range: archetype.attack_range,
                            attack_cooldown: archetype.attack_cooldown,
                            cooldown_remaining: 0,
                            crit_chance: 0.0,
                            crit_multiplier: 1.5,
                        }),
                        Component::AiState(AiStateComponent {
                            state: NpcAiState::Idle { ticks_remaining: 10 },
                            waypoints,
                            leash_range: archetype.leash_range,
                            aggro_range: archetype.aggro_range,
                            move_speed: archetype.move_speed,
                            experience_reward: archetype.experience_reward,
                        }),
                    ];
                    ecs.spawn(components);
                }
            }
        }

        // 5. Compute the new game state hash
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
            Component::Combat(CombatComponent::default()),
            Component::Equipment(EquipmentComponent {
                main_hand: None,
                off_hand: None,
                head: None,
                chest: None,
                legs: None,
                boots: None,
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

    /// Get a reference to the terrain system.
    pub fn terrain(&self) -> &RwLock<WorldTerrain> {
        &self.terrain
    }

    /// Get a reference to the item registry.
    pub fn item_registry(&self) -> &ItemRegistry {
        &self.item_registry
    }

    /// Get a reference to the event manager.
    pub fn event_manager(&self) -> &RwLock<EventManager> {
        &self.event_manager
    }

    /// Get a reference to the spawn manager.
    pub fn spawn_manager(&self) -> &RwLock<SpawnManager> {
        &self.spawn_manager
    }

    /// Get the number of player entities.
    pub fn player_count(&self) -> usize {
        self.ecs.read().player_entities().len()
    }

    /// Get all player infos (for snapshot queries).
    pub fn get_all_players(&self) -> Vec<crate::game_api_types::PlayerInfo> {
        let ecs = self.ecs.read();
        let players = ecs.player_entities();
        let mut result = Vec::new();
        for entity_id in players {
            if let Some(components) = ecs.get_components(entity_id) {
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
                            info.address = player.address.clone();
                            info.level = player.level;
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
                result.push(info);
            }
        }
        result
    }

    /// Get entities within a given area (bounding box query).
    pub fn get_entities_in_area(
        &self,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
    ) -> Vec<crate::game_api_types::EntitySnapshot> {
        let ecs = self.ecs.read();
        let mut result = Vec::new();
        for (entity_id, components) in ecs.all_entities() {
            let mut pos = None;
            let mut entity_type = "unknown".to_string();
            let mut health = None;
            for component in components {
                match component {
                    Component::Position(p) => pos = Some(p),
                    Component::Player(player) => entity_type = format!("player:{}", player.address),
                    Component::Npc(npc) => entity_type = format!("npc:{}", npc.npc_type),
                    Component::Health(h) => health = Some((h.current, h.maximum, h.is_dead)),
                    _ => {}
                }
            }
            if let Some(p) = pos {
                if p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y {
                    result.push(crate::game_api_types::EntitySnapshot {
                        entity_id: *entity_id,
                        entity_type,
                        position: [p.x, p.y, p.z],
                        health: health.map(|(c, m, d)| [c, m, if d { 1.0 } else { 0.0 }]),
                    });
                }
            }
        }
        result
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
