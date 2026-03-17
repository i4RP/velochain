//! NPC shop system with merchant buy/sell and dynamic pricing.
//!
//! Merchant NPCs offer items for sale and buy items from players.
//! Prices fluctuate based on supply and demand (trade volume).

use crate::items::{ItemDefId, ItemRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique shop identifier.
pub type ShopId = u32;

/// A shop listing (item for sale).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopListing {
    pub item_id: ItemDefId,
    /// Base price (before dynamic adjustments).
    pub base_price: u64,
    /// Current stock (None = unlimited).
    pub stock: Option<u32>,
    /// Maximum stock for restocking.
    pub max_stock: Option<u32>,
    /// Restock rate: units per N ticks.
    pub restock_rate: u32,
    /// Ticks between restocks.
    pub restock_interval: u32,
    /// Last restock tick.
    pub last_restock_tick: u64,
}

/// Shop definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopDef {
    pub id: ShopId,
    /// NPC type that owns this shop.
    pub npc_type: String,
    /// Display name.
    pub name: String,
    /// Items for sale.
    pub listings: Vec<ShopListing>,
    /// Buy-back rate: percentage of base value the shop pays (0.0-1.0).
    pub buyback_rate: f32,
    /// Price modifier based on demand (tracks recent purchases).
    pub demand_tracker: DemandTracker,
}

/// Tracks recent purchase/sell volumes for dynamic pricing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DemandTracker {
    /// Recent buy volume per item: item_id -> count in current window.
    pub buy_volume: HashMap<ItemDefId, u32>,
    /// Recent sell volume per item: item_id -> count in current window.
    pub sell_volume: HashMap<ItemDefId, u32>,
    /// Ticks since last reset.
    pub window_ticks: u64,
    /// Window size in ticks.
    pub window_size: u64,
}

impl DemandTracker {
    pub fn new(window_size: u64) -> Self {
        Self {
            buy_volume: HashMap::new(),
            sell_volume: HashMap::new(),
            window_ticks: 0,
            window_size,
        }
    }

    /// Record a purchase (player buying from shop).
    pub fn record_buy(&mut self, item_id: ItemDefId, quantity: u32) {
        *self.buy_volume.entry(item_id).or_insert(0) += quantity;
    }

    /// Record a sale (player selling to shop).
    pub fn record_sell(&mut self, item_id: ItemDefId, quantity: u32) {
        *self.sell_volume.entry(item_id).or_insert(0) += quantity;
    }

    /// Tick the demand window. Resets counters after window expires.
    pub fn tick(&mut self) {
        self.window_ticks += 1;
        if self.window_ticks >= self.window_size {
            // Decay volumes instead of hard reset
            for v in self.buy_volume.values_mut() {
                *v /= 2;
            }
            for v in self.sell_volume.values_mut() {
                *v /= 2;
            }
            self.window_ticks = 0;
        }
    }

    /// Get the price multiplier for an item based on demand.
    /// High demand (many buys) increases price; high supply (many sells) decreases.
    pub fn price_multiplier(&self, item_id: ItemDefId) -> f32 {
        let buys = self.buy_volume.get(&item_id).copied().unwrap_or(0) as f32;
        let sells = self.sell_volume.get(&item_id).copied().unwrap_or(0) as f32;

        // Base multiplier is 1.0
        // Each net buy adds 2%, each net sell subtracts 1%
        // Clamped to [0.5, 2.0]
        let net = buys - sells;
        let multiplier = 1.0 + net * 0.02;
        multiplier.clamp(0.5, 2.0)
    }
}

/// Result of a shop transaction.
#[derive(Debug, Clone)]
pub enum ShopResult {
    /// Purchase successful.
    Bought { item_id: ItemDefId, quantity: u32, total_cost: u64 },
    /// Sale successful.
    Sold { item_id: ItemDefId, quantity: u32, total_earned: u64 },
    /// Error.
    Error(ShopError),
}

/// Shop error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShopError {
    /// Shop not found.
    ShopNotFound(ShopId),
    /// Item not sold by this shop.
    ItemNotAvailable(ItemDefId),
    /// Not enough stock.
    OutOfStock { item_id: ItemDefId, available: u32, requested: u32 },
    /// Player can't afford it.
    InsufficientGold { required: u64, available: u64 },
    /// Player doesn't have the item to sell.
    InsufficientItems { item_id: ItemDefId, required: u32, available: u32 },
    /// Item cannot be sold to this shop.
    CannotSellItem(ItemDefId),
}

