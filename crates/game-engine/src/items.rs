//! Item definitions, drop tables, and inventory management.
//!
//! All items are defined on-chain with deterministic properties.
//! Items can be dropped by NPCs, picked up by players, and traded.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique item definition ID.
pub type ItemDefId = u32;

/// Item rarity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl ItemRarity {
    /// Drop weight multiplier (lower = rarer).
    pub fn drop_weight(&self) -> u32 {
        match self {
            ItemRarity::Common => 100,
            ItemRarity::Uncommon => 40,
            ItemRarity::Rare => 15,
            ItemRarity::Epic => 5,
            ItemRarity::Legendary => 1,
        }
    }

    /// Color code for display.
    pub fn color_hex(&self) -> &'static str {
        match self {
            ItemRarity::Common => "#9d9d9d",
            ItemRarity::Uncommon => "#1eff00",
            ItemRarity::Rare => "#0070dd",
            ItemRarity::Epic => "#a335ee",
            ItemRarity::Legendary => "#ff8000",
        }
    }
}

/// Item category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ItemCategory {
    /// Weapons (swords, bows, staffs).
    Weapon,
    /// Armor (helmet, chest, legs, boots).
    Armor,
    /// Consumable (potions, food).
    Consumable,
    /// Material (crafting ingredients).
    Material,
    /// Quest item.
    Quest,
    /// Miscellaneous.
    Misc,
}

/// Equipment slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum EquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Boots,
}

/// Item definition (template).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemDefId,
    pub name: String,
    pub description: String,
    pub category: ItemCategory,
    pub rarity: ItemRarity,
    /// Maximum stack size (1 = not stackable).
    pub max_stack: u32,
    /// Base stats provided by this item.
    pub stats: ItemStats,
    /// Equipment slot (if equippable).
    pub equip_slot: Option<EquipSlot>,
    /// Value in base currency.
    pub value: u64,
}

/// Stats that an item can provide.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemStats {
    /// Attack damage bonus.
    pub attack: f32,
    /// Defense bonus.
    pub defense: f32,
    /// Health bonus.
    pub health_bonus: f32,
    /// Movement speed bonus (multiplier).
    pub speed_bonus: f32,
    /// Health restored on use (for consumables).
    pub heal_amount: f32,
}

/// A single drop table entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEntry {
    pub item_id: ItemDefId,
    /// Weight for random selection (higher = more likely).
    pub weight: u32,
    /// Minimum quantity.
    pub min_quantity: u32,
    /// Maximum quantity.
    pub max_quantity: u32,
}

/// Drop table for an NPC type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropTable {
    /// NPC type this table belongs to.
    pub npc_type: String,
    /// Possible drops.
    pub entries: Vec<DropEntry>,
    /// Chance of dropping nothing (0-100).
    pub nothing_weight: u32,
}

impl DropTable {
    /// Roll a drop from this table using a deterministic seed.
    pub fn roll(&self, seed: u64) -> Option<(ItemDefId, u32)> {
        let total_weight: u64 =
            self.nothing_weight as u64 + self.entries.iter().map(|e| e.weight as u64).sum::<u64>();
        if total_weight == 0 {
            return None;
        }

        let roll = deterministic_random(seed, 0) % total_weight;

        let mut cumulative = self.nothing_weight as u64;
        if roll < cumulative {
            return None; // No drop
        }

        for entry in &self.entries {
            cumulative += entry.weight as u64;
            if roll < cumulative {
                // Determine quantity
                let range = entry.max_quantity.saturating_sub(entry.min_quantity) + 1;
                let qty = if range <= 1 {
                    entry.min_quantity
                } else {
                    let qty_roll = deterministic_random(seed, 1) % range as u64;
                    entry.min_quantity + qty_roll as u32
                };
                return Some((entry.item_id, qty));
            }
        }

        None
    }
}

/// Item registry containing all item definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRegistry {
    items: HashMap<ItemDefId, ItemDef>,
    drop_tables: HashMap<String, DropTable>,
}

