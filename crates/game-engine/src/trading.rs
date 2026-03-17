//! Player-to-player trading system with on-chain secure item exchange.
//!
//! Trades are two-phase: both players must confirm before items swap.
//! All trade operations are deterministic and processed in the game tick.

use crate::items::ItemDefId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique trade session identifier.
pub type TradeId = u64;

/// State of a trade session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeState {
    /// Trade proposed, waiting for acceptor to join.
    Proposed,
    /// Both players are in the trade window, adding items.
    Negotiating,
    /// Initiator has confirmed.
    InitiatorConfirmed,
    /// Acceptor has confirmed.
    AcceptorConfirmed,
    /// Both confirmed, trade executed.
    Completed,
    /// Trade was cancelled by one party.
    Cancelled,
    /// Trade expired (timed out).
    Expired,
}

/// A trade offer from one player.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeOffer {
    /// Items offered: (item_id, quantity).
    pub items: Vec<(ItemDefId, u32)>,
    /// Currency (gold) offered.
    pub gold: u64,
    /// Whether this player has confirmed.
    pub confirmed: bool,
}

/// A trade session between two players.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSession {
    pub id: TradeId,
    /// Address of the player who initiated the trade.
    pub initiator: String,
    /// Address of the player who was invited.
    pub acceptor: String,
    /// Current state.
    pub state: TradeState,
    /// Initiator's offer.
    pub initiator_offer: TradeOffer,
    /// Acceptor's offer.
    pub acceptor_offer: TradeOffer,
    /// Tick when trade was created (for timeout).
    pub created_tick: u64,
    /// Maximum ticks before expiry.
    pub timeout_ticks: u64,
}

/// Result of a trade operation.
#[derive(Debug, Clone)]
pub enum TradeResult {
    /// Trade created successfully.
    Created(TradeId),
    /// Trade accepted (other player joined).
    Accepted(TradeId),
    /// Item added to offer.
    ItemAdded,
    /// Trade confirmed by this player.
    Confirmed,
    /// Trade executed successfully.
    Executed {
        trade_id: TradeId,
        initiator_received: Vec<(ItemDefId, u32)>,
        acceptor_received: Vec<(ItemDefId, u32)>,
    },
    /// Trade cancelled.
    Cancelled(TradeId),
    /// Error.
    Error(TradeError),
}

/// Trade error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeError {
    /// Trade not found.
    NotFound(TradeId),
    /// Player is not part of this trade.
    NotParticipant,
    /// Invalid state transition.
    InvalidState(String),
    /// Player doesn't have enough items.
    InsufficientItems(ItemDefId, u32, u32),
    /// Trade has expired.
    Expired,
    /// Player already in another trade.
    AlreadyTrading,
}

/// Trade manager handles all active trades.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeManager {
    /// Active trade sessions.
    trades: HashMap<TradeId, TradeSession>,
    /// Next trade ID.
    next_id: TradeId,
    /// Map of player address -> active trade ID (one trade at a time).
    active_trades: HashMap<String, TradeId>,
    /// Default timeout in ticks.
    pub default_timeout: u64,
}

impl TradeManager {
    pub fn new() -> Self {
        Self {
            trades: HashMap::new(),
            next_id: 1,
            active_trades: HashMap::new(),
            default_timeout: 600, // ~10 minutes at 1 tick/sec
        }
    }

    /// Propose a trade to another player.
    pub fn propose_trade(
        &mut self,
        initiator: &str,
        acceptor: &str,
        current_tick: u64,
    ) -> TradeResult {
        // Check if either player is already trading
        if self.active_trades.contains_key(initiator) {
            return TradeResult::Error(TradeError::AlreadyTrading);
        }
        if self.active_trades.contains_key(acceptor) {
            return TradeResult::Error(TradeError::AlreadyTrading);
        }

        let id = self.next_id;
        self.next_id += 1;

        let session = TradeSession {
            id,
            initiator: initiator.to_string(),
            acceptor: acceptor.to_string(),
            state: TradeState::Proposed,
            initiator_offer: TradeOffer::default(),
            acceptor_offer: TradeOffer::default(),
            created_tick: current_tick,
            timeout_ticks: self.default_timeout,
        };

        self.trades.insert(id, session);
        self.active_trades.insert(initiator.to_string(), id);

        TradeResult::Created(id)
    }

