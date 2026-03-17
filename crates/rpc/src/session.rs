//! Player session management for VeloChain.
//!
//! Tracks connected players, manages session lifecycle (connect, disconnect,
//! heartbeat), and provides session-related RPC endpoints.

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Session information stored server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Unique session identifier.
    pub session_id: String,
    /// Player's on-chain address.
    pub address: String,
    /// Connection timestamp (unix seconds).
    pub connected_at: u64,
    /// Last activity timestamp (unix seconds).
    pub last_activity: u64,
    /// Whether the session is active.
    pub is_active: bool,
}

/// Session manager tracks all active player sessions.
pub struct SessionManager {
    /// Active sessions keyed by session ID.
    sessions: RwLock<HashMap<String, SessionInfo>>,
    /// Map from address to session ID for quick lookup.
    address_index: RwLock<HashMap<String, String>>,
    /// Session timeout in seconds (default: 300 = 5 minutes).
    timeout_secs: u64,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            address_index: RwLock::new(HashMap::new()),
            timeout_secs,
        }
    }

    /// Connect a player, creating a new session.
    pub fn connect(&self, address: &str, _signature: &str) -> String {
        let now = current_timestamp();

        // Check for existing session
        {
            let index = self.address_index.read();
            if let Some(existing_id) = index.get(address) {
                let mut sessions = self.sessions.write();
                if let Some(session) = sessions.get_mut(existing_id) {
                    // Reactivate existing session
                    session.last_activity = now;
                    session.is_active = true;
                    info!(
                        "Session reactivated: address={}, session_id={}",
                        address, existing_id
                    );
                    return existing_id.clone();
                }
            }
        }

        // Create new session
        let session_id = generate_session_id(address, now);
        let session = SessionInfo {
            session_id: session_id.clone(),
            address: address.to_string(),
            connected_at: now,
            last_activity: now,
            is_active: true,
        };

        self.sessions.write().insert(session_id.clone(), session);
        self.address_index
            .write()
            .insert(address.to_string(), session_id.clone());

        info!(
            "New session created: address={}, session_id={}",
            address, session_id
        );
        session_id
    }

    /// Disconnect a player session.
    pub fn disconnect(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write();
        if let Some(session) = sessions.get_mut(session_id) {
            session.is_active = false;
            let address = session.address.clone();
            self.address_index.write().remove(&address);
            info!("Session disconnected: session_id={}", session_id);
            true
        } else {
            warn!("Disconnect failed: session not found: {}", session_id);
            false
        }
    }

    /// Update heartbeat for a session.
    pub fn heartbeat(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write();
        if let Some(session) = sessions.get_mut(session_id) {
            session.last_activity = current_timestamp();
            debug!("Heartbeat: session_id={}", session_id);
            true
        } else {
            false
        }
    }

    /// Get active session count.
    pub fn active_count(&self) -> usize {
        self.sessions
            .read()
            .values()
            .filter(|s| s.is_active)
            .count()
    }

    /// Get session info by session ID.
    pub fn get_session(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions.read().get(session_id).cloned()
    }

    /// Get session by player address.
    pub fn get_session_by_address(&self, address: &str) -> Option<SessionInfo> {
        let index = self.address_index.read();
        let session_id = index.get(address)?;
        self.sessions.read().get(session_id).cloned()
    }

    /// Get all active sessions.
    pub fn get_active_sessions(&self) -> Vec<SessionInfo> {
        self.sessions
            .read()
            .values()
            .filter(|s| s.is_active)
            .cloned()
            .collect()
    }

    /// Clean up timed-out sessions.
    pub fn cleanup_expired(&self) -> usize {
        let now = current_timestamp();
        let timeout = self.timeout_secs;
        let mut sessions = self.sessions.write();
        let mut address_index = self.address_index.write();

        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| s.is_active && (now - s.last_activity) > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in &expired {
            if let Some(session) = sessions.get_mut(id) {
                session.is_active = false;
                address_index.remove(&session.address);
                info!("Session expired: session_id={}", id);
            }
        }

        count
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(300) // 5 minutes
    }
}

/// Session management JSON-RPC API.
#[rpc(server, namespace = "game")]
pub trait SessionApi {
    /// Connect a player to the game world.
    /// Returns a session ID for subsequent requests.
    #[method(name = "connectPlayer")]
    async fn connect_player(&self, address: String, signature: String) -> RpcResult<String>;