/// Shop manager holding all NPC shops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopManager {
    shops: HashMap<ShopId, ShopDef>,
}

impl ShopManager {
    pub fn new() -> Self {
        Self {
            shops: HashMap::new(),
        }
    }

    /// Create the default shop manager with built-in shops.
    pub fn default_shops(item_registry: &ItemRegistry) -> Self {
        let mut manager = Self::new();
        let _ = item_registry;

        // General merchant shop
        manager.register(ShopDef {
            id: 1,
            npc_type: "merchant".into(),
            name: "General Store".into(),
            listings: vec![
                ShopListing {
                    item_id: 20, // Health Potion
                    base_price: 30,
                    stock: Some(20),
                    max_stock: Some(20),
                    restock_rate: 2,
                    restock_interval: 100,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 21, // Greater Health Potion
                    base_price: 100,
                    stock: Some(5),
                    max_stock: Some(5),
                    restock_rate: 1,
                    restock_interval: 200,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 22, // Cooked Meat
                    base_price: 8,
                    stock: Some(50),
                    max_stock: Some(50),
                    restock_rate: 5,
                    restock_interval: 50,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 1, // Wooden Sword
                    base_price: 15,
                    stock: Some(5),
                    max_stock: Some(5),
                    restock_rate: 1,
                    restock_interval: 300,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 10, // Leather Helmet
                    base_price: 20,
                    stock: Some(3),
                    max_stock: Some(3),
                    restock_rate: 1,
                    restock_interval: 300,
                    last_restock_tick: 0,
                },
            ],
            buyback_rate: 0.4,
            demand_tracker: DemandTracker::new(600),
        });

        // Blacksmith shop
        manager.register(ShopDef {
            id: 2,
            npc_type: "guard".into(),
            name: "Armory".into(),
            listings: vec![
                ShopListing {
                    item_id: 2, // Iron Sword
                    base_price: 75,
                    stock: Some(3),
                    max_stock: Some(3),
                    restock_rate: 1,
                    restock_interval: 500,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 11, // Iron Chestplate
                    base_price: 120,
                    stock: Some(2),
                    max_stock: Some(2),
                    restock_rate: 1,
                    restock_interval: 500,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 12, // Steel Boots
                    base_price: 90,
                    stock: Some(2),
                    max_stock: Some(2),
                    restock_rate: 1,
                    restock_interval: 500,
                    last_restock_tick: 0,
                },
                ShopListing {
                    item_id: 30, // Iron Ore
                    base_price: 5,
                    stock: Some(30),
                    max_stock: Some(30),
                    restock_rate: 3,
                    restock_interval: 100,
                    last_restock_tick: 0,
                },
            ],
            buyback_rate: 0.5,
            demand_tracker: DemandTracker::new(600),
        });

        manager
    }

    /// Register a shop.
    pub fn register(&mut self, shop: ShopDef) {
        self.shops.insert(shop.id, shop);
    }

    /// Get a shop by ID.
    pub fn get(&self, id: ShopId) -> Option<&ShopDef> {
        self.shops.get(&id)
    }

    /// Get a mutable shop by ID.
    pub fn get_mut(&mut self, id: ShopId) -> Option<&mut ShopDef> {
        self.shops.get_mut(&id)
    }

    /// Find a shop by NPC type.
    pub fn find_by_npc(&self, npc_type: &str) -> Option<&ShopDef> {
        self.shops.values().find(|s| s.npc_type == npc_type)
    }

    /// Find a mutable shop by NPC type.
    pub fn find_by_npc_mut(&mut self, npc_type: &str) -> Option<&mut ShopDef> {
        self.shops.values_mut().find(|s| s.npc_type == npc_type)
    }

    /// Shop count.
    pub fn shop_count(&self) -> usize {
        self.shops.len()
    }

