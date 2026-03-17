//! Chain data pruning.
//!
//! Automatically deletes old block headers, bodies, receipts, and transaction
//! indices to bound disk usage. Keeps a configurable number of recent blocks.

use std::collections::VecDeque;
use tracing::debug;

/// Pruning mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruningMode {
    /// Keep all data (archive node).
    Archive,
    /// Keep only the last N blocks.
    KeepRecent(u64),
    /// Keep only headers for old blocks (light pruning).
    LightPrune(u64),
}

/// What data to prune for a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneTarget {
    /// Block number to prune.
    pub block_number: u64,
    /// Block hash (for header/body/receipt lookup).
    pub block_hash: [u8; 32],
    /// Transaction hashes in this block (for tx index cleanup).
    pub tx_hashes: Vec<[u8; 32]>,
    /// Whether to prune the header too (false for light prune).
    pub prune_header: bool,
}

/// Result of a pruning operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PruneResult {
    /// Blocks were pruned.
    Pruned {
        /// Number of blocks pruned.
        blocks_pruned: u64,
        /// Lowest remaining block number.
        lowest_remaining: u64,
    },
    /// Nothing to prune.
    NothingToPrune,
    /// Pruning is disabled (archive mode).
    Disabled,
    /// Error during pruning.
    Error(String),
}

/// Statistics about the pruning state.
#[derive(Debug, Clone, Default)]
pub struct PruneStats {
    /// Total blocks pruned since startup.
    pub total_pruned: u64,
    /// Last block number that was pruned.
    pub last_pruned_block: u64,
    /// Lowest block number still available.
    pub lowest_available: u64,
    /// Current chain head.
    pub chain_head: u64,
}

/// Configuration for the pruning engine.
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Pruning mode.
    pub mode: PruningMode,
    /// Maximum number of blocks to prune in a single batch.
    pub batch_size: u64,
    /// How often (in blocks) to run pruning.
    pub interval_blocks: u64,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            mode: PruningMode::KeepRecent(10_000),
            batch_size: 100,
            interval_blocks: 10,
        }
    }
}

/// The pruning engine.
///
/// Tracks the chain head and generates prune targets for blocks
/// that fall outside the retention window.
pub struct PruningEngine {
    /// Configuration.
    config: PruningConfig,
    /// Current chain head block number.
    chain_head: u64,
    /// Lowest block number still in the database.
    lowest_block: u64,
    /// Statistics.
    stats: PruneStats,
    /// Queue of blocks pending pruning (block_number, block_hash).
    pending_queue: VecDeque<(u64, [u8; 32])>,
    /// Last block at which pruning was run.
    last_prune_block: u64,
}

impl PruningEngine {
    /// Create a new pruning engine.
    pub fn new(config: PruningConfig, lowest_block: u64) -> Self {
        Self {
            config,
            chain_head: lowest_block,
            lowest_block,
            stats: PruneStats {
                lowest_available: lowest_block,
                ..Default::default()
            },
            pending_queue: VecDeque::new(),
            last_prune_block: 0,
        }
    }

    /// Get the pruning mode.
    pub fn mode(&self) -> PruningMode {
        self.config.mode
    }

    /// Get the current chain head.
    pub fn chain_head(&self) -> u64 {
        self.chain_head
    }

    /// Get the lowest available block.
    pub fn lowest_block(&self) -> u64 {
        self.lowest_block
    }

    /// Get pruning statistics.
    pub fn stats(&self) -> &PruneStats {
        &self.stats
    }

    /// Get the retention limit (number of blocks to keep).
    pub fn retention_limit(&self) -> Option<u64> {
        match self.config.mode {
            PruningMode::Archive => None,
            PruningMode::KeepRecent(n) | PruningMode::LightPrune(n) => Some(n),
        }
    }

    /// Update the chain head and check if pruning should run.
    ///
    /// Returns a list of `PruneTarget` blocks to prune, if any.
    pub fn on_new_block(&mut self, block_number: u64, _block_hash: [u8; 32]) -> Vec<PruneTarget> {
        self.chain_head = block_number;
        self.stats.chain_head = block_number;

        if self.config.mode == PruningMode::Archive {
            return Vec::new();
        }

        // Check interval.
        if block_number - self.last_prune_block < self.config.interval_blocks {
            return Vec::new();
        }

        self.last_prune_block = block_number;
        self.generate_prune_targets()
    }

