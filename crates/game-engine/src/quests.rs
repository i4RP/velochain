//! Quest system with quest definitions, progress tracking, and rewards.
//!
//! Quests are defined with objectives (kill, gather, explore) and
//! reward items/experience on completion. All progress is deterministic.

use crate::items::ItemDefId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique quest identifier.
pub type QuestId = u32;

/// Quest objective types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QuestObjective {
    /// Kill N entities of a specific type.
    Kill { npc_type: String, count: u32 },
    /// Gather N items of a specific type.
    Gather { item_id: ItemDefId, count: u32 },
    /// Reach a specific location (within radius).
    Explore { x: f32, y: f32, radius: f32 },
    /// Deliver items to an NPC.
    Deliver {
        item_id: ItemDefId,
        count: u32,
        npc_type: String,
    },
    /// Talk to a specific NPC type.
    TalkTo { npc_type: String },
    /// Craft a specific item.
    Craft { item_id: ItemDefId, count: u32 },
}

/// Quest reward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestReward {
    /// Experience reward.
    pub experience: u64,
    /// Item rewards: (item_id, quantity).
    pub items: Vec<(ItemDefId, u32)>,
    /// Gold reward.
    pub gold: u64,
}

/// Quest definition (template).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDef {
    pub id: QuestId,
    pub name: String,
    pub description: String,
    /// Objectives to complete.
    pub objectives: Vec<QuestObjective>,
    /// Rewards on completion.
    pub reward: QuestReward,
    /// Required player level to accept.
    pub required_level: u32,
    /// Prerequisites: quest IDs that must be completed first.
    pub prerequisites: Vec<QuestId>,
    /// Whether this quest can be repeated.
    pub repeatable: bool,
}

/// Progress on a single objective.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveProgress {
    /// Current progress count.
    pub current: u32,
    /// Required count.
    pub required: u32,
    /// Whether this objective is complete.
    pub complete: bool,
}

/// Player's active quest state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveQuest {
    pub quest_id: QuestId,
    /// Progress for each objective (same order as definition).
    pub objectives: Vec<ObjectiveProgress>,
    /// Tick when quest was accepted.
    pub accepted_tick: u64,
}

impl ActiveQuest {
    /// Check if all objectives are complete.
    pub fn is_complete(&self) -> bool {
        self.objectives.iter().all(|o| o.complete)
    }
}

/// Quest state for a player.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerQuestState {
    /// Currently active quests.
    pub active: Vec<ActiveQuest>,
    /// Completed quest IDs.
    pub completed: Vec<QuestId>,
    /// Maximum concurrent active quests.
    pub max_active: usize,
}

impl PlayerQuestState {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            completed: Vec::new(),
            max_active: 5,
        }
    }

    /// Check if a quest is already active.
    pub fn is_active(&self, quest_id: QuestId) -> bool {
        self.active.iter().any(|q| q.quest_id == quest_id)
    }

    /// Check if a quest is completed.
    pub fn is_completed(&self, quest_id: QuestId) -> bool {
        self.completed.contains(&quest_id)
    }
}

/// Result of a quest operation.
#[derive(Debug, Clone)]
pub enum QuestResult {
    /// Quest accepted.
    Accepted(QuestId),
    /// Quest objective updated.
    ProgressUpdated {
        quest_id: QuestId,
        objective_index: usize,
        current: u32,
        required: u32,
    },
    /// Quest completed.
    Completed {
        quest_id: QuestId,
        reward: QuestReward,
    },
    /// Error.
    Error(QuestError),
}

/// Quest error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestError {
    /// Quest not found.
    NotFound(QuestId),
    /// Player level too low.
    LevelTooLow { required: u32, current: u32 },
    /// Prerequisites not met.
    PrerequisitesNotMet(Vec<QuestId>),
    /// Already active.
    AlreadyActive(QuestId),
    /// Already completed (non-repeatable).
    AlreadyCompleted(QuestId),
    /// Too many active quests.
    TooManyActive,
    /// Quest not active (can't update progress).
    NotActive(QuestId),
}