    /// Get the current buy price for an item in a shop.
    pub fn buy_price(&self, shop_id: ShopId, item_id: ItemDefId) -> Option<u64> {
        let shop = self.shops.get(&shop_id)?;
        let listing = shop.listings.iter().find(|l| l.item_id == item_id)?;
        let multiplier = shop.demand_tracker.price_multiplier(item_id);
        Some((listing.base_price as f32 * multiplier).ceil() as u64)
    }

    /// Get the sell price for an item at a shop.
    pub fn sell_price(
        &self,
        shop_id: ShopId,
        item_id: ItemDefId,
        item_registry: &ItemRegistry,
    ) -> Option<u64> {
        let shop = self.shops.get(&shop_id)?;
        let item = item_registry.get(item_id)?;
        let multiplier = shop.demand_tracker.price_multiplier(item_id);
        // Sell price = base value * buyback_rate / demand multiplier
        let price = (item.value as f32 * shop.buyback_rate / multiplier).floor() as u64;
        Some(price.max(1))
    }

    /// Buy an item from a shop.
    pub fn buy_item(
        &mut self,
        shop_id: ShopId,
        item_id: ItemDefId,
        quantity: u32,
        player_gold: u64,
    ) -> ShopResult {
        let shop = match self.shops.get_mut(&shop_id) {
            Some(s) => s,
            None => return ShopResult::Error(ShopError::ShopNotFound(shop_id)),
        };

        let listing = match shop.listings.iter_mut().find(|l| l.item_id == item_id) {
            Some(l) => l,
            None => return ShopResult::Error(ShopError::ItemNotAvailable(item_id)),
        };

        // Check stock
        if let Some(stock) = listing.stock {
            if stock < quantity {
                return ShopResult::Error(ShopError::OutOfStock {
                    item_id,
                    available: stock,
                    requested: quantity,
                });
            }
        }

        // Calculate total cost
        let multiplier = shop.demand_tracker.price_multiplier(item_id);
        let unit_price = (listing.base_price as f32 * multiplier).ceil() as u64;
        let total_cost = unit_price * quantity as u64;

        if player_gold < total_cost {
            return ShopResult::Error(ShopError::InsufficientGold {
                required: total_cost,
                available: player_gold,
            });
        }

        // Execute purchase
        if let Some(ref mut stock) = listing.stock {
            *stock -= quantity;
        }
        shop.demand_tracker.record_buy(item_id, quantity);

        ShopResult::Bought {
            item_id,
            quantity,
            total_cost,
        }
    }

    /// Sell an item to a shop.
    pub fn sell_item(
        &mut self,
        shop_id: ShopId,
        item_id: ItemDefId,
        quantity: u32,
        player_item_count: u32,
        item_registry: &ItemRegistry,
    ) -> ShopResult {
        let item = match item_registry.get(item_id) {
            Some(i) => i,
            None => return ShopResult::Error(ShopError::CannotSellItem(item_id)),
        };

        let shop = match self.shops.get_mut(&shop_id) {
            Some(s) => s,
            None => return ShopResult::Error(ShopError::ShopNotFound(shop_id)),
        };

        if player_item_count < quantity {
            return ShopResult::Error(ShopError::InsufficientItems {
                item_id,
                required: quantity,
                available: player_item_count,
            });
        }

        let multiplier = shop.demand_tracker.price_multiplier(item_id);
        let unit_price = (item.value as f32 * shop.buyback_rate / multiplier).floor() as u64;
        let total_earned = unit_price.max(1) * quantity as u64;

        shop.demand_tracker.record_sell(item_id, quantity);

        ShopResult::Sold {
            item_id,
            quantity,
            total_earned,
        }
    }

    /// Tick all shops for restocking and demand window.
    pub fn tick_shops(&mut self, current_tick: u64) {
        for shop in self.shops.values_mut() {
            // Tick demand tracker
            shop.demand_tracker.tick();

            // Restock items
            for listing in &mut shop.listings {
                if let (Some(stock), Some(max_stock)) = (listing.stock, listing.max_stock) {
                    if stock < max_stock
                        && listing.restock_interval > 0
                        && current_tick >= listing.last_restock_tick + listing.restock_interval as u64
                    {
                        let new_stock = (stock + listing.restock_rate).min(max_stock);
                        listing.stock = Some(new_stock);
                        listing.last_restock_tick = current_tick;
                    }
                }
            }
        }
    }
}

