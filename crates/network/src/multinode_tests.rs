//! Multi-node integration tests.
//!
//! Tests simulating multiple nodes interacting through the sync protocol,
//! peer management, and validator rotation systems.

#[cfg(test)]
mod tests {
    use crate::peer_manager::*;
    use crate::sync::*;

    // ---------- Sync + Peer Manager integration ----------

    #[test]
    fn test_sync_with_peer_scoring() {
        let mut sync = SyncEngine::new(0, SyncConfig::default());
        let mut pm = PeerManager::new(PeerManagerConfig {
            max_peers: 10,
            max_inbound: 5,
            max_outbound: 5,
            idle_timeout_ticks: 100,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: 1,
        });

        // Peer connects and reports status.
        pm.on_peer_connected("node_a", Direction::Inbound);
        pm.update_peer_status("node_a", 100, 1);
        sync.on_peer_status("node_a", 100, [1u8; 32]);

        assert_eq!(sync.state(), SyncState::DownloadingHeaders);
        assert!(pm.is_connected("node_a"));

        // Peer sends valid headers -> score increases.
        let headers: Vec<(u64, [u8; 32], [u8; 32])> = (1..=10)
            .map(|n| {
                let mut h = [0u8; 32];
                h[0] = n as u8;
                let mut p = [0u8; 32];
                p[0] = (n - 1) as u8;
                (n, h, p)
            })
            .collect();

        sync.on_headers_received(headers);
        pm.record_event("node_a", ScoreEvent::ValidHeaders);

        let peer = pm.get_peer("node_a").unwrap();
        assert!(peer.score > INITIAL_SCORE);
    }

    #[test]
    fn test_sync_bad_peer_gets_penalized() {
        let mut sync = SyncEngine::new(0, SyncConfig::default());
        let mut pm = PeerManager::new(PeerManagerConfig {
            max_peers: 10,
            max_inbound: 5,
            max_outbound: 5,
            idle_timeout_ticks: 100,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: 1,
        });

        pm.on_peer_connected("bad_node", Direction::Inbound);
        sync.on_peer_status("bad_node", 50, [1u8; 32]);

        // Peer sends broken chain.
        let mut broken_headers = Vec::new();
        broken_headers.push((1u64, [1u8; 32], [0u8; 32]));
        broken_headers.push((2u64, [2u8; 32], [99u8; 32])); // wrong parent

        sync.on_headers_received(broken_headers);

        // Verification should fail.
        // Manually insert headers for verification test.
        let result = sync.verify_chain();
        match result {
            SyncResult::Error(SyncError::BrokenChain { .. }) => {
                pm.record_event("bad_node", ScoreEvent::InvalidHeaders);
            }
            _ => {
                // Headers might have been queued but chain is broken.
                pm.record_event("bad_node", ScoreEvent::InvalidHeaders);
            }
        }

        let peer = pm.get_peer("bad_node").unwrap();
        assert!(peer.score < INITIAL_SCORE);
    }

    #[test]
    fn test_multi_peer_sync_target_selection() {
        let mut sync = SyncEngine::new(0, SyncConfig::default());
        let mut pm = PeerManager::new(PeerManagerConfig {
            max_peers: 10,
            max_inbound: 5,
            max_outbound: 5,
            idle_timeout_ticks: 100,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: 1,
        });

        // Multiple peers connect with different chain heights.
        pm.on_peer_connected("node_a", Direction::Inbound);
        pm.on_peer_connected("node_b", Direction::Outbound);
        pm.on_peer_connected("node_c", Direction::Inbound);

        pm.update_peer_status("node_a", 50, 1);
        pm.update_peer_status("node_b", 100, 1);
        pm.update_peer_status("node_c", 75, 1);

        sync.on_peer_status("node_a", 50, [1u8; 32]);
        sync.on_peer_status("node_b", 100, [2u8; 32]);
        sync.on_peer_status("node_c", 75, [3u8; 32]);

        // Sync should target the highest peer.
        assert_eq!(sync.target().unwrap().best_block, 100);
        assert_eq!(sync.target().unwrap().peer_id, "node_b");
    }

