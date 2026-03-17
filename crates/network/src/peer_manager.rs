//! Peer scoring and connection management.
//!
//! Tracks peer reputation based on behavior (good blocks, bad data, timeouts)
//! and manages connection limits, eviction, and reconnection.

use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Peer reputation score.
pub type Score = i64;

/// Default initial score for new peers.
pub const INITIAL_SCORE: Score = 100;
/// Score threshold below which a peer is disconnected.
pub const DISCONNECT_THRESHOLD: Score = -50;
/// Score threshold below which a peer is banned.
pub const BAN_THRESHOLD: Score = -200;
/// Maximum score a peer can achieve.
pub const MAX_SCORE: Score = 500;

/// Reasons for scoring adjustments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreEvent {
    /// Peer provided a valid block.
    ValidBlock,
    /// Peer provided valid headers during sync.
    ValidHeaders,
    /// Peer provided valid bodies during sync.
    ValidBodies,
    /// Peer responded to a ping/status request.
    PongReceived,
    /// Peer provided an invalid block.
    InvalidBlock,
    /// Peer provided invalid headers.
    InvalidHeaders,
    /// Peer request timed out.
    Timeout,
    /// Peer sent duplicate data.
    DuplicateData,
    /// Peer sent unsolicited data.
    UnsolicitedData,
    /// Peer's chain ID doesn't match.
    WrongChainId,
    /// Periodic decay toward initial score.
    Decay,
}

impl ScoreEvent {
    /// Get the score adjustment for this event.
    pub fn score_delta(self) -> Score {
        match self {
            Self::ValidBlock => 10,
            Self::ValidHeaders => 5,
            Self::ValidBodies => 5,
            Self::PongReceived => 1,
            Self::InvalidBlock => -50,
            Self::InvalidHeaders => -30,
            Self::Timeout => -10,
            Self::DuplicateData => -5,
            Self::UnsolicitedData => -3,
            Self::WrongChainId => -100,
            Self::Decay => 0, // handled separately
        }
    }
}

/// Connection direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// We initiated the connection.
    Outbound,
    /// The peer initiated the connection.
    Inbound,
}

/// Information about a connected peer.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer identifier (e.g. libp2p PeerId string).
    pub peer_id: String,
    /// Current reputation score.
    pub score: Score,
    /// Connection direction.
    pub direction: Direction,
    /// Peer's reported best block number.
    pub best_block: u64,
    /// Peer's chain ID (0 if unknown).
    pub chain_id: u64,
    /// Tick when the peer connected.
    pub connected_tick: u64,
    /// Tick of last activity.
    pub last_activity_tick: u64,
    /// Number of valid messages received.
    pub good_messages: u64,
    /// Number of invalid messages received.
    pub bad_messages: u64,
    /// Whether the peer has been marked for disconnection.
    pub disconnect_pending: bool,
}

/// Peer manager configuration.
#[derive(Debug, Clone)]
pub struct PeerManagerConfig {
    /// Maximum number of connected peers.
    pub max_peers: usize,
    /// Maximum number of inbound peers.
    pub max_inbound: usize,
    /// Maximum number of outbound peers.
    pub max_outbound: usize,
    /// Ticks of inactivity before a peer is considered idle.
    pub idle_timeout_ticks: u64,
    /// How often (in ticks) to decay scores toward the initial value.
    pub decay_interval_ticks: u64,
    /// Score decay amount per interval (moves toward INITIAL_SCORE).
    pub decay_amount: Score,
    /// Our chain ID for validation.
    pub chain_id: u64,
}

impl Default for PeerManagerConfig {
    fn default() -> Self {
        Self {
            max_peers: 50,
            max_inbound: 30,
            max_outbound: 20,
            idle_timeout_ticks: 300,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: velochain_primitives::DEFAULT_CHAIN_ID,
        }
    }
}

/// Manages peer connections, scoring, and eviction.
pub struct PeerManager {
    /// Connected peers indexed by peer ID.
    peers: HashMap<String, PeerInfo>,
    /// Banned peer IDs with ban expiry tick.
    banned: HashMap<String, u64>,
    /// Configuration.
    config: PeerManagerConfig,
    /// Current tick.
    current_tick: u64,
    /// Last decay tick.
    last_decay_tick: u64,
}

