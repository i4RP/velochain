//! Block synchronization protocol.
//!
//! Implements header-first sync: download headers, verify chain, then download bodies.
//! Supports range-based sync and gap detection.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, info, warn};

/// Unique identifier for a sync request.
pub type RequestId = u64;

/// State of the sync engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Node is synced with the network.
    Synced,
    /// Downloading block headers.
    DownloadingHeaders,
    /// Downloading block bodies.
    DownloadingBodies,
    /// Verifying downloaded chain data.
    Verifying,
    /// Sync stalled (no progress).
    Stalled,
}

/// A sync target representing the best known chain from peers.
#[derive(Debug, Clone)]
pub struct SyncTarget {
    /// Best known block number in the network.
    pub best_block: u64,
    /// Hash of the best known block.
    pub best_hash: [u8; 32],
    /// Peer that reported this target.
    pub peer_id: String,
}

/// A pending header request.
#[derive(Debug, Clone)]
pub struct HeaderRequest {
    /// Starting block number.
    pub start: u64,
    /// Maximum number of headers to fetch.
    pub count: u64,
    /// Peer we sent the request to.
    pub peer_id: String,
    /// Tick when the request was created.
    pub created_tick: u64,
    /// Number of retries.
    pub retries: u32,
}

/// A pending body request.
#[derive(Debug, Clone)]
pub struct BodyRequest {
    /// Block hashes to fetch bodies for.
    pub hashes: Vec<[u8; 32]>,
    /// Peer we sent the request to.
    pub peer_id: String,
    /// Tick when the request was created.
    pub created_tick: u64,
    /// Number of retries.
    pub retries: u32,
}

/// A downloaded header awaiting body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedHeader {
    /// Block number.
    pub number: u64,
    /// Block hash.
    pub hash: [u8; 32],
    /// Parent hash.
    pub parent_hash: [u8; 32],
    /// Whether the body has been downloaded.
    pub body_downloaded: bool,
    /// Whether the header has been verified.
    pub verified: bool,
}

/// Result of a sync operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncResult {
    /// Headers received and queued.
    HeadersQueued(u64),
    /// Bodies received and matched.
    BodiesMatched(usize),
    /// Block range verified and ready for import.
    RangeVerified { from: u64, to: u64 },
    /// Sync completed (caught up with target).
    Completed,
    /// An error occurred.
    Error(SyncError),
}

/// Sync errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// No peers available for sync.
    NoPeers,
    /// Header chain is broken (parent mismatch).
    BrokenChain { at_block: u64 },
    /// Request timed out.
    Timeout { request_id: RequestId },
    /// Duplicate block received.
    DuplicateBlock(u64),
    /// Invalid header received.
    InvalidHeader(String),
    /// Max retries exceeded.
    MaxRetries { request_id: RequestId },
}

/// Configuration for the sync engine.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Number of headers to request per batch.
    pub headers_per_request: u64,
    /// Number of body hashes to request per batch.
    pub bodies_per_request: usize,
    /// Timeout in ticks before retrying a request.
    pub request_timeout_ticks: u64,
    /// Maximum number of retries per request.
    pub max_retries: u32,
    /// Maximum number of concurrent header requests.
    pub max_header_requests: usize,
    /// Maximum number of concurrent body requests.
    pub max_body_requests: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            headers_per_request: 256,
            bodies_per_request: 64,
            request_timeout_ticks: 30,
            max_retries: 3,
            max_header_requests: 4,
            max_body_requests: 8,
        }
    }
}

/// The block sync engine.
///
/// Manages header-first synchronization:
/// 1. Discover best chain from peers
/// 2. Download headers in batches
/// 3. Verify header chain continuity
/// 4. Download bodies for verified headers
/// 5. Import complete blocks
pub struct SyncEngine {
    /// Current sync state.
    state: SyncState,
    /// Our local chain head.
    local_head: u64,
    /// Best sync target from peers.
    target: Option<SyncTarget>,
    /// Downloaded headers indexed by block number.
    headers: BTreeMap<u64, SyncedHeader>,
    /// Pending header requests.
    header_requests: HashMap<RequestId, HeaderRequest>,
    /// Pending body requests.
    body_requests: HashMap<RequestId, BodyRequest>,
    /// Next request ID.
    next_request_id: RequestId,
    /// Current tick for timeout tracking.
    current_tick: u64,
    /// Configuration.
    config: SyncConfig,
    /// Blocks that have been fully verified and are ready for import.
    import_ready: Vec<u64>,
    /// Set of peers that have been penalized (bad data).
    banned_peers: HashSet<String>,
}