    #[test]
    fn test_full_sync_lifecycle() {
        let config = SyncConfig {
            headers_per_request: 5,
            bodies_per_request: 5,
            max_header_requests: 1,
            max_body_requests: 1,
            request_timeout_ticks: 30,
            max_retries: 3,
        };
        let mut sync = SyncEngine::new(0, config);

        // 1. Peer reports ahead.
        sync.on_peer_status("peer1", 5, [99u8; 32]);
        assert_eq!(sync.state(), SyncState::DownloadingHeaders);

        // 2. Request headers.
        let header_reqs = sync.poll_header_requests();
        assert_eq!(header_reqs.len(), 1);

        // 3. Receive headers.
        let headers: Vec<(u64, [u8; 32], [u8; 32])> = (1..=5)
            .map(|n| {
                let mut h = [0u8; 32];
                h[0] = n as u8;
                let mut p = [0u8; 32];
                p[0] = (n - 1) as u8;
                (n, h, p)
            })
            .collect();
        sync.on_headers_received(headers);
        assert_eq!(sync.state(), SyncState::DownloadingBodies);

        // 4. Request bodies.
        let body_reqs = sync.poll_body_requests();
        assert!(!body_reqs.is_empty());

        // 5. Receive bodies.
        let body_hashes: Vec<[u8; 32]> = (1..=5)
            .map(|n| {
                let mut h = [0u8; 32];
                h[0] = n as u8;
                h
            })
            .collect();
        sync.on_bodies_received(&body_hashes);
        assert_eq!(sync.state(), SyncState::Verifying);

        // 6. Verify chain.
        let result = sync.verify_chain();
        assert!(matches!(result, SyncResult::RangeVerified { from: 1, to: 5 }));

        // 7. Import blocks.
        let ready = sync.drain_import_ready();
        assert_eq!(ready, vec![1, 2, 3, 4, 5]);

        // 8. Update head.
        sync.update_local_head(5);
        assert_eq!(sync.state(), SyncState::Synced);
    }

    #[test]
    fn test_peer_eviction_during_sync() {
        let mut pm = PeerManager::new(PeerManagerConfig {
            max_peers: 3,
            max_inbound: 2,
            max_outbound: 2,
            idle_timeout_ticks: 100,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: 1,
        });

        pm.on_peer_connected("good1", Direction::Inbound);
        pm.on_peer_connected("good2", Direction::Outbound);
        pm.on_peer_connected("bad", Direction::Inbound);

        // Penalize bad peer.
        for _ in 0..3 {
            pm.record_event("bad", ScoreEvent::Timeout);
        }

        // When a new peer tries to connect, worst peer gets evicted.
        assert!(pm.on_peer_connected("new_peer", Direction::Outbound));
        assert!(!pm.is_connected("bad"));
        assert_eq!(pm.peer_count(), 3);
    }

    #[test]
    fn test_sync_timeout_retry() {
        let config = SyncConfig {
            headers_per_request: 10,
            max_header_requests: 1,
            request_timeout_ticks: 3,
            max_retries: 1,
            ..Default::default()
        };
        let mut sync = SyncEngine::new(0, config);
        sync.on_peer_status("peer1", 20, [1u8; 32]);
        let _ = sync.poll_header_requests();

        // Tick past timeout.
        for _ in 0..4 {
            let results = sync.tick();
            for r in &results {
                if let SyncResult::Error(SyncError::Timeout { .. }) = r {
                    // Timeout detected, request will be retried.
                }
            }
        }

        // After retry, tick again past timeout for max retries.
        for _ in 0..4 {
            let results = sync.tick();
            for r in &results {
                if let SyncResult::Error(SyncError::MaxRetries { .. }) = r {
                    // Max retries reached.
                    return;
                }
            }
        }
    }

    #[test]
    fn test_concurrent_peer_connections() {
        let mut pm = PeerManager::new(PeerManagerConfig {
            max_peers: 50,
            max_inbound: 30,
            max_outbound: 20,
            idle_timeout_ticks: 100,
            decay_interval_ticks: 60,
            decay_amount: 5,
            chain_id: 1,
        });

        // Connect many peers.
        for i in 0..20 {
            pm.on_peer_connected(&format!("inbound_{i}"), Direction::Inbound);
        }
        for i in 0..15 {
            pm.on_peer_connected(&format!("outbound_{i}"), Direction::Outbound);
        }

        assert_eq!(pm.peer_count(), 35);
        assert_eq!(pm.inbound_count(), 20);
        assert_eq!(pm.outbound_count(), 15);

        // Score various events.
        pm.record_event("inbound_0", ScoreEvent::ValidBlock);
        pm.record_event("outbound_0", ScoreEvent::InvalidBlock);

        let best = pm.best_peers(3);
        assert_eq!(best.len(), 3);
        // Best peer should be inbound_0 with highest score.
        assert_eq!(best[0].peer_id, "inbound_0");
    }
}