    /// Accept a trade proposal.
    pub fn accept_trade(&mut self, trade_id: TradeId, player: &str) -> TradeResult {
        let trade = match self.trades.get_mut(&trade_id) {
            Some(t) => t,
            None => return TradeResult::Error(TradeError::NotFound(trade_id)),
        };

        if trade.acceptor != player {
            return TradeResult::Error(TradeError::NotParticipant);
        }

        if trade.state != TradeState::Proposed {
            return TradeResult::Error(TradeError::InvalidState(
                "Trade is not in Proposed state".into(),
            ));
        }

        trade.state = TradeState::Negotiating;
        self.active_trades.insert(player.to_string(), trade_id);

        TradeResult::Accepted(trade_id)
    }

    /// Add an item to a player's trade offer.
    pub fn add_item(
        &mut self,
        trade_id: TradeId,
        player: &str,
        item_id: ItemDefId,
        quantity: u32,
    ) -> TradeResult {
        let trade = match self.trades.get_mut(&trade_id) {
            Some(t) => t,
            None => return TradeResult::Error(TradeError::NotFound(trade_id)),
        };

        match trade.state {
            TradeState::Negotiating
            | TradeState::InitiatorConfirmed
            | TradeState::AcceptorConfirmed => {}
            _ => {
                return TradeResult::Error(TradeError::InvalidState(
                    "Trade is not in a modifiable state".into(),
                ));
            }
        }

        let offer = if trade.initiator == player {
            // Reset confirmations when offer changes
            trade.initiator_offer.confirmed = false;
            trade.acceptor_offer.confirmed = false;
            trade.state = TradeState::Negotiating;
            &mut trade.initiator_offer
        } else if trade.acceptor == player {
            trade.initiator_offer.confirmed = false;
            trade.acceptor_offer.confirmed = false;
            trade.state = TradeState::Negotiating;
            &mut trade.acceptor_offer
        } else {
            return TradeResult::Error(TradeError::NotParticipant);
        };

        // Add or increment item in offer
        if let Some(existing) = offer.items.iter_mut().find(|(id, _)| *id == item_id) {
            existing.1 += quantity;
        } else {
            offer.items.push((item_id, quantity));
        }

        TradeResult::ItemAdded
    }

    /// Confirm a trade offer.
    pub fn confirm_trade(&mut self, trade_id: TradeId, player: &str) -> TradeResult {
        let trade = match self.trades.get_mut(&trade_id) {
            Some(t) => t,
            None => return TradeResult::Error(TradeError::NotFound(trade_id)),
        };

        match trade.state {
            TradeState::Negotiating
            | TradeState::InitiatorConfirmed
            | TradeState::AcceptorConfirmed => {}
            _ => {
                return TradeResult::Error(TradeError::InvalidState(
                    "Trade cannot be confirmed in current state".into(),
                ));
            }
        }

        if trade.initiator == player {
            trade.initiator_offer.confirmed = true;
            if trade.acceptor_offer.confirmed {
                trade.state = TradeState::Completed;
            } else {
                trade.state = TradeState::InitiatorConfirmed;
            }
        } else if trade.acceptor == player {
            trade.acceptor_offer.confirmed = true;
            if trade.initiator_offer.confirmed {
                trade.state = TradeState::Completed;
            } else {
                trade.state = TradeState::AcceptorConfirmed;
            }
        } else {
            return TradeResult::Error(TradeError::NotParticipant);
        }

        if trade.state == TradeState::Completed {
            let initiator_received = trade.acceptor_offer.items.clone();
            let acceptor_received = trade.initiator_offer.items.clone();
            let tid = trade.id;

            // Clean up
            let initiator = trade.initiator.clone();
            let acceptor = trade.acceptor.clone();
            self.active_trades.remove(&initiator);
            self.active_trades.remove(&acceptor);

            return TradeResult::Executed {
                trade_id: tid,
                initiator_received,
                acceptor_received,
            };
        }

        TradeResult::Confirmed
    }

    /// Cancel a trade.
    pub fn cancel_trade(&mut self, trade_id: TradeId, player: &str) -> TradeResult {
        let trade = match self.trades.get_mut(&trade_id) {
            Some(t) => t,
            None => return TradeResult::Error(TradeError::NotFound(trade_id)),
        };

        if trade.initiator != player && trade.acceptor != player {
            return TradeResult::Error(TradeError::NotParticipant);
        }

        trade.state = TradeState::Cancelled;
        self.active_trades.remove(&trade.initiator);
        self.active_trades.remove(&trade.acceptor);

        TradeResult::Cancelled(trade_id)
    }