impl ItemRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            drop_tables: HashMap::new(),
        }
    }

    /// Create the default item registry with built-in items.
    pub fn default_registry() -> Self {
        let mut registry = Self::new();

        // Weapons
        registry.register(ItemDef {
            id: 1,
            name: "Wooden Sword".into(),
            description: "A simple wooden training sword.".into(),
            category: ItemCategory::Weapon,
            rarity: ItemRarity::Common,
            max_stack: 1,
            stats: ItemStats {
                attack: 5.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::MainHand),
            value: 10,
        });
        registry.register(ItemDef {
            id: 2,
            name: "Iron Sword".into(),
            description: "A sturdy iron sword.".into(),
            category: ItemCategory::Weapon,
            rarity: ItemRarity::Uncommon,
            max_stack: 1,
            stats: ItemStats {
                attack: 15.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::MainHand),
            value: 50,
        });
        registry.register(ItemDef {
            id: 3,
            name: "Steel Greatsword".into(),
            description: "A finely crafted steel greatsword.".into(),
            category: ItemCategory::Weapon,
            rarity: ItemRarity::Rare,
            max_stack: 1,
            stats: ItemStats {
                attack: 30.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::MainHand),
            value: 200,
        });
        registry.register(ItemDef {
            id: 4,
            name: "Dragon Slayer".into(),
            description: "A legendary blade forged in dragon fire.".into(),
            category: ItemCategory::Weapon,
            rarity: ItemRarity::Legendary,
            max_stack: 1,
            stats: ItemStats {
                attack: 80.0,
                health_bonus: 50.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::MainHand),
            value: 5000,
        });

        // Armor
        registry.register(ItemDef {
            id: 10,
            name: "Leather Helmet".into(),
            description: "Basic leather head protection.".into(),
            category: ItemCategory::Armor,
            rarity: ItemRarity::Common,
            max_stack: 1,
            stats: ItemStats {
                defense: 3.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::Head),
            value: 15,
        });
        registry.register(ItemDef {
            id: 11,
            name: "Iron Chestplate".into(),
            description: "Iron armor for the torso.".into(),
            category: ItemCategory::Armor,
            rarity: ItemRarity::Uncommon,
            max_stack: 1,
            stats: ItemStats {
                defense: 12.0,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::Chest),
            value: 80,
        });
        registry.register(ItemDef {
            id: 12,
            name: "Steel Boots".into(),
            description: "Heavy steel boots.".into(),
            category: ItemCategory::Armor,
            rarity: ItemRarity::Uncommon,
            max_stack: 1,
            stats: ItemStats {
                defense: 8.0,
                speed_bonus: -0.05,
                ..Default::default()
            },
            equip_slot: Some(EquipSlot::Boots),
            value: 60,
        });

        // Consumables
        registry.register(ItemDef {
            id: 20,
            name: "Health Potion".into(),
            description: "Restores 50 health.".into(),
            category: ItemCategory::Consumable,
            rarity: ItemRarity::Common,
            max_stack: 10,
            stats: ItemStats {
                heal_amount: 50.0,
                ..Default::default()
            },
            equip_slot: None,
            value: 25,
        });
        registry.register(ItemDef {
            id: 21,
            name: "Greater Health Potion".into(),
            description: "Restores 150 health.".into(),
            category: ItemCategory::Consumable,
            rarity: ItemRarity::Uncommon,
            max_stack: 10,
            stats: ItemStats {
                heal_amount: 150.0,
                ..Default::default()
            },
            equip_slot: None,
            value: 75,
        });
        registry.register(ItemDef {
            id: 22,
            name: "Cooked Meat".into(),
            description: "A hearty meal. Restores 30 health.".into(),
            category: ItemCategory::Consumable,
            rarity: ItemRarity::Common,
            max_stack: 20,
            stats: ItemStats {
                heal_amount: 30.0,
                ..Default::default()
            },
            equip_slot: None,
            value: 5,
        });

        // Materials
        registry.register(ItemDef {
            id: 30,
            name: "Iron Ore".into(),
            description: "Raw iron ore for smelting.".into(),
            category: ItemCategory::Material,
            rarity: ItemRarity::Common,
            max_stack: 50,
            stats: ItemStats::default(),
            equip_slot: None,
            value: 3,
        });
        registry.register(ItemDef {
            id: 31,
            name: "Wood".into(),
            description: "A piece of lumber.".into(),
            category: ItemCategory::Material,
            rarity: ItemRarity::Common,
            max_stack: 50,
            stats: ItemStats::default(),
            equip_slot: None,
            value: 1,
        });
        registry.register(ItemDef {
            id: 32,
            name: "Dragon Scale".into(),
            description: "A shimmering scale from a dragon.".into(),
            category: ItemCategory::Material,
            rarity: ItemRarity::Epic,
            max_stack: 10,
            stats: ItemStats::default(),
            equip_slot: None,
            value: 500,
        });

        // Drop tables
        registry.register_drop_table(DropTable {
            npc_type: "merchant".into(),
            entries: vec![
                DropEntry {
                    item_id: 20,
                    weight: 30,
                    min_quantity: 1,
                    max_quantity: 2,
                },
                DropEntry {
                    item_id: 31,
                    weight: 40,
                    min_quantity: 1,
                    max_quantity: 3,
                },
            ],
            nothing_weight: 50,
        });
        registry.register_drop_table(DropTable {
            npc_type: "guard".into(),
            entries: vec![
                DropEntry {
                    item_id: 1,
                    weight: 20,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                DropEntry {
                    item_id: 2,
                    weight: 5,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                DropEntry {
                    item_id: 10,
                    weight: 15,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                DropEntry {
                    item_id: 30,
                    weight: 30,
                    min_quantity: 1,
                    max_quantity: 5,
                },
            ],
            nothing_weight: 30,
        });
        registry.register_drop_table(DropTable {
            npc_type: "wolf".into(),
            entries: vec![
                DropEntry {
                    item_id: 22,
                    weight: 40,
                    min_quantity: 1,
                    max_quantity: 2,
                },
                DropEntry {
                    item_id: 31,
                    weight: 20,
                    min_quantity: 1,
                    max_quantity: 1,
                },
            ],
            nothing_weight: 40,
        });
        registry.register_drop_table(DropTable {
            npc_type: "dragon".into(),
            entries: vec![
                DropEntry {
                    item_id: 4,
                    weight: 2,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                DropEntry {
                    item_id: 3,
                    weight: 10,
                    min_quantity: 1,
                    max_quantity: 1,
                },
                DropEntry {
                    item_id: 32,
                    weight: 30,
                    min_quantity: 1,
                    max_quantity: 3,
                },
                DropEntry {
                    item_id: 21,
                    weight: 20,
                    min_quantity: 1,
                    max_quantity: 2,
                },
            ],
            nothing_weight: 5,
        });

        registry
    }

    /// Register an item definition.
    pub fn register(&mut self, def: ItemDef) {
        self.items.insert(def.id, def);
    }

    /// Register a drop table.
    pub fn register_drop_table(&mut self, table: DropTable) {
        self.drop_tables.insert(table.npc_type.clone(), table);
    }

    /// Get an item definition by ID.
    pub fn get(&self, id: ItemDefId) -> Option<&ItemDef> {
        self.items.get(&id)
    }

    /// Get a drop table by NPC type.
    pub fn get_drop_table(&self, npc_type: &str) -> Option<&DropTable> {
        self.drop_tables.get(npc_type)
    }

    /// Get all item definitions.
    pub fn all_items(&self) -> impl Iterator<Item = &ItemDef> {
        self.items.values()
    }

    /// Item count.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

/// Deterministic random number from seed and index.
fn deterministic_random(seed: u64, index: u64) -> u64 {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(index.to_le_bytes());
    let result = hasher.finalize();
    u64::from_le_bytes([
        result[0], result[1], result[2], result[3], result[4], result[5], result[6], result[7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_items() {
        let registry = ItemRegistry::default_registry();
        assert!(registry.item_count() > 0);
        assert!(registry.get(1).is_some()); // Wooden Sword
        assert!(registry.get(20).is_some()); // Health Potion
    }

    #[test]
    fn test_item_categories() {
        let registry = ItemRegistry::default_registry();
        let sword = registry.get(1).unwrap();
        assert_eq!(sword.category, ItemCategory::Weapon);
        assert_eq!(sword.rarity, ItemRarity::Common);
        assert!(sword.equip_slot.is_some());
    }

    #[test]
    fn test_drop_table_deterministic() {
        let registry = ItemRegistry::default_registry();
        let table = registry.get_drop_table("guard").unwrap();

        let drop1 = table.roll(12345);
        let drop2 = table.roll(12345);
        assert_eq!(drop1, drop2);
    }

    #[test]
    fn test_drop_table_different_seeds() {
        let registry = ItemRegistry::default_registry();
        let table = registry.get_drop_table("dragon").unwrap();

        // With enough different seeds, we should get some drops
        let mut got_drop = false;
        for seed in 0..100 {
            if table.roll(seed).is_some() {
                got_drop = true;
                break;
            }
        }
        assert!(got_drop, "Dragon table should produce drops");
    }

    #[test]
    fn test_rarity_weights() {
        assert!(ItemRarity::Common.drop_weight() > ItemRarity::Legendary.drop_weight());
        assert!(ItemRarity::Uncommon.drop_weight() > ItemRarity::Rare.drop_weight());
    }

    #[test]
    fn test_item_stats_default() {
        let stats = ItemStats::default();
        assert_eq!(stats.attack, 0.0);
        assert_eq!(stats.defense, 0.0);
        assert_eq!(stats.heal_amount, 0.0);
    }

    #[test]
    fn test_consumable_not_equippable() {
        let registry = ItemRegistry::default_registry();
        let potion = registry.get(20).unwrap();
        assert_eq!(potion.category, ItemCategory::Consumable);
        assert!(potion.equip_slot.is_none());
        assert!(potion.stats.heal_amount > 0.0);
    }
}