    /// Generate prune targets for blocks outside the retention window.
    fn generate_prune_targets(&mut self) -> Vec<PruneTarget> {
        let keep = match self.config.mode {
            PruningMode::Archive => return Vec::new(),
            PruningMode::KeepRecent(n) | PruningMode::LightPrune(n) => n,
        };

        let prune_header = matches!(self.config.mode, PruningMode::KeepRecent(_));

        if self.chain_head <= keep {
            return Vec::new();
        }

        let cutoff = self.chain_head - keep;
        let mut targets = Vec::new();
        let mut count = 0u64;

        while self.lowest_block < cutoff && count < self.config.batch_size {
            // First check pending queue for known hashes.
            if let Some((num, hash)) = self.pending_queue.pop_front() {
                if num < cutoff {
                    targets.push(PruneTarget {
                        block_number: num,
                        block_hash: hash,
                        tx_hashes: Vec::new(), // caller fills in from DB
                        prune_header,
                    });
                    self.lowest_block = num + 1;
                    count += 1;
                    continue;
                } else {
                    // Put it back; not ready yet.
                    self.pending_queue.push_front((num, hash));
                    break;
                }
            }

            // No more queued blocks; generate synthetic targets.
            // The caller is responsible for looking up the actual hash from DB.
            targets.push(PruneTarget {
                block_number: self.lowest_block,
                block_hash: [0u8; 32], // placeholder - caller fills from DB
                tx_hashes: Vec::new(),
                prune_header,
            });
            self.lowest_block += 1;
            count += 1;
        }

        if !targets.is_empty() {
            debug!(
                "Generated {} prune targets (blocks {} to {})",
                targets.len(),
                targets.first().unwrap().block_number,
                targets.last().unwrap().block_number,
            );
        }

        targets
    }

    /// Record that a block has been successfully pruned.
    pub fn on_block_pruned(&mut self, block_number: u64) {
        self.stats.total_pruned += 1;
        self.stats.last_pruned_block = block_number;
        self.stats.lowest_available = self.lowest_block;
    }

    /// Enqueue a block for future pruning (when its hash is known).
    pub fn enqueue_block(&mut self, block_number: u64, block_hash: [u8; 32]) {
        self.pending_queue.push_back((block_number, block_hash));
    }

    /// Check if a given block number is within the retention window.
    pub fn is_retained(&self, block_number: u64) -> bool {
        match self.config.mode {
            PruningMode::Archive => true,
            PruningMode::KeepRecent(n) | PruningMode::LightPrune(n) => {
                if self.chain_head < n {
                    true
                } else {
                    block_number >= self.chain_head - n
                }
            }
        }
    }

    /// Get the number of blocks pending in the prune queue.
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }

    /// Estimate the number of blocks that can be pruned right now.
    pub fn prunable_count(&self) -> u64 {
        let keep = match self.config.mode {
            PruningMode::Archive => return 0,
            PruningMode::KeepRecent(n) | PruningMode::LightPrune(n) => n,
        };

        if self.chain_head <= keep {
            0
        } else {
            let cutoff = self.chain_head - keep;
            cutoff.saturating_sub(self.lowest_block)
        }
    }
}

impl Default for PruningEngine {
    fn default() -> Self {
        Self::new(PruningConfig::default(), 0)
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
    fn test_archive_mode() {
        let config = PruningConfig {
            mode: PruningMode::Archive,
            ..Default::default()
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(100_000, make_hash(1));
        assert!(targets.is_empty());
        assert!(engine.is_retained(0));
        assert_eq!(engine.prunable_count(), 0);
    }

    #[test]
    fn test_keep_recent_no_prune_below_threshold() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(100),
            interval_blocks: 1,
            ..Default::default()
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(50, make_hash(1));
        assert!(targets.is_empty());
    }

    #[test]
    fn test_keep_recent_generates_targets() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(10),
            batch_size: 5,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(20, make_hash(1));
        assert_eq!(targets.len(), 5);
        assert_eq!(targets[0].block_number, 0);
        assert_eq!(targets[4].block_number, 4);
        assert!(targets[0].prune_header);
    }