    /// Expire old trades. Called each tick.
    pub fn tick_expirations(&mut self, current_tick: u64) {
        let expired: Vec<TradeId> = self
            .trades
            .iter()
            .filter(|(_, t)| {
                !matches!(
                    t.state,
                    TradeState::Completed | TradeState::Cancelled | TradeState::Expired
                ) && current_tick > t.created_tick + t.timeout_ticks
            })
            .map(|(id, _)| *id)
            .collect();

        for id in expired {
            if let Some(trade) = self.trades.get_mut(&id) {
                trade.state = TradeState::Expired;
                self.active_trades.remove(&trade.initiator);
                self.active_trades.remove(&trade.acceptor);
            }
        }
    }

    /// Get a trade session by ID.
    pub fn get_trade(&self, trade_id: TradeId) -> Option<&TradeSession> {
        self.trades.get(&trade_id)
    }

    /// Get the active trade for a player.
    pub fn get_player_trade(&self, player: &str) -> Option<&TradeSession> {
        self.active_trades
            .get(player)
            .and_then(|id| self.trades.get(id))
    }

    /// Active trade count.
    pub fn active_count(&self) -> usize {
        self.active_trades.len()
    }
}

impl Default for TradeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propose_trade() {
        let mut mgr = TradeManager::new();
        let result = mgr.propose_trade("alice", "bob", 1);
        assert!(matches!(result, TradeResult::Created(1)));
        assert_eq!(mgr.active_count(), 1);
    }

    #[test]
    fn test_accept_trade() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        let result = mgr.accept_trade(1, "bob");
        assert!(matches!(result, TradeResult::Accepted(1)));
        assert_eq!(mgr.active_count(), 2);
    }

    #[test]
    fn test_add_items_and_confirm() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        mgr.accept_trade(1, "bob");

        mgr.add_item(1, "alice", 1, 1); // Alice offers 1 Wooden Sword
        mgr.add_item(1, "bob", 30, 5); // Bob offers 5 Iron Ore

        let r1 = mgr.confirm_trade(1, "alice");
        assert!(matches!(r1, TradeResult::Confirmed));

        let r2 = mgr.confirm_trade(1, "bob");
        assert!(matches!(r2, TradeResult::Executed { .. }));
    }

    #[test]
    fn test_cancel_trade() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        let result = mgr.cancel_trade(1, "alice");
        assert!(matches!(result, TradeResult::Cancelled(1)));
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_already_trading() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        let result = mgr.propose_trade("alice", "charlie", 1);
        assert!(matches!(
            result,
            TradeResult::Error(TradeError::AlreadyTrading)
        ));
    }

    #[test]
    fn test_trade_expiration() {
        let mut mgr = TradeManager::new();
        mgr.default_timeout = 100;
        mgr.propose_trade("alice", "bob", 1);
        mgr.tick_expirations(200);
        let trade = mgr.get_trade(1).unwrap();
        assert_eq!(trade.state, TradeState::Expired);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_not_participant() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        let result = mgr.accept_trade(1, "charlie");
        assert!(matches!(
            result,
            TradeResult::Error(TradeError::NotParticipant)
        ));
    }

    #[test]
    fn test_confirm_resets_on_offer_change() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        mgr.accept_trade(1, "bob");

        mgr.add_item(1, "alice", 1, 1);
        mgr.confirm_trade(1, "alice");

        // Bob adds item, which should reset Alice's confirmation
        mgr.add_item(1, "bob", 30, 5);
        let trade = mgr.get_trade(1).unwrap();
        assert!(!trade.initiator_offer.confirmed);
    }

    #[test]
    fn test_trade_not_found() {
        let mgr = TradeManager::new();
        assert!(mgr.get_trade(999).is_none());
    }

    #[test]
    fn test_get_player_trade() {
        let mut mgr = TradeManager::new();
        mgr.propose_trade("alice", "bob", 1);
        assert!(mgr.get_player_trade("alice").is_some());
        assert!(mgr.get_player_trade("bob").is_none()); // Not yet accepted
    }
}
