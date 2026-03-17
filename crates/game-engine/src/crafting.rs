//! Crafting system with recipes, material consumption, and item creation.
//!
//! All crafting is deterministic and processed as part of the game tick.
//! Recipes define required materials and the resulting item.

use crate::items::{ItemCategory, ItemDefId, ItemRarity, ItemRegistry, ItemStats, ItemDef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique recipe identifier.
pub type RecipeId = u32;

/// A single crafting recipe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRecipe {
    pub id: RecipeId,
    /// Display name.
    pub name: String,
    /// Required materials: (item_id, quantity).
    pub materials: Vec<(ItemDefId, u32)>,
    /// Result item ID.
    pub result_item_id: ItemDefId,
    /// Result quantity.
    pub result_quantity: u32,
    /// Required player level.
    pub required_level: u32,
    /// Crafting time in ticks (0 = instant).
    pub craft_ticks: u32,
    /// Category tag for UI filtering.
    pub category: CraftCategory,
}

/// Crafting category for grouping recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum CraftCategory {
    Weapon,
    Armor,
    Consumable,
    Material,
    Tool,
}

/// Result of attempting to craft.
#[derive(Debug, Clone)]
pub enum CraftResult {
    /// Crafting succeeded.
    Success {
        recipe_id: RecipeId,
        item_id: ItemDefId,
        quantity: u32,
    },
    /// Missing materials: list of (item_id, required, available).
    MissingMaterials(Vec<(ItemDefId, u32, u32)>),
    /// Player level too low.
    LevelTooLow { required: u32, current: u32 },
    /// Recipe not found.
    RecipeNotFound(RecipeId),
}

/// Crafting registry holding all recipes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftingRegistry {
    recipes: HashMap<RecipeId, CraftingRecipe>,
}

impl CraftingRegistry {
    /// Create a new empty crafting registry.
    pub fn new() -> Self {
        Self {
            recipes: HashMap::new(),
        }
    }

    /// Create the default crafting registry with built-in recipes.
    pub fn default_registry(item_registry: &ItemRegistry) -> Self {
        let mut registry = Self::new();
        let _ = item_registry; // Used for validation in production

        // Weapon recipes
        registry.register(CraftingRecipe {
            id: 1,
            name: "Craft Wooden Sword".into(),
            materials: vec![(31, 5)], // 5 Wood
            result_item_id: 1,        // Wooden Sword
            result_quantity: 1,
            required_level: 1,
            craft_ticks: 0,
            category: CraftCategory::Weapon,
        });
        registry.register(CraftingRecipe {
            id: 2,
            name: "Craft Iron Sword".into(),
            materials: vec![(30, 5), (31, 2)], // 5 Iron Ore + 2 Wood
            result_item_id: 2,                  // Iron Sword
            result_quantity: 1,
            required_level: 3,
            craft_ticks: 0,
            category: CraftCategory::Weapon,
        });
        registry.register(CraftingRecipe {
            id: 3,
            name: "Forge Steel Greatsword".into(),
            materials: vec![(30, 15), (31, 5)], // 15 Iron Ore + 5 Wood
            result_item_id: 3,                   // Steel Greatsword
            result_quantity: 1,
            required_level: 8,
            craft_ticks: 0,
            category: CraftCategory::Weapon,
        });
        registry.register(CraftingRecipe {
            id: 4,
            name: "Forge Dragon Slayer".into(),
            materials: vec![(30, 30), (32, 5), (31, 10)], // 30 Iron + 5 Dragon Scale + 10 Wood
            result_item_id: 4,                              // Dragon Slayer
            result_quantity: 1,
            required_level: 15,
            craft_ticks: 0,
            category: CraftCategory::Weapon,
        });

        // Armor recipes
        registry.register(CraftingRecipe {
            id: 10,
            name: "Craft Leather Helmet".into(),
            materials: vec![(31, 3)], // 3 Wood (placeholder for leather)
            result_item_id: 10,       // Leather Helmet
            result_quantity: 1,
            required_level: 1,
            craft_ticks: 0,
            category: CraftCategory::Armor,
        });
        registry.register(CraftingRecipe {
            id: 11,
            name: "Forge Iron Chestplate".into(),
            materials: vec![(30, 10), (31, 3)], // 10 Iron + 3 Wood
            result_item_id: 11,                  // Iron Chestplate
            result_quantity: 1,
            required_level: 5,
            craft_ticks: 0,
            category: CraftCategory::Armor,
        });
        registry.register(CraftingRecipe {
            id: 12,
            name: "Forge Steel Boots".into(),
            materials: vec![(30, 8), (31, 2)], // 8 Iron + 2 Wood
            result_item_id: 12,                 // Steel Boots
            result_quantity: 1,
            required_level: 4,
            craft_ticks: 0,
            category: CraftCategory::Armor,
        });

        // Consumable recipes
        registry.register(CraftingRecipe {
            id: 20,
            name: "Brew Health Potion".into(),
            materials: vec![(31, 2)], // 2 Wood (herbs placeholder)
            result_item_id: 20,       // Health Potion
            result_quantity: 2,
            required_level: 1,
            craft_ticks: 0,
            category: CraftCategory::Consumable,
        });
        registry.register(CraftingRecipe {
            id: 21,
            name: "Brew Greater Health Potion".into(),
            materials: vec![(20, 3), (32, 1)], // 3 Health Potions + 1 Dragon Scale
            result_item_id: 21,                 // Greater Health Potion
            result_quantity: 1,
            required_level: 10,
            craft_ticks: 0,
            category: CraftCategory::Consumable,
        });
        registry.register(CraftingRecipe {
            id: 22,
            name: "Cook Meat".into(),
            materials: vec![(31, 1)], // 1 Wood (fuel)
            result_item_id: 22,       // Cooked Meat
            result_quantity: 2,
            required_level: 1,
            craft_ticks: 0,
            category: CraftCategory::Consumable,
        });

        // Material processing
        registry.register(CraftingRecipe {
            id: 30,
            name: "Smelt Iron Ingot".into(),
            materials: vec![(30, 3), (31, 1)], // 3 Iron Ore + 1 Wood (fuel)
            result_item_id: 30,                 // Iron Ore (represents ingot, same ID)
            result_quantity: 2,
            required_level: 2,
            craft_ticks: 0,
            category: CraftCategory::Material,
        });

        registry
    }