/// Quest registry holding all quest definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestRegistry {
    quests: HashMap<QuestId, QuestDef>,
}

impl QuestRegistry {
    pub fn new() -> Self {
        Self {
            quests: HashMap::new(),
        }
    }

    /// Create the default quest registry with built-in quests.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // Tutorial quests
        registry.register(QuestDef {
            id: 1,
            name: "First Steps".into(),
            description: "Explore the world around the spawn area.".into(),
            objectives: vec![QuestObjective::Explore {
                x: 50.0,
                y: 50.0,
                radius: 20.0,
            }],
            reward: QuestReward {
                experience: 50,
                items: vec![(1, 1)],
                gold: 10,
            },
            required_level: 1,
            prerequisites: vec![],
            repeatable: false,
        });
        registry.register(QuestDef {
            id: 2,
            name: "Gathering Wood".into(),
            description: "Collect wood for the village.".into(),
            objectives: vec![QuestObjective::Gather {
                item_id: 31,
                count: 5,
            }],
            reward: QuestReward {
                experience: 80,
                items: vec![(20, 2)],
                gold: 20,
            },
            required_level: 1,
            prerequisites: vec![],
            repeatable: true,
        });
        registry.register(QuestDef {
            id: 3,
            name: "Wolf Menace".into(),
            description: "Wolves have been threatening the village. Defeat 3 wolves.".into(),
            objectives: vec![QuestObjective::Kill {
                npc_type: "wolf".into(),
                count: 3,
            }],
            reward: QuestReward {
                experience: 150,
                items: vec![(2, 1)],
                gold: 50,
            },
            required_level: 2,
            prerequisites: vec![1],
            repeatable: false,
        });

        // Mid-level quests
        registry.register(QuestDef {
            id: 10,
            name: "Iron Supply".into(),
            description: "Gather iron ore for the blacksmith.".into(),
            objectives: vec![QuestObjective::Gather {
                item_id: 30,
                count: 10,
            }],
            reward: QuestReward {
                experience: 200,
                items: vec![(11, 1)],
                gold: 100,
            },
            required_level: 3,
            prerequisites: vec![],
            repeatable: true,
        });
        registry.register(QuestDef {
            id: 11,
            name: "Merchant's Request".into(),
            description: "Talk to the merchant and deliver 5 wood.".into(),
            objectives: vec![
                QuestObjective::TalkTo {
                    npc_type: "merchant".into(),
                },
                QuestObjective::Deliver {
                    item_id: 31,
                    count: 5,
                    npc_type: "merchant".into(),
                },
            ],
            reward: QuestReward {
                experience: 120,
                items: vec![(20, 3)],
                gold: 30,
            },
            required_level: 2,
            prerequisites: vec![],
            repeatable: false,
        });
        registry.register(QuestDef {
            id: 12,
            name: "Smith's Challenge".into(),
            description: "Craft an Iron Sword to prove your skill.".into(),
            objectives: vec![QuestObjective::Craft {
                item_id: 2,
                count: 1,
            }],
            reward: QuestReward {
                experience: 250,
                items: vec![(30, 10)],
                gold: 75,
            },
            required_level: 3,
            prerequisites: vec![10],
            repeatable: false,
        });

        // High-level quests
        registry.register(QuestDef {
            id: 20,
            name: "Dragon Hunt".into(),
            description: "Slay the fearsome dragon that terrorizes the land.".into(),
            objectives: vec![QuestObjective::Kill {
                npc_type: "dragon".into(),
                count: 1,
            }],
            reward: QuestReward {
                experience: 1000,
                items: vec![(32, 3), (21, 5)],
                gold: 500,
            },
            required_level: 10,
            prerequisites: vec![3],
            repeatable: false,
        });
        registry.register(QuestDef {
            id: 21,
            name: "Skeleton Purge".into(),
            description: "Clear out the skeleton infestation. Defeat 10 skeletons.".into(),
            objectives: vec![QuestObjective::Kill {
                npc_type: "skeleton".into(),
                count: 10,
            }],
            reward: QuestReward {
                experience: 500,
                items: vec![(3, 1)],
                gold: 200,
            },
            required_level: 5,
            prerequisites: vec![3],
            repeatable: true,
        });
        registry.register(QuestDef {
            id: 22,
            name: "Master Crafter".into(),
            description: "Forge the legendary Dragon Slayer sword.".into(),
            objectives: vec![QuestObjective::Craft {
                item_id: 4,
                count: 1,
            }],
            reward: QuestReward {
                experience: 2000,
                items: vec![],
                gold: 1000,
            },
            required_level: 15,
            prerequisites: vec![12, 20],
            repeatable: false,
        });

