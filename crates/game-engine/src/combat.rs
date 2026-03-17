//! Combat system with damage calculation, range checks, cooldowns, and PvP/PvE rules.
//!
//! All combat is deterministic and executed as part of the game tick.

use crate::ecs::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Combat stats for an entity (computed from base stats + equipment).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CombatStats {
    /// Base attack damage.
    pub attack: f32,
    /// Defense (reduces incoming damage).
    pub defense: f32,
    /// Attack range in world units.
    pub attack_range: f32,
    /// Cooldown between attacks in ticks.
    pub attack_cooldown: u32,
    /// Ticks until next attack is available.
    pub cooldown_remaining: u32,
    /// Critical hit chance (0.0 - 1.0).
    pub crit_chance: f32,
    /// Critical hit damage multiplier.
    pub crit_multiplier: f32,
}

impl CombatStats {
    /// Create default player combat stats.
    pub fn player_default() -> Self {
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

    /// Create NPC combat stats from archetype values.
    pub fn from_npc(damage: f32, range: f32, cooldown: u32) -> Self {
        Self {
            attack: damage,
            defense: damage * 0.3, // NPCs get defense proportional to attack
            attack_range: range,
            attack_cooldown: cooldown,
            cooldown_remaining: 0,
            crit_chance: 0.0,
            crit_multiplier: 1.5,
        }
    }

    /// Check if this entity can attack right now.
    pub fn can_attack(&self) -> bool {
        self.cooldown_remaining == 0 && self.attack > 0.0
    }

    /// Tick cooldown by one.
    pub fn tick_cooldown(&mut self) {
        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
        }
    }

    /// Start attack cooldown after performing an attack.
    pub fn start_cooldown(&mut self) {
        self.cooldown_remaining = self.attack_cooldown;
    }
}

/// Result of a damage calculation.
#[derive(Debug, Clone)]
pub struct DamageResult {
    /// Raw damage before defense.
    pub raw_damage: f32,
    /// Final damage after defense reduction.
    pub final_damage: f32,
    /// Whether this was a critical hit.
    pub is_critical: bool,
    /// Whether the target died.
    pub is_kill: bool,
}

/// Calculate damage from attacker to defender.
///
/// Formula: final_damage = max(1, attack * crit_mult - defense * 0.5)
/// This ensures at least 1 damage is always dealt.
pub fn calculate_damage(
    attacker: &CombatStats,
    defender: &CombatStats,
    tick: u64,
    attacker_id: u64,
) -> DamageResult {
    // Deterministic "random" for crit using tick and attacker ID
    let crit_roll = deterministic_float(tick, attacker_id, 0);
    let is_critical = crit_roll < attacker.crit_chance;

    let crit_mult = if is_critical {
        attacker.crit_multiplier
    } else {
        1.0
    };

    let raw_damage = attacker.attack * crit_mult;
    let defense_reduction = defender.defense * 0.5;
    let final_damage = (raw_damage - defense_reduction).max(1.0);

    DamageResult {
        raw_damage,
        final_damage,
        is_critical,
        is_kill: false, // Set by caller after applying damage
    }
}

/// Check if attacker is in range of target.
pub fn is_in_range(
    attacker_pos: &PositionComponent,
    target_pos: &PositionComponent,
    attack_range: f32,
) -> bool {
    let dx = attacker_pos.x - target_pos.x;
    let dy = attacker_pos.y - target_pos.y;
    let dist_sq = dx * dx + dy * dy;
    dist_sq <= attack_range * attack_range
}

/// Experience required to reach the next level.
pub fn exp_for_level(level: u32) -> u64 {
    // Simple quadratic scaling: 100 * level^2
    100 * (level as u64) * (level as u64)
}

/// Calculate experience gained from killing an entity.
pub fn exp_gained(killer_level: u32, victim_level: u32, base_exp: u64) -> u64 {
    let level_diff = victim_level as i32 - killer_level as i32;
    let multiplier = match level_diff {
        d if d >= 5 => 2.0,
        d if d >= 2 => 1.5,
        d if d >= 0 => 1.0,
        d if d >= -2 => 0.75,
        d if d >= -5 => 0.5,
        _ => 0.1,
    };
    (base_exp as f64 * multiplier) as u64
}

/// Pending combat action to be resolved during the tick.
#[derive(Debug, Clone)]
pub struct CombatAction {
    pub attacker_id: EntityId,
    pub target_id: EntityId,
}

/// Combat log entry for events that occurred during a tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatLogEntry {
    pub tick: u64,
    pub attacker_id: EntityId,
    pub target_id: EntityId,
    pub damage: f32,
    pub is_critical: bool,
    pub is_kill: bool,
}

/// Combat resolver processes all combat actions for a tick.
#[derive(Debug, Default)]
pub struct CombatResolver {
    /// Pending combat actions for this tick.
    pending_actions: Vec<CombatAction>,
    /// Combat log entries from the last tick.
    pub log: Vec<CombatLogEntry>,
}

