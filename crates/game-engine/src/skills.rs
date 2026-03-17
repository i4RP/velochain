//! Skill system with skill trees, skill points, and passive/active abilities.
//!
//! Players earn skill points on level up and allocate them into
//! skill trees. Skills provide passive bonuses or unlock abilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique skill identifier.
pub type SkillId = u32;

/// Skill tree category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum SkillTree {
    /// Combat skills (melee, damage, defense).
    Combat,
    /// Gathering skills (mining, woodcutting, herbalism).
    Gathering,
    /// Crafting skills (smithing, alchemy, cooking).
    Crafting,
    /// Survival skills (health, stamina, movement).
    Survival,
}

/// Skill type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillType {
    /// Passive bonus (always active).
    Passive,
    /// Active ability (triggered by player).
    Active,
}

/// Effect that a skill provides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillEffect {
    /// Increase attack damage by flat amount.
    AttackBonus(f32),
    /// Increase defense by flat amount.
    DefenseBonus(f32),
    /// Increase max health by flat amount.
    HealthBonus(f32),
    /// Increase movement speed by multiplier (e.g. 0.1 = +10%).
    SpeedBonus(f32),
    /// Increase critical hit chance by flat amount.
    CritChanceBonus(f32),
    /// Increase critical hit damage multiplier.
    CritDamageBonus(f32),
    /// Increase gathering yield by multiplier.
    GatherBonus(f32),
    /// Reduce crafting material cost by percentage (0.0-1.0).
    CraftEfficiency(f32),
    /// Increase item drop rate by multiplier.
    DropRateBonus(f32),
    /// Health regeneration per tick.
    HealthRegen(f32),
}

/// Skill definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub tree: SkillTree,
    pub skill_type: SkillType,
    /// Maximum level for this skill.
    pub max_level: u32,
    /// Skill point cost per level.
    pub cost_per_level: u32,
    /// Required player level to unlock.
    pub required_level: u32,
    /// Prerequisite skill IDs (and minimum level).
    pub prerequisites: Vec<(SkillId, u32)>,
    /// Effect per level.
    pub effect_per_level: SkillEffect,
}

/// Player's skill state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerSkillState {
    /// Allocated skill levels: skill_id -> current_level.
    pub skills: HashMap<SkillId, u32>,
    /// Available skill points.
    pub available_points: u32,
    /// Total skill points ever earned.
    pub total_points_earned: u32,
}

impl PlayerSkillState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current level of a skill.
    pub fn skill_level(&self, skill_id: SkillId) -> u32 {
        self.skills.get(&skill_id).copied().unwrap_or(0)
    }

    /// Total allocated points.
    pub fn total_allocated(&self) -> u32 {
        self.skills.values().sum()
    }
}

/// Skill points awarded per level up.
pub fn skill_points_per_level(player_level: u32) -> u32 {
    match player_level {
        1..=5 => 1,
        6..=10 => 2,
        11..=20 => 3,
        _ => 4,
    }
}

/// Result of a skill operation.
#[derive(Debug, Clone)]
pub enum SkillResult {
    /// Skill leveled up.
    LeveledUp { skill_id: SkillId, new_level: u32 },
    /// Skill points awarded.
    PointsAwarded(u32),
    /// Error.
    Error(SkillError),
}

/// Skill error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillError {
    /// Skill not found.
    NotFound(SkillId),
    /// Not enough skill points.
    NotEnoughPoints { required: u32, available: u32 },
    /// Skill already at max level.
    MaxLevel(SkillId),
    /// Player level too low.
    LevelTooLow { required: u32, current: u32 },
    /// Prerequisites not met.
    PrerequisitesNotMet(Vec<(SkillId, u32)>),
}

/// Computed stat bonuses from all skills.
#[derive(Debug, Clone, Default)]
pub struct SkillBonuses {
    pub attack_bonus: f32,
    pub defense_bonus: f32,
    pub health_bonus: f32,
    pub speed_bonus: f32,
    pub crit_chance_bonus: f32,
    pub crit_damage_bonus: f32,
    pub gather_bonus: f32,
    pub craft_efficiency: f32,
    pub drop_rate_bonus: f32,
    pub health_regen: f32,
}