    #[test]
    fn test_light_prune_keeps_headers() {
        let config = PruningConfig {
            mode: PruningMode::LightPrune(10),
            batch_size: 5,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(20, make_hash(1));
        assert!(!targets.is_empty());
        assert!(!targets[0].prune_header);
    }

    #[test]
    fn test_batch_size_limit() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(10),
            batch_size: 3,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(100, make_hash(1));
        assert_eq!(targets.len(), 3);
    }

    #[test]
    fn test_incremental_pruning() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(5),
            batch_size: 100,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);

        // Block 10: should prune 0-4.
        let targets = engine.on_new_block(10, make_hash(10));
        assert_eq!(targets.len(), 5);
        for t in &targets {
            engine.on_block_pruned(t.block_number);
        }

        // Block 15: should prune 5-9.
        let targets = engine.on_new_block(15, make_hash(15));
        assert_eq!(targets.len(), 5);
        assert_eq!(targets[0].block_number, 5);
    }

    #[test]
    fn test_is_retained() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(10),
            ..Default::default()
        };
        let mut engine = PruningEngine::new(config, 0);
        engine.chain_head = 50;

        assert!(!engine.is_retained(39));
        assert!(engine.is_retained(40));
        assert!(engine.is_retained(50));
    }

    #[test]
    fn test_enqueue_block() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(5),
            batch_size: 100,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);

        // Enqueue known blocks.
        for i in 0u8..10 {
            engine.enqueue_block(i as u64, make_hash(i));
        }
        assert_eq!(engine.pending_count(), 10);

        // Trigger pruning.
        let targets = engine.on_new_block(10, make_hash(99));
        assert_eq!(targets.len(), 5);
        assert_eq!(targets[0].block_hash, make_hash(0));
        assert_eq!(targets[4].block_hash, make_hash(4));
    }

    #[test]
    fn test_prunable_count() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(10),
            ..Default::default()
        };
        let mut engine = PruningEngine::new(config, 0);
        engine.chain_head = 50;
        assert_eq!(engine.prunable_count(), 40);
    }

    #[test]
    fn test_stats_tracking() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(5),
            batch_size: 100,
            interval_blocks: 1,
        };
        let mut engine = PruningEngine::new(config, 0);
        let targets = engine.on_new_block(10, make_hash(1));

        for t in &targets {
            engine.on_block_pruned(t.block_number);
        }

        let stats = engine.stats();
        assert_eq!(stats.total_pruned, 5);
        assert_eq!(stats.last_pruned_block, 4);
        assert_eq!(stats.chain_head, 10);
    }

    #[test]
    fn test_interval_blocks() {
        let config = PruningConfig {
            mode: PruningMode::KeepRecent(5),
            batch_size: 100,
            interval_blocks: 10,
        };
        let mut engine = PruningEngine::new(config, 0);

        // Block 15: first check, should prune.
        let targets = engine.on_new_block(15, make_hash(1));
        assert!(!targets.is_empty());

        for t in &targets {
            engine.on_block_pruned(t.block_number);
        }

        // Block 16: too soon, should not prune.
        let targets = engine.on_new_block(16, make_hash(2));
        assert!(targets.is_empty());

        // Block 25: interval passed, should prune.
        let targets = engine.on_new_block(25, make_hash(3));
        assert!(!targets.is_empty());
    }

    #[test]
    fn test_retention_limit() {
        let archive = PruningEngine::new(
            PruningConfig {
                mode: PruningMode::Archive,
                ..Default::default()
            },
            0,
        );
        assert_eq!(archive.retention_limit(), None);

        let recent = PruningEngine::new(
            PruningConfig {
                mode: PruningMode::KeepRecent(1000),
                ..Default::default()
            },
            0,
        );
        assert_eq!(recent.retention_limit(), Some(1000));
    }

    #[test]
    fn test_default_engine() {
        let engine = PruningEngine::default();
        assert_eq!(engine.mode(), PruningMode::KeepRecent(10_000));
        assert_eq!(engine.chain_head(), 0);
        assert_eq!(engine.lowest_block(), 0);
    }
}