    /// Register a recipe.
    pub fn register(&mut self, recipe: CraftingRecipe) {
        self.recipes.insert(recipe.id, recipe);
    }

    /// Get a recipe by ID.
    pub fn get(&self, id: RecipeId) -> Option<&CraftingRecipe> {
        self.recipes.get(&id)
    }

    /// Get all recipes.
    pub fn all_recipes(&self) -> impl Iterator<Item = &CraftingRecipe> {
        self.recipes.values()
    }

    /// Get recipes available at a given level.
    pub fn recipes_for_level(&self, level: u32) -> Vec<&CraftingRecipe> {
        self.recipes
            .values()
            .filter(|r| r.required_level <= level)
            .collect()
    }

    /// Get recipes by category.
    pub fn recipes_by_category(&self, category: CraftCategory) -> Vec<&CraftingRecipe> {
        self.recipes
            .values()
            .filter(|r| r.category == category)
            .collect()
    }

    /// Recipe count.
    pub fn recipe_count(&self) -> usize {
        self.recipes.len()
    }

    /// Check if a player can craft a recipe (checks materials and level).
    /// `inventory` is a map of item_id -> quantity the player has.
    pub fn can_craft(
        &self,
        recipe_id: RecipeId,
        player_level: u32,
        inventory: &HashMap<ItemDefId, u32>,
    ) -> CraftResult {
        let recipe = match self.get(recipe_id) {
            Some(r) => r,
            None => return CraftResult::RecipeNotFound(recipe_id),
        };

        if player_level < recipe.required_level {
            return CraftResult::LevelTooLow {
                required: recipe.required_level,
                current: player_level,
            };
        }

        let mut missing = Vec::new();
        for &(item_id, required_qty) in &recipe.materials {
            let available = inventory.get(&item_id).copied().unwrap_or(0);
            if available < required_qty {
                missing.push((item_id, required_qty, available));
            }
        }

        if !missing.is_empty() {
            return CraftResult::MissingMaterials(missing);
        }

        CraftResult::Success {
            recipe_id,
            item_id: recipe.result_item_id,
            quantity: recipe.result_quantity,
        }
    }

    /// Execute crafting: consume materials and return the result.
    /// Returns (consumed materials, produced item_id, produced quantity).
    pub fn execute_craft(
        &self,
        recipe_id: RecipeId,
        player_level: u32,
        inventory: &mut HashMap<ItemDefId, u32>,
    ) -> CraftResult {
        let result = self.can_craft(recipe_id, player_level, inventory);
        if let CraftResult::Success { recipe_id: rid, item_id, quantity } = &result {
            let recipe = self.get(*rid).unwrap();
            // Consume materials
            for &(mat_id, mat_qty) in &recipe.materials {
                let entry = inventory.get_mut(&mat_id).unwrap();
                *entry -= mat_qty;
                if *entry == 0 {
                    inventory.remove(&mat_id);
                }
            }
            // Add result
            *inventory.entry(*item_id).or_insert(0) += quantity;
        }
        result
    }
}

impl Default for CraftingRegistry {
    fn default() -> Self {
        Self::default_registry(&ItemRegistry::default_registry())
    }
}