    /// Disconnect a player session.
    #[method(name = "disconnectPlayer")]
    async fn disconnect_player(&self, session_id: String) -> RpcResult<bool>;

    /// Send a heartbeat to keep the session alive.
    #[method(name = "heartbeat")]
    async fn heartbeat(&self, session_id: String) -> RpcResult<bool>;

    /// Get the number of active sessions.
    #[method(name = "getActiveSessions")]
    async fn get_active_sessions(&self) -> RpcResult<usize>;

    /// Get session info by session ID.
    #[method(name = "getSessionInfo")]
    async fn get_session_info(&self, session_id: String) -> RpcResult<Option<SessionInfo>>;
}

/// Session API implementation.
pub struct SessionApiImpl {
    session_manager: Arc<SessionManager>,
}

impl SessionApiImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[jsonrpsee::core::async_trait]
impl SessionApiServer for SessionApiImpl {
    async fn connect_player(&self, address: String, signature: String) -> RpcResult<String> {
        let session_id = self.session_manager.connect(&address, &signature);
        Ok(session_id)
    }

    async fn disconnect_player(&self, session_id: String) -> RpcResult<bool> {
        Ok(self.session_manager.disconnect(&session_id))
    }

    async fn heartbeat(&self, session_id: String) -> RpcResult<bool> {
        Ok(self.session_manager.heartbeat(&session_id))
    }

    async fn get_active_sessions(&self) -> RpcResult<usize> {
        Ok(self.session_manager.active_count())
    }

    async fn get_session_info(&self, session_id: String) -> RpcResult<Option<SessionInfo>> {
        Ok(self.session_manager.get_session(&session_id))
    }
}

/// Generate a unique session ID from address and timestamp.
fn generate_session_id(address: &str, timestamp: u64) -> String {
    let input = format!("{}:{}", address, timestamp);
    let hash = Keccak256::digest(input.as_bytes());
    format!("session_{}", hex::encode(&hash[..16]))
}

/// Get current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_connect_disconnect() {
        let manager = SessionManager::new(300);

        let session_id = manager.connect("0xabc123", "sig");
        assert!(!session_id.is_empty());
        assert_eq!(manager.active_count(), 1);

        let disconnected = manager.disconnect(&session_id);
        assert!(disconnected);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_session_reconnect() {
        let manager = SessionManager::new(300);

        let id1 = manager.connect("0xabc123", "sig");
        let id2 = manager.connect("0xabc123", "sig");

        // Same address should reuse session
        assert_eq!(id1, id2);
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_session_heartbeat() {
        let manager = SessionManager::new(300);

        let session_id = manager.connect("0xabc123", "sig");
        assert!(manager.heartbeat(&session_id));
        assert!(!manager.heartbeat("nonexistent"));
    }

    #[test]
    fn test_session_get_by_address() {
        let manager = SessionManager::new(300);

        manager.connect("0xabc123", "sig");
        let session = manager.get_session_by_address("0xabc123");
        assert!(session.is_some());
        assert_eq!(session.unwrap().address, "0xabc123");

        assert!(manager.get_session_by_address("0xnonexistent").is_none());
    }

    #[test]
    fn test_multiple_sessions() {
        let manager = SessionManager::new(300);

        manager.connect("0xaddr1", "sig1");
        manager.connect("0xaddr2", "sig2");
        manager.connect("0xaddr3", "sig3");

        assert_eq!(manager.active_count(), 3);

        let active = manager.get_active_sessions();
        assert_eq!(active.len(), 3);
    }

    #[test]
    fn test_session_disconnect_nonexistent() {
        let manager = SessionManager::new(300);
        assert!(!manager.disconnect("nonexistent"));
    }

    #[test]
    fn test_session_cleanup() {
        let manager = SessionManager::new(0); // 0 second timeout = immediate expiry

        manager.connect("0xaddr1", "sig1");
        assert_eq!(manager.active_count(), 1);

        // Manually set last_activity to a past timestamp to simulate expiry
        {
            let mut sessions = manager.sessions.write();
            for session in sessions.values_mut() {
                session.last_activity = session.last_activity.saturating_sub(2);
            }
        }

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.active_count(), 0);
    }
}
