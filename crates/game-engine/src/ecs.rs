//! Simple Entity-Component-System for on-chain game state.
//!
//! A minimal ECS implementation designed for deterministic execution
//! and serializable state. Inspired by Veloren's use of specs,
//! but simplified for blockchain constraints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Entity identifier.
pub type EntityId = u64;

/// Component types that can be attached to entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Component {
    /// Position in the world.
    Position(PositionComponent),
    /// Health and combat stats.
    Health(HealthComponent),
    /// Player-specific data.
    Player(PlayerComponent),
    /// NPC-specific data.
    Npc(NpcComponent),
    /// Inventory data.
    Inventory(InventoryComponent),
    /// Physics body.
    Physics(PhysicsComponent),
    /// Combat stats (attack, defense, cooldowns).
    Combat(CombatComponent),
    /// NPC AI state.
    AiState(AiStateComponent),
    /// Equipment slots.
    Equipment(EquipmentComponent),
}

/// Position component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionComponent {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Health component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthComponent {
    pub current: f32,
    pub maximum: f32,
    pub is_dead: bool,
}

/// Player component (linked to an on-chain address).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerComponent {
    /// On-chain address (20 bytes, hex-encoded).
    pub address: String,
    pub name: String,
    pub level: u32,
    pub experience: u64,
}

/// NPC component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcComponent {
    pub npc_type: String,
    pub behavior: NpcBehavior,
    pub home_position: PositionComponent,
}

/// NPC behavior types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NpcBehavior {
    Idle,
    Patrol,
    Aggressive,
    Merchant,
}

/// Inventory component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryComponent {
    pub items: Vec<Item>,
    pub max_slots: u32,
}

/// An item in an inventory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub item_id: u32,
    pub quantity: u32,
    pub metadata: Option<String>,
}

/// Physics body component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhysicsComponent {
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub velocity_z: f32,
    pub mass: f32,
    pub is_grounded: bool,
}

/// Combat stats component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CombatComponent {
    pub attack: f32,
    pub defense: f32,
    pub attack_range: f32,
    pub attack_cooldown: u32,
    pub cooldown_remaining: u32,
    pub crit_chance: f32,
    pub crit_multiplier: f32,
}

impl Default for CombatComponent {
    fn default() -> Self {
        Self {
            attack: 10.0,
            defense: 5.0,
            attack_range: 2.0,
            attack_cooldown: 5,
            cooldown_remaining: 0,
            crit_chance: 0.05,
            crit_multiplier: 2.0,
        }
    }
}

/// NPC AI state component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiStateComponent {
    /// Current AI behavior state.
    pub state: NpcAiState,
    /// Patrol waypoints.
    pub waypoints: Vec<[f32; 3]>,
    /// Leash range (max distance from home).
    pub leash_range: f32,
    /// Aggro range.
    pub aggro_range: f32,
    /// Movement speed.
    pub move_speed: f32,
    /// Experience reward on kill.
    pub experience_reward: u64,
}

/// AI state machine values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NpcAiState {
    Idle { ticks_remaining: u32 },
    Patrolling { waypoint_index: usize },
    Chasing { target_id: EntityId },
    Fleeing { threat_id: EntityId, ticks_remaining: u32 },
    Returning,
    Dead { respawn_ticks: u32 },
}

/// Equipment component tracking equipped items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentComponent {
    pub main_hand: Option<u32>,
    pub off_hand: Option<u32>,
    pub head: Option<u32>,
    pub chest: Option<u32>,
    pub legs: Option<u32>,
    pub boots: Option<u32>,
}

/// The ECS world storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcsWorld {
    /// Next available entity ID.
    next_entity_id: EntityId,
    /// All entities and their components.
    entities: HashMap<EntityId, Vec<Component>>,
}

impl EcsWorld {
    /// Create a new empty ECS world.
    pub fn new() -> Self {
        Self {
            next_entity_id: 1,
            entities: HashMap::new(),
        }
    }

    /// Spawn a new entity with the given components.
    pub fn spawn(&mut self, components: Vec<Component>) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        self.entities.insert(id, components);
        id
    }

    /// Despawn an entity.
    pub fn despawn(&mut self, entity: EntityId) -> bool {
        self.entities.remove(&entity).is_some()
    }

    /// Get all components of an entity.
    pub fn get_components(&self, entity: EntityId) -> Option<&Vec<Component>> {
        self.entities.get(&entity)
    }

    /// Get all components of an entity (mutable).
    pub fn get_components_mut(&mut self, entity: EntityId) -> Option<&mut Vec<Component>> {
        self.entities.get_mut(&entity)
    }

    /// Get a specific component type from an entity.
    pub fn get_component(&self, entity: EntityId, matcher: fn(&Component) -> bool) -> Option<&Component> {
        self.entities
            .get(&entity)?
            .iter()
            .find(|c| matcher(c))
    }

    /// Get the position of an entity.
    pub fn get_position(&self, entity: EntityId) -> Option<&PositionComponent> {
        self.entities.get(&entity)?.iter().find_map(|c| match c {
            Component::Position(pos) => Some(pos),
            _ => None,
        })
    }

    /// Get all entities that have a specific component type.
    pub fn entities_with(&self, matcher: fn(&Component) -> bool) -> Vec<EntityId> {
        self.entities
            .iter()
            .filter(|(_, components)| components.iter().any(&matcher))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all player entities.
    pub fn player_entities(&self) -> Vec<EntityId> {
        self.entities_with(|c| matches!(c, Component::Player(_)))
    }

    /// Get all NPC entities.
    pub fn npc_entities(&self) -> Vec<EntityId> {
        self.entities_with(|c| matches!(c, Component::Npc(_)))
    }

    /// Iterate over all entities and their components.
    pub fn all_entities(&self) -> impl Iterator<Item = (&EntityId, &Vec<Component>)> {
        self.entities.iter()
    }

    /// Get total entity count.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Serialize the entire ECS world to bytes.
    pub fn serialize(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| e.to_string())
    }

    /// Deserialize an ECS world from bytes.
    pub fn deserialize(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| e.to_string())
    }

    /// Compute a deterministic hash of the world state.
    pub fn state_hash(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let data = self.serialize().unwrap_or_default();
        let hash = Keccak256::digest(&data);
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        result
    }
}

impl Default for EcsWorld {
    fn default() -> Self {
        Self::new()
    }
}