/// Register additional crafting items in the item registry.
/// Call this to add items that are crafting-specific but not in the base registry.
pub fn register_crafting_items(registry: &mut ItemRegistry) {
    // Refined materials
    registry.register(ItemDef {
        id: 33,
        name: "Iron Ingot".into(),
        description: "A refined iron bar, ready for smithing.".into(),
        category: ItemCategory::Material,
        rarity: ItemRarity::Common,
        max_stack: 50,
        stats: ItemStats::default(),
        equip_slot: None,
        value: 8,
    });
    registry.register(ItemDef {
        id: 34,
        name: "Leather".into(),
        description: "Tanned animal hide.".into(),
        category: ItemCategory::Material,
        rarity: ItemRarity::Common,
        max_stack: 50,
        stats: ItemStats::default(),
        equip_slot: None,
        value: 5,
    });
    registry.register(ItemDef {
        id: 35,
        name: "Herb".into(),
        description: "A medicinal herb used in potion brewing.".into(),
        category: ItemCategory::Material,
        rarity: ItemRarity::Common,
        max_stack: 50,
        stats: ItemStats::default(),
        equip_slot: None,
        value: 3,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_recipes() {
        let registry = CraftingRegistry::default();
        assert!(registry.recipe_count() > 0);
        assert!(registry.get(1).is_some()); // Craft Wooden Sword
    }

    #[test]
    fn test_craft_wooden_sword_success() {
        let registry = CraftingRegistry::default();
        let mut inventory: HashMap<ItemDefId, u32> = HashMap::new();
        inventory.insert(31, 10); // 10 Wood

        let result = registry.can_craft(1, 1, &inventory);
        assert!(matches!(result, CraftResult::Success { .. }));
    }

    #[test]
    fn test_craft_missing_materials() {
        let registry = CraftingRegistry::default();
        let inventory: HashMap<ItemDefId, u32> = HashMap::new(); // Empty

        let result = registry.can_craft(1, 1, &inventory);
        assert!(matches!(result, CraftResult::MissingMaterials(_)));
    }

    #[test]
    fn test_craft_level_too_low() {
        let registry = CraftingRegistry::default();
        let mut inventory: HashMap<ItemDefId, u32> = HashMap::new();
        inventory.insert(30, 50);
        inventory.insert(31, 50);

        // Iron Sword requires level 3
        let result = registry.can_craft(2, 1, &inventory);
        assert!(matches!(result, CraftResult::LevelTooLow { required: 3, current: 1 }));
    }

    #[test]
    fn test_craft_recipe_not_found() {
        let registry = CraftingRegistry::default();
        let inventory: HashMap<ItemDefId, u32> = HashMap::new();

        let result = registry.can_craft(999, 1, &inventory);
        assert!(matches!(result, CraftResult::RecipeNotFound(999)));
    }

    #[test]
    fn test_execute_craft_consumes_materials() {
        let registry = CraftingRegistry::default();
        let mut inventory: HashMap<ItemDefId, u32> = HashMap::new();
        inventory.insert(31, 10); // 10 Wood

        let result = registry.execute_craft(1, 1, &mut inventory);
        assert!(matches!(result, CraftResult::Success { .. }));
        assert_eq!(inventory.get(&31).copied().unwrap_or(0), 5); // 10 - 5 = 5 remaining
        assert_eq!(inventory.get(&1).copied().unwrap_or(0), 1); // 1 Wooden Sword
    }

    #[test]
    fn test_recipes_for_level() {
        let registry = CraftingRegistry::default();
        let level1 = registry.recipes_for_level(1);
        let level10 = registry.recipes_for_level(10);
        assert!(level10.len() >= level1.len());
    }

    #[test]
    fn test_recipes_by_category() {
        let registry = CraftingRegistry::default();
        let weapons = registry.recipes_by_category(CraftCategory::Weapon);
        assert!(!weapons.is_empty());
        for recipe in weapons {
            assert_eq!(recipe.category, CraftCategory::Weapon);
        }
    }

    #[test]
    fn test_double_craft_depletes_materials() {
        let registry = CraftingRegistry::default();
        let mut inventory: HashMap<ItemDefId, u32> = HashMap::new();
        inventory.insert(31, 10); // 10 Wood

        // First craft: success (uses 5 wood)
        let r1 = registry.execute_craft(1, 1, &mut inventory);
        assert!(matches!(r1, CraftResult::Success { .. }));

        // Second craft: success (uses remaining 5 wood)
        let r2 = registry.execute_craft(1, 1, &mut inventory);
        assert!(matches!(r2, CraftResult::Success { .. }));

        // Third craft: fail (no wood left)
        let r3 = registry.execute_craft(1, 1, &mut inventory);
        assert!(matches!(r3, CraftResult::MissingMaterials(_)));
    }

    #[test]
    fn test_register_crafting_items() {
        let mut item_reg = ItemRegistry::default_registry();
        let before = item_reg.item_count();
        register_crafting_items(&mut item_reg);
        assert!(item_reg.item_count() > before);
        assert!(item_reg.get(33).is_some()); // Iron Ingot
    }
}
