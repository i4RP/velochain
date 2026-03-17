//! Prometheus metrics for VeloChain node observability.
//!
//! Exposes counters and gauges for blocks, transactions, peers, and game ticks.

use prometheus::{IntCounter, IntGauge, Histogram, HistogramOpts, Registry};

/// Node-wide metrics collected via Prometheus.
#[derive(Clone)]
pub struct NodeMetrics {
    /// Total number of blocks produced / imported.
    pub blocks_total: IntCounter,
    /// Current chain height (latest block number).
    pub chain_height: IntGauge,
    /// Total transactions processed.
    pub transactions_total: IntCounter,
    /// Current transaction pool size.
    pub txpool_size: IntGauge,
    /// Number of connected peers.
    pub peer_count: IntGauge,
    /// Total game ticks executed.
    pub game_ticks_total: IntCounter,
    /// Block production latency in seconds.
    pub block_production_seconds: Histogram,
    /// Game tick execution latency in seconds.
    pub game_tick_seconds: Histogram,
}

impl NodeMetrics {
    /// Create and register all metrics with the given registry.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let blocks_total = IntCounter::new("velochain_blocks_total", "Total blocks produced/imported")?;
        let chain_height = IntGauge::new("velochain_chain_height", "Current chain height")?;
        let transactions_total = IntCounter::new("velochain_transactions_total", "Total transactions processed")?;
        let txpool_size = IntGauge::new("velochain_txpool_size", "Current transaction pool size")?;
        let peer_count = IntGauge::new("velochain_peer_count", "Number of connected peers")?;
        let game_ticks_total = IntCounter::new("velochain_game_ticks_total", "Total game ticks executed")?;
        let block_production_seconds = Histogram::with_opts(
            HistogramOpts::new("velochain_block_production_seconds", "Block production latency")
                .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        )?;
        let game_tick_seconds = Histogram::with_opts(
            HistogramOpts::new("velochain_game_tick_seconds", "Game tick execution latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.2]),
        )?;

        registry.register(Box::new(blocks_total.clone()))?;
        registry.register(Box::new(chain_height.clone()))?;
        registry.register(Box::new(transactions_total.clone()))?;
        registry.register(Box::new(txpool_size.clone()))?;
        registry.register(Box::new(peer_count.clone()))?;
        registry.register(Box::new(game_ticks_total.clone()))?;
        registry.register(Box::new(block_production_seconds.clone()))?;
        registry.register(Box::new(game_tick_seconds.clone()))?;

        Ok(Self {
            blocks_total,
            chain_height,
            transactions_total,
            txpool_size,
            peer_count,
            game_ticks_total,
            block_production_seconds,
            game_tick_seconds,
        })
    }

    /// Create metrics with the default global registry.
    pub fn with_default_registry() -> Result<Self, prometheus::Error> {
        Self::new(&Registry::new())
    }

    /// Encode all metrics into the Prometheus text exposition format.
    pub fn encode(&self, registry: &Registry) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        let _ = encoder.encode(&metric_families, &mut buffer);
        String::from_utf8(buffer).unwrap_or_default()
    }

    /// Record a new block.
    pub fn record_block(&self, block_number: u64, tx_count: u64) {
        self.blocks_total.inc();
        self.chain_height.set(block_number as i64);
        self.transactions_total.inc_by(tx_count);
    }

    /// Record a game tick.
    pub fn record_game_tick(&self, duration_secs: f64) {
        self.game_ticks_total.inc();
        self.game_tick_seconds.observe(duration_secs);
    }

    /// Update the transaction pool size.
    pub fn set_txpool_size(&self, size: usize) {
        self.txpool_size.set(size as i64);
    }

    /// Update the peer count.
    pub fn set_peer_count(&self, count: usize) {
        self.peer_count.set(count as i64);
    }
}