impl CombatResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a combat action.
    pub fn queue_attack(&mut self, attacker_id: EntityId, target_id: EntityId) {
        self.pending_actions.push(CombatAction {
            attacker_id,
            target_id,
        });
    }

    /// Resolve all pending combat actions for this tick.
    /// Returns a map of entity_id -> (damage_to_apply, Vec<CombatLogEntry>).
    pub fn resolve(
        &mut self,
        tick: u64,
        stats: &HashMap<EntityId, CombatStats>,
    ) -> Vec<(EntityId, f32, CombatLogEntry)> {
        self.log.clear();
        let mut results = Vec::new();

        for action in &self.pending_actions {
            let attacker_stats = match stats.get(&action.attacker_id) {
                Some(s) if s.can_attack() => s,
                _ => continue,
            };
            let defender_stats = stats.get(&action.target_id).cloned().unwrap_or_default();

            let damage = calculate_damage(
                attacker_stats,
                &defender_stats,
                tick,
                action.attacker_id,
            );

            let entry = CombatLogEntry {
                tick,
                attacker_id: action.attacker_id,
                target_id: action.target_id,
                damage: damage.final_damage,
                is_critical: damage.is_critical,
                is_kill: false, // Updated after applying
            };

            self.log.push(entry.clone());
            results.push((action.target_id, damage.final_damage, entry));
        }

        self.pending_actions.clear();
        results
    }
}

/// Deterministic float in [0, 1) from tick and entity.
fn deterministic_float(tick: u64, entity_id: u64, index: u64) -> f32 {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(tick.to_le_bytes());
    hasher.update(entity_id.to_le_bytes());
    hasher.update(index.to_le_bytes());
    let result = hasher.finalize();
    let val = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
    val as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_calculation_basic() {
        let attacker = CombatStats {
            attack: 20.0,
            crit_chance: 0.0,
            crit_multiplier: 2.0,
            ..Default::default()
        };
        let defender = CombatStats {
            defense: 10.0,
            ..Default::default()
        };
        let result = calculate_damage(&attacker, &defender, 1, 1);
        // 20 - 10*0.5 = 15
        assert!((result.final_damage - 15.0).abs() < 0.01);
        assert!(!result.is_critical);
    }

    #[test]
    fn test_minimum_damage() {
        let attacker = CombatStats {
            attack: 1.0,
            crit_chance: 0.0,
            ..Default::default()
        };
        let defender = CombatStats {
            defense: 100.0,
            ..Default::default()
        };
        let result = calculate_damage(&attacker, &defender, 1, 1);
        assert!((result.final_damage - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_range_check() {
        let a = PositionComponent { x: 0.0, y: 0.0, z: 0.0 };
        let b = PositionComponent { x: 1.0, y: 0.0, z: 0.0 };
        assert!(is_in_range(&a, &b, 2.0));
        assert!(!is_in_range(&a, &b, 0.5));
    }

    #[test]
    fn test_cooldown() {
        let mut stats = CombatStats::player_default();
        assert!(stats.can_attack());
        stats.start_cooldown();
        assert!(!stats.can_attack());
        for _ in 0..stats.attack_cooldown {
            stats.tick_cooldown();
        }
        assert!(stats.can_attack());
    }

    #[test]
    fn test_exp_for_level() {
        assert_eq!(exp_for_level(1), 100);
        assert_eq!(exp_for_level(2), 400);
        assert_eq!(exp_for_level(10), 10000);
    }

    #[test]
    fn test_exp_gained_level_scaling() {
        let base = 100;
        // Same level
        assert_eq!(exp_gained(5, 5, base), 100);
        // Higher victim
        assert_eq!(exp_gained(5, 7, base), 150);
        // Lower victim
        assert_eq!(exp_gained(5, 3, base), 75);
        // Much lower victim
        assert_eq!(exp_gained(10, 1, base), 10);
    }

    #[test]
    fn test_combat_resolver() {
        let mut resolver = CombatResolver::new();
        let mut stats = HashMap::new();
        stats.insert(1, CombatStats::player_default());
        stats.insert(2, CombatStats::from_npc(5.0, 2.0, 10));

        resolver.queue_attack(1, 2);
        let results = resolver.resolve(1, &stats);
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.0); // Some damage dealt
    }

    #[test]
    fn test_combat_resolver_cooldown_blocks() {
        let mut resolver = CombatResolver::new();
        let mut stats = HashMap::new();
        let mut player = CombatStats::player_default();
        player.cooldown_remaining = 3; // On cooldown
        stats.insert(1, player);
        stats.insert(2, CombatStats::from_npc(5.0, 2.0, 10));

        resolver.queue_attack(1, 2);
        let results = resolver.resolve(1, &stats);
        assert_eq!(results.len(), 0); // No damage because on cooldown
    }

    #[test]
    fn test_deterministic_damage() {
        let attacker = CombatStats::player_default();
        let defender = CombatStats::from_npc(5.0, 2.0, 10);
        let r1 = calculate_damage(&attacker, &defender, 100, 1);
        let r2 = calculate_damage(&attacker, &defender, 100, 1);
        assert_eq!(r1.final_damage, r2.final_damage);
        assert_eq!(r1.is_critical, r2.is_critical);
    }
}