        registry
    }

    /// Register a quest definition.
    pub fn register(&mut self, def: QuestDef) {
        self.quests.insert(def.id, def);
    }

    /// Get a quest by ID.
    pub fn get(&self, id: QuestId) -> Option<&QuestDef> {
        self.quests.get(&id)
    }

    /// Get all quests.
    pub fn all_quests(&self) -> impl Iterator<Item = &QuestDef> {
        self.quests.values()
    }

    /// Get quests available to a player.
    pub fn available_quests(
        &self,
        player_level: u32,
        completed: &[QuestId],
        active: &[QuestId],
    ) -> Vec<&QuestDef> {
        self.quests
            .values()
            .filter(|q| {
                q.required_level <= player_level
                    && !active.contains(&q.id)
                    && (q.repeatable || !completed.contains(&q.id))
                    && q.prerequisites.iter().all(|p| completed.contains(p))
            })
            .collect()
    }

    /// Quest count.
    pub fn quest_count(&self) -> usize {
        self.quests.len()
    }

    /// Accept a quest for a player.
    pub fn accept_quest(
        &self,
        quest_id: QuestId,
        player_level: u32,
        state: &mut PlayerQuestState,
        current_tick: u64,
    ) -> QuestResult {
        let quest = match self.get(quest_id) {
            Some(q) => q,
            None => return QuestResult::Error(QuestError::NotFound(quest_id)),
        };

        if player_level < quest.required_level {
            return QuestResult::Error(QuestError::LevelTooLow {
                required: quest.required_level,
                current: player_level,
            });
        }

        let missing_prereqs: Vec<QuestId> = quest
            .prerequisites
            .iter()
            .filter(|p| !state.is_completed(**p))
            .copied()
            .collect();
        if !missing_prereqs.is_empty() {
            return QuestResult::Error(QuestError::PrerequisitesNotMet(missing_prereqs));
        }

        if state.is_active(quest_id) {
            return QuestResult::Error(QuestError::AlreadyActive(quest_id));
        }

        if !quest.repeatable && state.is_completed(quest_id) {
            return QuestResult::Error(QuestError::AlreadyCompleted(quest_id));
        }

        if state.active.len() >= state.max_active {
            return QuestResult::Error(QuestError::TooManyActive);
        }

        let objectives = quest
            .objectives
            .iter()
            .map(|obj| {
                let required = match obj {
                    QuestObjective::Kill { count, .. } => *count,
                    QuestObjective::Gather { count, .. } => *count,
                    QuestObjective::Explore { .. } => 1,
                    QuestObjective::Deliver { count, .. } => *count,
                    QuestObjective::TalkTo { .. } => 1,
                    QuestObjective::Craft { count, .. } => *count,
                };
                ObjectiveProgress {
                    current: 0,
                    required,
                    complete: false,
                }
            })
            .collect();

        state.active.push(ActiveQuest {
            quest_id,
            objectives,
            accepted_tick: current_tick,
        });

        QuestResult::Accepted(quest_id)
    }

    /// Update quest progress for a kill event.
    pub fn on_kill(&self, npc_type: &str, state: &mut PlayerQuestState) -> Vec<QuestResult> {
        let mut results = Vec::new();
        for active in &mut state.active {
            let quest = match self.get(active.quest_id) {
                Some(q) => q,
                None => continue,
            };
            for (i, obj) in quest.objectives.iter().enumerate() {
                if let QuestObjective::Kill { npc_type: qt, .. } = obj {
                    if qt == npc_type && !active.objectives[i].complete {
                        active.objectives[i].current += 1;
                        if active.objectives[i].current >= active.objectives[i].required {
                            active.objectives[i].complete = true;
                        }
                        results.push(QuestResult::ProgressUpdated {
                            quest_id: active.quest_id,
                            objective_index: i,
                            current: active.objectives[i].current,
                            required: active.objectives[i].required,
                        });
                    }
                }
            }
        }
        results
    }

    /// Update quest progress for a gather event.
    pub fn on_gather(
        &self,
        item_id: ItemDefId,
        quantity: u32,
        state: &mut PlayerQuestState,
    ) -> Vec<QuestResult> {
        let mut results = Vec::new();
        for active in &mut state.active {
            let quest = match self.get(active.quest_id) {
                Some(q) => q,
                None => continue,
            };
            for (i, obj) in quest.objectives.iter().enumerate() {
                if let QuestObjective::Gather { item_id: qi, .. } = obj {
                    if *qi == item_id && !active.objectives[i].complete {
                        active.objectives[i].current += quantity;
                        if active.objectives[i].current >= active.objectives[i].required {
                            active.objectives[i].current = active.objectives[i].required;
                            active.objectives[i].complete = true;
                        }
                        results.push(QuestResult::ProgressUpdated {
                            quest_id: active.quest_id,
                            objective_index: i,
                            current: active.objectives[i].current,
                            required: active.objectives[i].required,
                        });
                    }
                }
            }
        }
        results
    }

    /// Update quest progress for a craft event.
    pub fn on_craft(
        &self,
        item_id: ItemDefId,
        quantity: u32,
        state: &mut PlayerQuestState,
    ) -> Vec<QuestResult> {
        let mut results = Vec::new();
        for active in &mut state.active {
            let quest = match self.get(active.quest_id) {
                Some(q) => q,
                None => continue,
            };
            for (i, obj) in quest.objectives.iter().enumerate() {
                if let QuestObjective::Craft { item_id: qi, .. } = obj {
                    if *qi == item_id && !active.objectives[i].complete {
                        active.objectives[i].current += quantity;
                        if active.objectives[i].current >= active.objectives[i].required {
                            active.objectives[i].current = active.objectives[i].required;
                            active.objectives[i].complete = true;
                        }
                        results.push(QuestResult::ProgressUpdated {
                            quest_id: active.quest_id,
                            objective_index: i,
                            current: active.objectives[i].current,
                            required: active.objectives[i].required,
                        });
                    }
                }
            }
        }
        results
    }

    /// Check and complete any quests that have all objectives done.
    pub fn check_completions(&self, state: &mut PlayerQuestState) -> Vec<QuestResult> {
        let mut results = Vec::new();
        let mut completed_indices = Vec::new();

        for (idx, active) in state.active.iter().enumerate() {
            if active.is_complete() {
                if let Some(quest) = self.get(active.quest_id) {
                    results.push(QuestResult::Completed {
                        quest_id: active.quest_id,
                        reward: quest.reward.clone(),
                    });
                    completed_indices.push(idx);
                }
            }
        }

        // Remove completed quests (reverse order to maintain indices)
        for idx in completed_indices.into_iter().rev() {
            let quest_id = state.active[idx].quest_id;
            state.active.remove(idx);
            state.completed.push(quest_id);
        }

        results
    }
}