impl SyncEngine {
    /// Create a new sync engine.
    pub fn new(local_head: u64, config: SyncConfig) -> Self {
        Self {
            state: SyncState::Synced,
            local_head,
            target: None,
            headers: BTreeMap::new(),
            header_requests: HashMap::new(),
            body_requests: HashMap::new(),
            next_request_id: 1,
            current_tick: 0,
            config,
            import_ready: Vec::new(),
            banned_peers: HashSet::new(),
        }
    }

    /// Get the current sync state.
    pub fn state(&self) -> SyncState {
        self.state
    }

    /// Get the local chain head.
    pub fn local_head(&self) -> u64 {
        self.local_head
    }

    /// Get the sync target.
    pub fn target(&self) -> Option<&SyncTarget> {
        self.target.as_ref()
    }

    /// Number of downloaded headers.
    pub fn header_count(&self) -> usize {
        self.headers.len()
    }

    /// Number of blocks ready for import.
    pub fn import_ready_count(&self) -> usize {
        self.import_ready.len()
    }

    /// Drain blocks ready for import.
    pub fn drain_import_ready(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.import_ready)
    }

    /// Check if a peer is banned.
    pub fn is_peer_banned(&self, peer_id: &str) -> bool {
        self.banned_peers.contains(peer_id)
    }

    /// Ban a peer for sending bad data.
    pub fn ban_peer(&mut self, peer_id: &str) {
        warn!("Banning peer for bad sync data: {}", peer_id);
        self.banned_peers.insert(peer_id.to_string());
    }

    /// Update the sync target from a peer's status message.
    pub fn on_peer_status(&mut self, peer_id: &str, best_block: u64, best_hash: [u8; 32]) {
        if best_block <= self.local_head {
            return;
        }

        let should_update = match &self.target {
            Some(t) => best_block > t.best_block,
            None => true,
        };

        if should_update {
            info!(
                "New sync target: block {} from peer {}",
                best_block, peer_id
            );
            self.target = Some(SyncTarget {
                best_block,
                best_hash,
                peer_id: peer_id.to_string(),
            });
            if self.state == SyncState::Synced {
                self.state = SyncState::DownloadingHeaders;
            }
        }
    }

    /// Update the local head after importing blocks.
    pub fn update_local_head(&mut self, new_head: u64) {
        self.local_head = new_head;

        // Remove headers at or below the new head.
        let to_remove: Vec<u64> = self
            .headers
            .range(..=new_head)
            .map(|(&k, _)| k)
            .collect();
        for num in to_remove {
            self.headers.remove(&num);
        }

        // Check if we caught up.
        if let Some(ref target) = self.target {
            if new_head >= target.best_block {
                info!("Sync completed at block {}", new_head);
                self.state = SyncState::Synced;
                self.target = None;
            }
        }
    }

    /// Generate header download requests for the next batch.
    ///
    /// Returns a list of (request_id, start_block, count, preferred_peer) tuples.
    pub fn poll_header_requests(&mut self) -> Vec<(RequestId, u64, u64, Option<String>)> {
        if self.state != SyncState::DownloadingHeaders {
            return Vec::new();
        }

        let target_block = match &self.target {
            Some(t) => t.best_block,
            None => return Vec::new(),
        };

        let mut requests = Vec::new();
        let max_new = self.config.max_header_requests.saturating_sub(self.header_requests.len());

        // Find the highest block we've requested or downloaded.
        let highest_requested = self
            .header_requests
            .values()
            .map(|r| r.start + r.count)
            .max()
            .unwrap_or(0);
        let highest_downloaded = self.headers.keys().last().copied().unwrap_or(0);
        let mut start = std::cmp::max(highest_requested, highest_downloaded)
            .max(self.local_head + 1);

        for _ in 0..max_new {
            if start > target_block {
                break;
            }
            let count = std::cmp::min(self.config.headers_per_request, target_block - start + 1);
            let request_id = self.next_request_id;
            self.next_request_id += 1;

            let peer = self.target.as_ref().map(|t| t.peer_id.clone());

            self.header_requests.insert(
                request_id,
                HeaderRequest {
                    start,
                    count,
                    peer_id: peer.clone().unwrap_or_default(),
                    created_tick: self.current_tick,
                    retries: 0,
                },
            );

            requests.push((request_id, start, count, peer));
            start += count;
        }

        requests
    }

    /// Process received headers from a peer.
    pub fn on_headers_received(
        &mut self,
        headers: Vec<(u64, [u8; 32], [u8; 32])>, // (number, hash, parent_hash)
    ) -> SyncResult {
        if headers.is_empty() {
            return SyncResult::HeadersQueued(0);
        }

        let first_num = headers[0].0;
        let count = headers.len() as u64;

        // Remove matching pending request.
        self.header_requests
            .retain(|_, req| req.start != first_num);

        // Store headers.
        for (number, hash, parent_hash) in &headers {
            if self.headers.contains_key(number) {
                continue; // skip duplicates
            }
            self.headers.insert(
                *number,
                SyncedHeader {
                    number: *number,
                    hash: *hash,
                    parent_hash: *parent_hash,
                    body_downloaded: false,
                    verified: false,
                },
            );
        }

        debug!("Received {} headers starting at {}", count, first_num);

        // Check if we have all headers and can move to body download.
        if let Some(ref target) = self.target {
            let have_all = self.headers.keys().last().copied().unwrap_or(0) >= target.best_block;
            if have_all && self.header_requests.is_empty() {
                self.state = SyncState::DownloadingBodies;
            }
        }

        SyncResult::HeadersQueued(count)
    }

    /// Generate body download requests.
    ///
    /// Returns a list of (request_id, hashes, preferred_peer) tuples.
    pub fn poll_body_requests(&mut self) -> Vec<(RequestId, Vec<[u8; 32]>, Option<String>)> {
        if self.state != SyncState::DownloadingBodies {
            return Vec::new();
        }

        let max_new = self.config.max_body_requests.saturating_sub(self.body_requests.len());
        let mut requests = Vec::new();

        // Collect hashes of headers that still need bodies.
        let pending_bodies: Vec<[u8; 32]> = self
            .headers
            .values()
            .filter(|h| !h.body_downloaded)
            .take(max_new * self.config.bodies_per_request)
            .map(|h| h.hash)
            .collect();

        for chunk in pending_bodies.chunks(self.config.bodies_per_request) {
            let request_id = self.next_request_id;
            self.next_request_id += 1;

            let peer = self.target.as_ref().map(|t| t.peer_id.clone());

            self.body_requests.insert(
                request_id,
                BodyRequest {
                    hashes: chunk.to_vec(),
                    peer_id: peer.clone().unwrap_or_default(),
                    created_tick: self.current_tick,
                    retries: 0,
                },
            );

            requests.push((request_id, chunk.to_vec(), peer));
        }

        requests
    }

    /// Process received bodies for a set of block hashes.
    pub fn on_bodies_received(&mut self, hashes: &[[u8; 32]]) -> SyncResult {
        let mut matched = 0usize;

        for hash in hashes {
            for header in self.headers.values_mut() {
                if header.hash == *hash && !header.body_downloaded {
                    header.body_downloaded = true;
                    matched += 1;
                    break;
                }
            }
        }

        // Remove matching pending request.
        let hash_set: HashSet<[u8; 32]> = hashes.iter().copied().collect();
        self.body_requests.retain(|_, req| {
            !req.hashes.iter().all(|h| hash_set.contains(h))
        });

        debug!("Matched {} bodies", matched);

        // Check if all bodies are downloaded.
        let all_downloaded = self.headers.values().all(|h| h.body_downloaded);
        if all_downloaded && self.body_requests.is_empty() && !self.headers.is_empty() {
            self.state = SyncState::Verifying;
        }

        SyncResult::BodiesMatched(matched)
    }

    /// Verify the header chain continuity and mark blocks for import.
    pub fn verify_chain(&mut self) -> SyncResult {
        if self.headers.is_empty() {
            return SyncResult::Error(SyncError::NoPeers);
        }

        let numbers: Vec<u64> = self.headers.keys().copied().collect();
        let first = numbers[0];

        // Verify sequential continuity.
        for window in numbers.windows(2) {
            let (num_a, num_b) = (window[0], window[1]);
            if num_b != num_a + 1 {
                return SyncResult::Error(SyncError::BrokenChain { at_block: num_b });
            }

            let parent_hash = self.headers[&num_b].parent_hash;
            let expected_hash = self.headers[&num_a].hash;
            if parent_hash != expected_hash {
                return SyncResult::Error(SyncError::BrokenChain { at_block: num_b });
            }
        }

        // Mark all as verified.
        let last = *numbers.last().unwrap();
        for header in self.headers.values_mut() {
            header.verified = true;
        }

        // Queue for import.
        self.import_ready.extend(&numbers);
        self.state = SyncState::Synced;

        info!(
            "Verified header chain from {} to {} ({} blocks)",
            first,
            last,
            numbers.len()
        );

        SyncResult::RangeVerified {
            from: first,
            to: last,
        }
    }

    /// Tick the sync engine: handle timeouts and retries.
    pub fn tick(&mut self) -> Vec<SyncResult> {
        self.current_tick += 1;
        let mut results = Vec::new();

        // Check header request timeouts.
        let mut timed_out_headers = Vec::new();
        for (&id, req) in &self.header_requests {
            if self.current_tick - req.created_tick > self.config.request_timeout_ticks {
                timed_out_headers.push(id);
            }
        }

        for id in timed_out_headers {
            if let Some(mut req) = self.header_requests.remove(&id) {
                req.retries += 1;
                if req.retries > self.config.max_retries {
                    results.push(SyncResult::Error(SyncError::MaxRetries {
                        request_id: id,
                    }));
                } else {
                    // Re-queue with updated tick.
                    req.created_tick = self.current_tick;
                    let new_id = self.next_request_id;
                    self.next_request_id += 1;
                    self.header_requests.insert(new_id, req);
                    results.push(SyncResult::Error(SyncError::Timeout {
                        request_id: id,
                    }));
                }
            }
        }

        // Check body request timeouts.
        let mut timed_out_bodies = Vec::new();
        for (&id, req) in &self.body_requests {
            if self.current_tick - req.created_tick > self.config.request_timeout_ticks {
                timed_out_bodies.push(id);
            }
        }

        for id in timed_out_bodies {
            if let Some(mut req) = self.body_requests.remove(&id) {
                req.retries += 1;
                if req.retries > self.config.max_retries {
                    results.push(SyncResult::Error(SyncError::MaxRetries {
                        request_id: id,
                    }));
                } else {
                    req.created_tick = self.current_tick;
                    let new_id = self.next_request_id;
                    self.next_request_id += 1;
                    self.body_requests.insert(new_id, req);
                    results.push(SyncResult::Error(SyncError::Timeout {
                        request_id: id,
                    }));
                }
            }
        }

        // Detect stall.
        if self.state != SyncState::Synced
            && self.header_requests.is_empty()
            && self.body_requests.is_empty()
            && self.headers.is_empty()
            && self.target.is_some()
        {
            self.state = SyncState::Stalled;
        }

        results
    }

    /// Get pending header request count.
    pub fn pending_header_requests(&self) -> usize {
        self.header_requests.len()
    }

    /// Get pending body request count.
    pub fn pending_body_requests(&self) -> usize {
        self.body_requests.len()
    }
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::new(0, SyncConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hash(n: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = n;
        h
    }

    #[test]
    fn test_new_sync_engine() {
        let engine = SyncEngine::new(0, SyncConfig::default());
        assert_eq!(engine.state(), SyncState::Synced);
        assert_eq!(engine.local_head(), 0);
        assert!(engine.target().is_none());
    }

    #[test]
    fn test_peer_status_triggers_sync() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        engine.on_peer_status("peer1", 100, make_hash(1));
        assert_eq!(engine.state(), SyncState::DownloadingHeaders);
        assert_eq!(engine.target().unwrap().best_block, 100);
    }

    #[test]
    fn test_peer_status_no_update_if_behind() {
        let mut engine = SyncEngine::new(100, SyncConfig::default());
        engine.on_peer_status("peer1", 50, make_hash(1));
        assert_eq!(engine.state(), SyncState::Synced);
        assert!(engine.target().is_none());
    }

    #[test]
    fn test_peer_status_updates_higher_target() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        engine.on_peer_status("peer1", 100, make_hash(1));
        engine.on_peer_status("peer2", 200, make_hash(2));
        assert_eq!(engine.target().unwrap().best_block, 200);
        assert_eq!(engine.target().unwrap().peer_id, "peer2");
    }

    #[test]
    fn test_poll_header_requests() {
        let config = SyncConfig {
            headers_per_request: 10,
            max_header_requests: 2,
            ..Default::default()
        };
        let mut engine = SyncEngine::new(0, config);
        engine.on_peer_status("peer1", 50, make_hash(1));

        let requests = engine.poll_header_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].1, 1); // start at block 1
        assert_eq!(requests[0].2, 10); // 10 headers
        assert_eq!(requests[1].1, 11); // start at block 11
    }

    #[test]
    fn test_headers_received() {
        let config = SyncConfig {
            headers_per_request: 5,
            max_header_requests: 1,
            ..Default::default()
        };
        let mut engine = SyncEngine::new(0, config);
        engine.on_peer_status("peer1", 5, make_hash(99));
        let _ = engine.poll_header_requests();

        let headers: Vec<(u64, [u8; 32], [u8; 32])> = (1..=5)
            .map(|n| (n, make_hash(n as u8), make_hash((n - 1) as u8)))
            .collect();

        let result = engine.on_headers_received(headers);
        assert_eq!(result, SyncResult::HeadersQueued(5));
        assert_eq!(engine.header_count(), 5);
        // Should transition to body download since we have all headers.
        assert_eq!(engine.state(), SyncState::DownloadingBodies);
    }

    #[test]
    fn test_bodies_received() {
        let config = SyncConfig {
            headers_per_request: 3,
            bodies_per_request: 3,
            max_header_requests: 1,
            max_body_requests: 1,
            ..Default::default()
        };
        let mut engine = SyncEngine::new(0, config);
        engine.on_peer_status("peer1", 3, make_hash(99));
        let _ = engine.poll_header_requests();

        let headers: Vec<(u64, [u8; 32], [u8; 32])> = (1..=3)
            .map(|n| (n, make_hash(n as u8), make_hash((n - 1) as u8)))
            .collect();
        engine.on_headers_received(headers);

        let _ = engine.poll_body_requests();
        let hashes: Vec<[u8; 32]> = (1..=3).map(|n| make_hash(n as u8)).collect();
        let result = engine.on_bodies_received(&hashes);
        assert_eq!(result, SyncResult::BodiesMatched(3));
        assert_eq!(engine.state(), SyncState::Verifying);
    }

    #[test]
    fn test_verify_chain_success() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        // Manually insert headers with correct parent chain.
        for n in 1u64..=5 {
            engine.headers.insert(
                n,
                SyncedHeader {
                    number: n,
                    hash: make_hash(n as u8),
                    parent_hash: make_hash((n - 1) as u8),
                    body_downloaded: true,
                    verified: false,
                },
            );
        }

        let result = engine.verify_chain();
        assert_eq!(
            result,
            SyncResult::RangeVerified { from: 1, to: 5 }
        );
        assert_eq!(engine.import_ready_count(), 5);
    }

    #[test]
    fn test_verify_chain_broken() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        engine.headers.insert(
            1,
            SyncedHeader {
                number: 1,
                hash: make_hash(1),
                parent_hash: make_hash(0),
                body_downloaded: true,
                verified: false,
            },
        );
        engine.headers.insert(
            2,
            SyncedHeader {
                number: 2,
                hash: make_hash(2),
                parent_hash: make_hash(99), // wrong parent
                body_downloaded: true,
                verified: false,
            },
        );

        let result = engine.verify_chain();
        assert_eq!(
            result,
            SyncResult::Error(SyncError::BrokenChain { at_block: 2 })
        );
    }

    #[test]
    fn test_update_local_head_removes_old_headers() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        engine.target = Some(SyncTarget {
            best_block: 10,
            best_hash: make_hash(10),
            peer_id: "peer1".into(),
        });
        for n in 1u64..=10 {
            engine.headers.insert(
                n,
                SyncedHeader {
                    number: n,
                    hash: make_hash(n as u8),
                    parent_hash: make_hash((n - 1) as u8),
                    body_downloaded: true,
                    verified: true,
                },
            );
        }

        engine.update_local_head(5);
        assert_eq!(engine.header_count(), 5); // blocks 6-10 remain
        assert_eq!(engine.local_head(), 5);

        engine.update_local_head(10);
        assert_eq!(engine.state(), SyncState::Synced);
        assert!(engine.target().is_none());
    }

    #[test]
    fn test_tick_timeout_and_retry() {
        let config = SyncConfig {
            headers_per_request: 10,
            max_header_requests: 1,
            request_timeout_ticks: 5,
            max_retries: 2,
            ..Default::default()
        };
        let mut engine = SyncEngine::new(0, config);
        engine.on_peer_status("peer1", 20, make_hash(1));
        let _ = engine.poll_header_requests();
        assert_eq!(engine.pending_header_requests(), 1);

        // Tick past timeout.
        for _ in 0..6 {
            engine.tick();
        }
        // Request should have been retried (new request created).
        assert_eq!(engine.pending_header_requests(), 1);
    }

    #[test]
    fn test_ban_peer() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        assert!(!engine.is_peer_banned("peer1"));
        engine.ban_peer("peer1");
        assert!(engine.is_peer_banned("peer1"));
    }

    #[test]
    fn test_drain_import_ready() {
        let mut engine = SyncEngine::new(0, SyncConfig::default());
        engine.import_ready = vec![1, 2, 3];
        let ready = engine.drain_import_ready();
        assert_eq!(ready, vec![1, 2, 3]);
        assert_eq!(engine.import_ready_count(), 0);
    }

    #[test]
    fn test_default_sync_engine() {
        let engine = SyncEngine::default();
        assert_eq!(engine.state(), SyncState::Synced);
        assert_eq!(engine.local_head(), 0);
    }
}
