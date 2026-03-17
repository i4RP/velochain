//! WebSocket subscription API for real-time game state streaming.
//!
//! Provides subscription endpoints that clients can use via WebSocket
//! to receive real-time updates about game state, new blocks, and
//! pending transactions.

use jsonrpsee::core::SubscriptionResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::PendingSubscriptionSink;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;
use velochain_game_engine::GameWorld;
use velochain_storage::Database;

/// Events that can be broadcast to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GameEvent {
    /// A new block was produced.
    NewBlock {
        number: u64,
        hash: String,
        tx_count: usize,
        timestamp: u64,
    },
    /// Game tick completed.
    GameTick {
        tick: u64,
        entity_count: usize,
        player_count: usize,
        state_root: String,
    },
    /// A new pending transaction was received.
    PendingTransaction { hash: String },
    /// Player state changed (position, health, etc.).
    PlayerState {
        address: String,
        position: [f32; 3],
        health: f32,
        is_alive: bool,
    },
    /// Chat message broadcast.
    ChatMessage {
        sender: String,
        message: String,
        tick: u64,
    },
    /// Entity update (spawn, move, despawn).
    EntityUpdate {
        entity_id: u64,
        entity_type: String,
        position: [f32; 3],
        health: Option<f32>,
        removed: bool,
    },
}

/// Channel sender for broadcasting events.
pub type EventSender = broadcast::Sender<GameEvent>;

/// Create a new event broadcast channel.
pub fn new_event_channel() -> (EventSender, broadcast::Receiver<GameEvent>) {
    broadcast::channel(256)
}

/// Subscription API for real-time updates.
#[rpc(server, namespace = "velochain")]
pub trait SubscriptionApi {
    /// Subscribe to new block headers.
    #[subscription(name = "subscribeNewBlocks" => "newBlock", unsubscribe = "unsubscribeNewBlocks", item = GameEvent)]
    async fn subscribe_new_blocks(&self) -> SubscriptionResult;

    /// Subscribe to game tick updates.
    #[subscription(name = "subscribeGameTicks" => "gameTick", unsubscribe = "unsubscribeGameTicks", item = GameEvent)]
    async fn subscribe_game_ticks(&self) -> SubscriptionResult;

    /// Subscribe to pending transactions.
    #[subscription(name = "subscribePendingTxs" => "pendingTx", unsubscribe = "unsubscribePendingTxs", item = GameEvent)]
    async fn subscribe_pending_txs(&self) -> SubscriptionResult;

    /// Subscribe to player state changes.
    #[subscription(name = "subscribePlayerState" => "playerState", unsubscribe = "unsubscribePlayerState", item = GameEvent)]
    async fn subscribe_player_state(&self) -> SubscriptionResult;

    /// Subscribe to chat messages.
    #[subscription(name = "subscribeChatMessages" => "chatMessage", unsubscribe = "unsubscribeChatMessages", item = GameEvent)]
    async fn subscribe_chat_messages(&self) -> SubscriptionResult;

    /// Subscribe to entity updates (spawn, move, despawn).
    #[subscription(name = "subscribeEntityUpdates" => "entityUpdate", unsubscribe = "unsubscribeEntityUpdates", item = GameEvent)]
    async fn subscribe_entity_updates(&self) -> SubscriptionResult;
}

/// Ethereum-standard subscription API.
#[rpc(server, namespace = "eth")]
pub trait EthSubscriptionApi {
    /// Subscribe to new block heads (Ethereum standard).
    #[subscription(name = "subscribe" => "subscription", unsubscribe = "unsubscribe", item = serde_json::Value)]
    async fn subscribe(&self, kind: String) -> SubscriptionResult;
}

/// Subscription API implementation.
pub struct SubscriptionApiImpl {
    event_tx: EventSender,
    _game_world: Arc<GameWorld>,
    _db: Arc<Database>,
}

impl SubscriptionApiImpl {
    pub fn new(event_tx: EventSender, game_world: Arc<GameWorld>, db: Arc<Database>) -> Self {
        Self {
            event_tx,
            _game_world: game_world,
            _db: db,
        }
    }
}

#[jsonrpsee::core::async_trait]
impl SubscriptionApiServer for SubscriptionApiImpl {
    async fn subscribe_new_blocks(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::NewBlock { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    async fn subscribe_game_ticks(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::GameTick { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    async fn subscribe_pending_txs(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::PendingTransaction { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    async fn subscribe_player_state(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::PlayerState { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    async fn subscribe_chat_messages(
        &self,
        pending: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::ChatMessage { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }

    async fn subscribe_entity_updates(
        &self,
        pending: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if matches!(event, GameEvent::EntityUpdate { .. }) {
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&event)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }
}

/// Ethereum standard subscription implementation.
pub struct EthSubscriptionApiImpl {
    event_tx: EventSender,
    _game_world: Arc<GameWorld>,
    _db: Arc<Database>,
}

impl EthSubscriptionApiImpl {
    pub fn new(event_tx: EventSender, game_world: Arc<GameWorld>, db: Arc<Database>) -> Self {
        Self {
            event_tx,
            _game_world: game_world,
            _db: db,
        }
    }
}

#[jsonrpsee::core::async_trait]
impl EthSubscriptionApiServer for EthSubscriptionApiImpl {
    async fn subscribe(
        &self,
        pending: PendingSubscriptionSink,
        kind: String,
    ) -> SubscriptionResult {
        let mut rx = self.event_tx.subscribe();
        let sink = pending.accept().await?;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let should_send = match kind.as_str() {
                            "newHeads" => matches!(event, GameEvent::NewBlock { .. }),
                            "newPendingTransactions" => {
                                matches!(event, GameEvent::PendingTransaction { .. })
                            }
                            _ => false,
                        };
                        if should_send {
                            let value = serde_json::to_value(&event).unwrap_or_default();
                            let msg = jsonrpsee::SubscriptionMessage::from_json(&value)
                                .expect("serialize event");
                            if sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Subscriber lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Ok(())
    }
}