impl Default for QuestRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_quests() {
        let registry = QuestRegistry::default();
        assert!(registry.quest_count() > 0);
        assert!(registry.get(1).is_some());
    }

    #[test]
    fn test_accept_quest() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        let result = registry.accept_quest(1, 1, &mut state, 0);
        assert!(matches!(result, QuestResult::Accepted(1)));
        assert_eq!(state.active.len(), 1);
    }

    #[test]
    fn test_accept_quest_level_too_low() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        // Dragon Hunt requires level 10
        let result = registry.accept_quest(20, 1, &mut state, 0);
        assert!(matches!(
            result,
            QuestResult::Error(QuestError::LevelTooLow { .. })
        ));
    }

    #[test]
    fn test_accept_quest_prerequisites() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        // Wolf Menace requires quest 1 completed
        let result = registry.accept_quest(3, 5, &mut state, 0);
        assert!(matches!(
            result,
            QuestResult::Error(QuestError::PrerequisitesNotMet(_))
        ));

        // Complete quest 1 first
        state.completed.push(1);
        let result = registry.accept_quest(3, 5, &mut state, 0);
        assert!(matches!(result, QuestResult::Accepted(3)));
    }

    #[test]
    fn test_kill_progress() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        state.completed.push(1); // Prerequisite
        registry.accept_quest(3, 5, &mut state, 0); // Wolf Menace: kill 3 wolves

        let results = registry.on_kill("wolf", &mut state);
        assert_eq!(results.len(), 1);
        if let QuestResult::ProgressUpdated {
            current, required, ..
        } = &results[0]
        {
            assert_eq!(*current, 1);
            assert_eq!(*required, 3);
        }
    }

    #[test]
    fn test_quest_completion() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        state.completed.push(1);
        registry.accept_quest(3, 5, &mut state, 0);

        // Kill 3 wolves
        registry.on_kill("wolf", &mut state);
        registry.on_kill("wolf", &mut state);
        registry.on_kill("wolf", &mut state);

        let completions = registry.check_completions(&mut state);
        assert_eq!(completions.len(), 1);
        assert!(matches!(
            completions[0],
            QuestResult::Completed { quest_id: 3, .. }
        ));
        assert!(state.is_completed(3));
        assert!(state.active.is_empty());
    }

    #[test]
    fn test_gather_progress() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        registry.accept_quest(2, 1, &mut state, 0); // Gathering Wood: 5 wood

        let results = registry.on_gather(31, 3, &mut state);
        assert_eq!(results.len(), 1);
        if let QuestResult::ProgressUpdated { current, .. } = &results[0] {
            assert_eq!(*current, 3);
        }
    }

    #[test]
    fn test_craft_progress() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        state.completed.push(10);
        registry.accept_quest(12, 5, &mut state, 0); // Smith's Challenge: craft Iron Sword

        let results = registry.on_craft(2, 1, &mut state);
        assert_eq!(results.len(), 1);

        let completions = registry.check_completions(&mut state);
        assert_eq!(completions.len(), 1);
    }

    #[test]
    fn test_repeatable_quest() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();

        // Gathering Wood is repeatable
        registry.accept_quest(2, 1, &mut state, 0);
        registry.on_gather(31, 5, &mut state);
        registry.check_completions(&mut state);
        assert!(state.is_completed(2));

        // Can accept again
        let result = registry.accept_quest(2, 1, &mut state, 10);
        assert!(matches!(result, QuestResult::Accepted(2)));
    }

    #[test]
    fn test_non_repeatable_quest() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();

        // First Steps is non-repeatable; simulate completion
        state.completed.push(1);
        let result = registry.accept_quest(1, 1, &mut state, 0);
        assert!(matches!(
            result,
            QuestResult::Error(QuestError::AlreadyCompleted(1))
        ));
    }

    #[test]
    fn test_max_active_quests() {
        let registry = QuestRegistry::default();
        let mut state = PlayerQuestState::new();
        state.max_active = 2;

        registry.accept_quest(1, 1, &mut state, 0);
        registry.accept_quest(2, 1, &mut state, 0);
        let result = registry.accept_quest(10, 3, &mut state, 0);
        assert!(matches!(
            result,
            QuestResult::Error(QuestError::TooManyActive)
        ));
    }

    #[test]
    fn test_available_quests() {
        let registry = QuestRegistry::default();
        let completed = vec![1_u32];
        let active = vec![2_u32];

        let available = registry.available_quests(5, &completed, &active);
        // Should not include quest 1 (completed, non-repeatable) or quest 2 (active)
        assert!(available.iter().all(|q| q.id != 1));
        assert!(available.iter().all(|q| q.id != 2));
    }
}