impl PeerManager {
    /// Create a new peer manager.
    pub fn new(config: PeerManagerConfig) -> Self {
        Self {
            peers: HashMap::new(),
            banned: HashMap::new(),
            config,
            current_tick: 0,
            last_decay_tick: 0,
        }
    }

    /// Get the number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Get the number of inbound peers.
    pub fn inbound_count(&self) -> usize {
        self.peers
            .values()
            .filter(|p| p.direction == Direction::Inbound)
            .count()
    }

    /// Get the number of outbound peers.
    pub fn outbound_count(&self) -> usize {
        self.peers
            .values()
            .filter(|p| p.direction == Direction::Outbound)
            .count()
    }

    /// Check if a peer is connected.
    pub fn is_connected(&self, peer_id: &str) -> bool {
        self.peers.contains_key(peer_id)
    }

    /// Check if a peer is banned.
    pub fn is_banned(&self, peer_id: &str) -> bool {
        if let Some(&expiry) = self.banned.get(peer_id) {
            if expiry == 0 || self.current_tick < expiry {
                return true;
            }
        }
        false
    }

    /// Get peer info.
    pub fn get_peer(&self, peer_id: &str) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    /// Get the best peers (highest score) up to a limit.
    pub fn best_peers(&self, limit: usize) -> Vec<&PeerInfo> {
        let mut peers: Vec<&PeerInfo> = self.peers.values().collect();
        peers.sort_by(|a, b| b.score.cmp(&a.score));
        peers.truncate(limit);
        peers
    }

    /// Get all peer IDs.
    pub fn peer_ids(&self) -> Vec<String> {
        self.peers.keys().cloned().collect()
    }

    /// Register a new peer connection.
    ///
    /// Returns `false` if the connection is rejected (banned, at capacity, etc.).
    pub fn on_peer_connected(&mut self, peer_id: &str, direction: Direction) -> bool {
        // Check if banned.
        if self.is_banned(peer_id) {
            warn!("Rejecting banned peer: {}", peer_id);
            return false;
        }

        // Check capacity.
        if self.peers.len() >= self.config.max_peers {
            // Try to evict the worst peer.
            if !self.evict_worst_peer() {
                warn!("Rejecting peer {}: at capacity", peer_id);
                return false;
            }
        }

        // Check direction limits.
        match direction {
            Direction::Inbound => {
                if self.inbound_count() >= self.config.max_inbound {
                    warn!("Rejecting inbound peer {}: at inbound limit", peer_id);
                    return false;
                }
            }
            Direction::Outbound => {
                if self.outbound_count() >= self.config.max_outbound {
                    warn!("Rejecting outbound peer {}: at outbound limit", peer_id);
                    return false;
                }
            }
        }

        info!("Peer connected: {} ({:?})", peer_id, direction);

        self.peers.insert(
            peer_id.to_string(),
            PeerInfo {
                peer_id: peer_id.to_string(),
                score: INITIAL_SCORE,
                direction,
                best_block: 0,
                chain_id: 0,
                connected_tick: self.current_tick,
                last_activity_tick: self.current_tick,
                good_messages: 0,
                bad_messages: 0,
                disconnect_pending: false,
            },
        );

        true
    }

    /// Handle peer disconnection.
    pub fn on_peer_disconnected(&mut self, peer_id: &str) {
        if let Some(peer) = self.peers.remove(peer_id) {
            info!(
                "Peer disconnected: {} (score: {})",
                peer_id, peer.score
            );
        }
    }

    /// Record a scoring event for a peer.
    pub fn record_event(&mut self, peer_id: &str, event: ScoreEvent) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            let delta = event.score_delta();
            peer.score = (peer.score + delta).clamp(-MAX_SCORE, MAX_SCORE);
            peer.last_activity_tick = self.current_tick;

            if delta > 0 {
                peer.good_messages += 1;
            } else if delta < 0 {
                peer.bad_messages += 1;
            }

            debug!(
                "Peer {} score: {} ({:?}, delta={})",
                peer_id, peer.score, event, delta
            );