impl Default for ShopManager {
    fn default() -> Self {
        Self::default_shops(&ItemRegistry::default_registry())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shops() {
        let manager = ShopManager::default();
        assert!(manager.shop_count() > 0);
        assert!(manager.get(1).is_some()); // General Store
    }

    #[test]
    fn test_buy_item() {
        let mut manager = ShopManager::default();
        let result = manager.buy_item(1, 20, 1, 1000); // Buy 1 Health Potion
        assert!(matches!(result, ShopResult::Bought { .. }));
        if let ShopResult::Bought { total_cost, .. } = result {
            assert!(total_cost > 0);
        }
    }

    #[test]
    fn test_buy_insufficient_gold() {
        let mut manager = ShopManager::default();
        let result = manager.buy_item(1, 20, 1, 0);
        assert!(matches!(result, ShopResult::Error(ShopError::InsufficientGold { .. })));
    }

    #[test]
    fn test_buy_out_of_stock() {
        let mut manager = ShopManager::default();
        // Buy all stock
        let result = manager.buy_item(1, 20, 100, 100000);
        assert!(matches!(result, ShopResult::Error(ShopError::OutOfStock { .. })));
    }

    #[test]
    fn test_sell_item() {
        let item_reg = ItemRegistry::default_registry();
        let mut manager = ShopManager::default();
        let result = manager.sell_item(1, 1, 1, 5, &item_reg); // Sell Wooden Sword
        assert!(matches!(result, ShopResult::Sold { .. }));
        if let ShopResult::Sold { total_earned, .. } = result {
            assert!(total_earned > 0);
        }
    }

    #[test]
    fn test_sell_insufficient_items() {
        let item_reg = ItemRegistry::default_registry();
        let mut manager = ShopManager::default();
        let result = manager.sell_item(1, 1, 5, 2, &item_reg);
        assert!(matches!(result, ShopResult::Error(ShopError::InsufficientItems { .. })));
    }

    #[test]
    fn test_dynamic_pricing() {
        let mut manager = ShopManager::default();
        let initial_price = manager.buy_price(1, 20).unwrap();

        // Simulate demand: buy many potions
        for _ in 0..10 {
            manager.buy_item(1, 20, 1, 100000);
        }

        let new_price = manager.buy_price(1, 20).unwrap();
        // Price should increase due to demand
        assert!(new_price >= initial_price);
    }

    #[test]
    fn test_demand_tracker() {
        let mut tracker = DemandTracker::new(100);
        assert!((tracker.price_multiplier(1) - 1.0).abs() < 0.01);

        tracker.record_buy(1, 10);
        let mult = tracker.price_multiplier(1);
        assert!(mult > 1.0);

        tracker.record_sell(1, 20);
        let mult2 = tracker.price_multiplier(1);
        assert!(mult2 < mult);
    }

    #[test]
    fn test_restock() {
        let mut manager = ShopManager::default();
        // Buy all potions
        while matches!(manager.buy_item(1, 20, 1, 100000), ShopResult::Bought { .. }) {}

        // Tick past restock interval
        manager.tick_shops(200);

        let shop = manager.get(1).unwrap();
        let potion_listing = shop.listings.iter().find(|l| l.item_id == 20).unwrap();
        assert!(potion_listing.stock.unwrap() > 0);
    }

    #[test]
    fn test_find_by_npc() {
        let manager = ShopManager::default();
        assert!(manager.find_by_npc("merchant").is_some());
        assert!(manager.find_by_npc("guard").is_some());
        assert!(manager.find_by_npc("dragon").is_none());
    }

    #[test]
    fn test_sell_price() {
        let item_reg = ItemRegistry::default_registry();
        let manager = ShopManager::default();
        let price = manager.sell_price(1, 1, &item_reg); // Wooden Sword
        assert!(price.is_some());
        assert!(price.unwrap() > 0);
    }

    #[test]
    fn test_demand_window_decay() {
        let mut tracker = DemandTracker::new(5);
        tracker.record_buy(1, 20);
        let mult_before = tracker.price_multiplier(1);

        // Tick through window
        for _ in 0..5 {
            tracker.tick();
        }
        let mult_after = tracker.price_multiplier(1);
        assert!(mult_after < mult_before); // Volume should decay
    }
}