/// Skill registry holding all skill definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRegistry {
    skills: HashMap<SkillId, SkillDef>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Create the default skill registry.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // Combat tree
        registry.register(SkillDef {
            id: 1,
            name: "Strength".into(),
            description: "Increases attack damage.".into(),
            tree: SkillTree::Combat,
            skill_type: SkillType::Passive,
            max_level: 10,
            cost_per_level: 1,
            required_level: 1,
            prerequisites: vec![],
            effect_per_level: SkillEffect::AttackBonus(3.0),
        });
        registry.register(SkillDef {
            id: 2,
            name: "Toughness".into(),
            description: "Increases defense.".into(),
            tree: SkillTree::Combat,
            skill_type: SkillType::Passive,
            max_level: 10,
            cost_per_level: 1,
            required_level: 1,
            prerequisites: vec![],
            effect_per_level: SkillEffect::DefenseBonus(2.0),
        });
        registry.register(SkillDef {
            id: 3,
            name: "Precision".into(),
            description: "Increases critical hit chance.".into(),
            tree: SkillTree::Combat,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 2,
            required_level: 3,
            prerequisites: vec![(1, 3)],
            effect_per_level: SkillEffect::CritChanceBonus(0.03),
        });
        registry.register(SkillDef {
            id: 4,
            name: "Devastation".into(),
            description: "Increases critical hit damage.".into(),
            tree: SkillTree::Combat,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 2,
            required_level: 5,
            prerequisites: vec![(3, 2)],
            effect_per_level: SkillEffect::CritDamageBonus(0.2),
        });

        // Gathering tree
        registry.register(SkillDef {
            id: 10,
            name: "Prospector".into(),
            description: "Increases mining yield.".into(),
            tree: SkillTree::Gathering,
            skill_type: SkillType::Passive,
            max_level: 10,
            cost_per_level: 1,
            required_level: 1,
            prerequisites: vec![],
            effect_per_level: SkillEffect::GatherBonus(0.1),
        });
        registry.register(SkillDef {
            id: 11,
            name: "Lucky Find".into(),
            description: "Increases rare item drop rate.".into(),
            tree: SkillTree::Gathering,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 2,
            required_level: 5,
            prerequisites: vec![(10, 3)],
            effect_per_level: SkillEffect::DropRateBonus(0.1),
        });

        // Crafting tree
        registry.register(SkillDef {
            id: 20,
            name: "Efficient Crafting".into(),
            description: "Reduces crafting material cost.".into(),
            tree: SkillTree::Crafting,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 2,
            required_level: 2,
            prerequisites: vec![],
            effect_per_level: SkillEffect::CraftEfficiency(0.05),
        });

        // Survival tree
        registry.register(SkillDef {
            id: 30,
            name: "Vitality".into(),
            description: "Increases maximum health.".into(),
            tree: SkillTree::Survival,
            skill_type: SkillType::Passive,
            max_level: 10,
            cost_per_level: 1,
            required_level: 1,
            prerequisites: vec![],
            effect_per_level: SkillEffect::HealthBonus(10.0),
        });
        registry.register(SkillDef {
            id: 31,
            name: "Regeneration".into(),
            description: "Slowly regenerate health over time.".into(),
            tree: SkillTree::Survival,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 2,
            required_level: 3,
            prerequisites: vec![(30, 3)],
            effect_per_level: SkillEffect::HealthRegen(0.5),
        });
        registry.register(SkillDef {
            id: 32,
            name: "Swiftness".into(),
            description: "Increases movement speed.".into(),
            tree: SkillTree::Survival,
            skill_type: SkillType::Passive,
            max_level: 5,
            cost_per_level: 1,
            required_level: 2,
            prerequisites: vec![],
            effect_per_level: SkillEffect::SpeedBonus(0.05),
        });

        registry
    }

    /// Register a skill definition.
    pub fn register(&mut self, def: SkillDef) {
        self.skills.insert(def.id, def);
    }

    /// Get a skill by ID.
    pub fn get(&self, id: SkillId) -> Option<&SkillDef> {
        self.skills.get(&id)
    }

    /// Get all skills.
    pub fn all_skills(&self) -> impl Iterator<Item = &SkillDef> {
        self.skills.values()
    }

    /// Get skills in a specific tree.
    pub fn skills_in_tree(&self, tree: SkillTree) -> Vec<&SkillDef> {
        self.skills.values().filter(|s| s.tree == tree).collect()
    }

    /// Skill count.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Award skill points for a level up.
    pub fn award_level_up_points(
        &self,
        player_level: u32,
        state: &mut PlayerSkillState,
    ) -> SkillResult {
        let points = skill_points_per_level(player_level);
        state.available_points += points;
        state.total_points_earned += points;
        SkillResult::PointsAwarded(points)
    }

    /// Allocate a skill point.
    pub fn allocate_point(
        &self,
        skill_id: SkillId,
        player_level: u32,
        state: &mut PlayerSkillState,
    ) -> SkillResult {
        let skill = match self.get(skill_id) {
            Some(s) => s,
            None => return SkillResult::Error(SkillError::NotFound(skill_id)),
        };

        let current_level = state.skill_level(skill_id);
        if current_level >= skill.max_level {
            return SkillResult::Error(SkillError::MaxLevel(skill_id));
        }

        if player_level < skill.required_level {
            return SkillResult::Error(SkillError::LevelTooLow {
                required: skill.required_level,
                current: player_level,
            });
        }

        // Check prerequisites
        let missing: Vec<(SkillId, u32)> = skill
            .prerequisites
            .iter()
            .filter(|(prereq_id, req_level)| state.skill_level(*prereq_id) < *req_level)
            .copied()
            .collect();
        if !missing.is_empty() {
            return SkillResult::Error(SkillError::PrerequisitesNotMet(missing));
        }

        if state.available_points < skill.cost_per_level {
            return SkillResult::Error(SkillError::NotEnoughPoints {
                required: skill.cost_per_level,
                available: state.available_points,
            });
        }

        state.available_points -= skill.cost_per_level;
        *state.skills.entry(skill_id).or_insert(0) += 1;
        let new_level = state.skill_level(skill_id);

        SkillResult::LeveledUp {
            skill_id,
            new_level,
        }
    }

    /// Compute total bonuses from all allocated skills.
    pub fn compute_bonuses(&self, state: &PlayerSkillState) -> SkillBonuses {
        let mut bonuses = SkillBonuses::default();

        for (&skill_id, &level) in &state.skills {
            if level == 0 {
                continue;
            }
            if let Some(skill) = self.get(skill_id) {
                let mult = level as f32;
                match &skill.effect_per_level {
                    SkillEffect::AttackBonus(v) => bonuses.attack_bonus += v * mult,
                    SkillEffect::DefenseBonus(v) => bonuses.defense_bonus += v * mult,
                    SkillEffect::HealthBonus(v) => bonuses.health_bonus += v * mult,
                    SkillEffect::SpeedBonus(v) => bonuses.speed_bonus += v * mult,
                    SkillEffect::CritChanceBonus(v) => bonuses.crit_chance_bonus += v * mult,
                    SkillEffect::CritDamageBonus(v) => bonuses.crit_damage_bonus += v * mult,
                    SkillEffect::GatherBonus(v) => bonuses.gather_bonus += v * mult,
                    SkillEffect::CraftEfficiency(v) => bonuses.craft_efficiency += v * mult,
                    SkillEffect::DropRateBonus(v) => bonuses.drop_rate_bonus += v * mult,
                    SkillEffect::HealthRegen(v) => bonuses.health_regen += v * mult,
                }
            }
        }

        bonuses
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_skills() {
        let registry = SkillRegistry::default();
        assert!(registry.skill_count() > 0);
        assert!(registry.get(1).is_some()); // Strength
    }

    #[test]
    fn test_allocate_skill_point() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 5;

        let result = registry.allocate_point(1, 1, &mut state); // Strength
        assert!(matches!(
            result,
            SkillResult::LeveledUp {
                skill_id: 1,
                new_level: 1
            }
        ));
        assert_eq!(state.available_points, 4);
        assert_eq!(state.skill_level(1), 1);
    }

    #[test]
    fn test_not_enough_points() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 0;

        let result = registry.allocate_point(1, 1, &mut state);
        assert!(matches!(
            result,
            SkillResult::Error(SkillError::NotEnoughPoints { .. })
        ));
    }

    #[test]
    fn test_max_level() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 100;
        state.skills.insert(1, 10); // Strength at max (10)

        let result = registry.allocate_point(1, 1, &mut state);
        assert!(matches!(
            result,
            SkillResult::Error(SkillError::MaxLevel(1))
        ));
    }

    #[test]
    fn test_prerequisites() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 10;

        // Precision requires Strength level 3
        let result = registry.allocate_point(3, 5, &mut state);
        assert!(matches!(
            result,
            SkillResult::Error(SkillError::PrerequisitesNotMet(_))
        ));

        // Level up Strength to 3
        state.skills.insert(1, 3);
        let result = registry.allocate_point(3, 5, &mut state);
        assert!(matches!(result, SkillResult::LeveledUp { .. }));
    }

    #[test]
    fn test_level_too_low() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 10;

        // Precision requires player level 3
        state.skills.insert(1, 3);
        let result = registry.allocate_point(3, 1, &mut state);
        assert!(matches!(
            result,
            SkillResult::Error(SkillError::LevelTooLow { .. })
        ));
    }

    #[test]
    fn test_award_level_up_points() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();

        registry.award_level_up_points(1, &mut state);
        assert_eq!(state.available_points, 1);

        registry.award_level_up_points(7, &mut state);
        assert_eq!(state.available_points, 3); // 1 + 2
    }

    #[test]
    fn test_compute_bonuses() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.skills.insert(1, 5); // Strength level 5: +15 attack
        state.skills.insert(30, 3); // Vitality level 3: +30 health

        let bonuses = registry.compute_bonuses(&state);
        assert!((bonuses.attack_bonus - 15.0).abs() < 0.01);
        assert!((bonuses.health_bonus - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_skills_in_tree() {
        let registry = SkillRegistry::default();
        let combat = registry.skills_in_tree(SkillTree::Combat);
        assert!(!combat.is_empty());
        for skill in combat {
            assert_eq!(skill.tree, SkillTree::Combat);
        }
    }

    #[test]
    fn test_skill_points_per_level() {
        assert_eq!(skill_points_per_level(1), 1);
        assert_eq!(skill_points_per_level(5), 1);
        assert_eq!(skill_points_per_level(6), 2);
        assert_eq!(skill_points_per_level(15), 3);
        assert_eq!(skill_points_per_level(25), 4);
    }

    #[test]
    fn test_cost_per_level_respected() {
        let registry = SkillRegistry::default();
        let mut state = PlayerSkillState::new();
        state.available_points = 2;
        state.skills.insert(1, 3); // Prerequisite for Precision

        // Precision costs 2 points per level
        let result = registry.allocate_point(3, 5, &mut state);
        assert!(matches!(result, SkillResult::LeveledUp { .. }));
        assert_eq!(state.available_points, 0);
    }
}