            // Check thresholds.
            if peer.score <= BAN_THRESHOLD {
                warn!("Banning peer {} (score: {})", peer_id, peer.score);
                self.banned.insert(peer_id.to_string(), 0); // permanent ban
                peer.disconnect_pending = true;
            } else if peer.score <= DISCONNECT_THRESHOLD {
                warn!("Disconnecting peer {} (score: {})", peer_id, peer.score);
                peer.disconnect_pending = true;
            }
        }
    }

    /// Update a peer's status information.
    pub fn update_peer_status(&mut self, peer_id: &str, best_block: u64, chain_id: u64) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.best_block = best_block;
            peer.chain_id = chain_id;
            peer.last_activity_tick = self.current_tick;

            // Check chain ID mismatch.
            if chain_id != 0 && chain_id != self.config.chain_id {
                self.record_event(peer_id, ScoreEvent::WrongChainId);
            }
        }
    }

    /// Get peers that should be disconnected (pending disconnect + idle peers).
    pub fn peers_to_disconnect(&self) -> Vec<String> {
        self.peers
            .values()
            .filter(|p| {
                p.disconnect_pending
                    || (self.current_tick - p.last_activity_tick > self.config.idle_timeout_ticks)
            })
            .map(|p| p.peer_id.clone())
            .collect()
    }

    /// Evict the worst-scoring peer. Returns true if a peer was evicted.
    fn evict_worst_peer(&mut self) -> bool {
        let worst = self
            .peers
            .values()
            .min_by_key(|p| p.score)
            .map(|p| p.peer_id.clone());

        if let Some(peer_id) = worst {
            info!("Evicting worst peer: {}", peer_id);
            self.peers.remove(&peer_id);
            true
        } else {
            false
        }
    }

    /// Tick: decay scores and clean up expired bans.
    pub fn tick(&mut self) {
        self.current_tick += 1;

        // Decay scores periodically.
        if self.current_tick - self.last_decay_tick >= self.config.decay_interval_ticks {
            self.last_decay_tick = self.current_tick;
            let decay = self.config.decay_amount;
            for peer in self.peers.values_mut() {
                if peer.score > INITIAL_SCORE {
                    peer.score = (peer.score - decay).max(INITIAL_SCORE);
                } else if peer.score < INITIAL_SCORE {
                    peer.score = (peer.score + decay).min(INITIAL_SCORE);
                }
            }
        }

        // Clean up expired bans.
        let tick = self.current_tick;
        self.banned.retain(|_, expiry| *expiry == 0 || tick < *expiry);
    }

    /// Get a snapshot of all peer scores.
    pub fn score_snapshot(&self) -> Vec<(String, Score)> {
        self.peers
            .values()
            .map(|p| (p.peer_id.clone(), p.score))
            .collect()
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new(PeerManagerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PeerManagerConfig {
        PeerManagerConfig {
            max_peers: 5,
            max_inbound: 3,
            max_outbound: 3,
            idle_timeout_ticks: 10,
            decay_interval_ticks: 5,
            decay_amount: 5,
            chain_id: 1,
        }
    }

    #[test]
    fn test_new_peer_manager() {
        let pm = PeerManager::new(default_config());
        assert_eq!(pm.peer_count(), 0);
        assert_eq!(pm.inbound_count(), 0);
        assert_eq!(pm.outbound_count(), 0);
    }

    #[test]
    fn test_connect_peer() {
        let mut pm = PeerManager::new(default_config());
        assert!(pm.on_peer_connected("peer1", Direction::Inbound));
        assert_eq!(pm.peer_count(), 1);
        assert!(pm.is_connected("peer1"));
        assert_eq!(pm.get_peer("peer1").unwrap().score, INITIAL_SCORE);
    }

    #[test]
    fn test_disconnect_peer() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);
        pm.on_peer_disconnected("peer1");
        assert_eq!(pm.peer_count(), 0);
        assert!(!pm.is_connected("peer1"));
    }

    #[test]
    fn test_max_peers_rejection() {
        let mut pm = PeerManager::new(default_config());
        // Fill with a mix of inbound and outbound to respect direction limits.
        pm.on_peer_connected("in0", Direction::Inbound);
        pm.on_peer_connected("in1", Direction::Inbound);
        pm.on_peer_connected("in2", Direction::Inbound);
        pm.on_peer_connected("out0", Direction::Outbound);
        pm.on_peer_connected("out1", Direction::Outbound);
        assert_eq!(pm.peer_count(), 5);
        // 6th outbound peer should evict worst and still connect.
        assert!(pm.on_peer_connected("out2", Direction::Outbound));
        assert_eq!(pm.peer_count(), 5);
    }

    #[test]
    fn test_inbound_limit() {
        let mut pm = PeerManager::new(default_config());
        for i in 0..3 {
            pm.on_peer_connected(&format!("in{i}"), Direction::Inbound);
        }
        assert!(!pm.on_peer_connected("in3", Direction::Inbound));
        assert_eq!(pm.inbound_count(), 3);
    }

    #[test]
    fn test_outbound_limit() {
        let mut pm = PeerManager::new(default_config());
        for i in 0..3 {
            pm.on_peer_connected(&format!("out{i}"), Direction::Outbound);
        }
        assert!(!pm.on_peer_connected("out3", Direction::Outbound));
        assert_eq!(pm.outbound_count(), 3);
    }

    #[test]
    fn test_score_events() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);

        pm.record_event("peer1", ScoreEvent::ValidBlock);
        assert_eq!(pm.get_peer("peer1").unwrap().score, INITIAL_SCORE + 10);
        assert_eq!(pm.get_peer("peer1").unwrap().good_messages, 1);

        pm.record_event("peer1", ScoreEvent::Timeout);
        assert_eq!(
            pm.get_peer("peer1").unwrap().score,
            INITIAL_SCORE + 10 - 10
        );
        assert_eq!(pm.get_peer("peer1").unwrap().bad_messages, 1);
    }

    #[test]
    fn test_disconnect_threshold() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);

        // Drive score below disconnect threshold.
        for _ in 0..20 {
            pm.record_event("peer1", ScoreEvent::InvalidBlock);
        }

        let peer = pm.get_peer("peer1").unwrap();
        assert!(peer.disconnect_pending);
    }

    #[test]
    fn test_ban_threshold() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);

        // Drive score way down.
        for _ in 0..10 {
            pm.record_event("peer1", ScoreEvent::InvalidBlock);
        }

        assert!(pm.is_banned("peer1"));

        // Banned peer cannot reconnect.
        pm.on_peer_disconnected("peer1");
        assert!(!pm.on_peer_connected("peer1", Direction::Inbound));
    }

    #[test]
    fn test_idle_timeout() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);

        // Advance ticks past idle timeout.
        for _ in 0..11 {
            pm.tick();
        }

        let to_disconnect = pm.peers_to_disconnect();
        assert!(to_disconnect.contains(&"peer1".to_string()));
    }

    #[test]
    fn test_score_decay() {
        let config = default_config();
        let mut pm = PeerManager::new(config);
        pm.on_peer_connected("peer1", Direction::Inbound);

        // Boost score above initial.
        for _ in 0..5 {
            pm.record_event("peer1", ScoreEvent::ValidBlock);
        }
        let boosted = pm.get_peer("peer1").unwrap().score;
        assert!(boosted > INITIAL_SCORE);

        // Decay ticks.
        for _ in 0..6 {
            pm.tick();
        }
        let decayed = pm.get_peer("peer1").unwrap().score;
        assert!(decayed < boosted);
    }

    #[test]
    fn test_best_peers() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);
        pm.on_peer_connected("peer2", Direction::Outbound);
        pm.record_event("peer2", ScoreEvent::ValidBlock);

        let best = pm.best_peers(1);
        assert_eq!(best.len(), 1);
        assert_eq!(best[0].peer_id, "peer2");
    }

    #[test]
    fn test_update_peer_status() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);
        pm.update_peer_status("peer1", 100, 1);

        let peer = pm.get_peer("peer1").unwrap();
        assert_eq!(peer.best_block, 100);
        assert_eq!(peer.chain_id, 1);
    }

    #[test]
    fn test_wrong_chain_id_penalized() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);
        pm.update_peer_status("peer1", 100, 999); // wrong chain ID

        let peer = pm.get_peer("peer1").unwrap();
        assert!(peer.score < INITIAL_SCORE);
    }

    #[test]
    fn test_score_snapshot() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("peer1", Direction::Inbound);
        pm.on_peer_connected("peer2", Direction::Outbound);

        let snapshot = pm.score_snapshot();
        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn test_peer_ids() {
        let mut pm = PeerManager::new(default_config());
        pm.on_peer_connected("a", Direction::Inbound);
        pm.on_peer_connected("b", Direction::Outbound);

        let mut ids = pm.peer_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }
}
