//! Enhanced logging and tracing configuration.
//!
//! Provides structured logging with per-module log level control
//! and tracing span support for key node operations.

use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Logging configuration options.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Base log level filter string (e.g. "info", "debug,velochain_rpc=trace").
    pub filter: String,
    /// Whether to include timestamps in log output.
    pub timestamps: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            filter: "info".to_string(),
            timestamps: true,
        }
    }
}

/// Initialize the global tracing subscriber with the given configuration.
///
/// This should be called once at node startup. The filter string supports
/// per-module overrides, e.g.:
///
/// ```text
/// info,velochain_rpc=debug,velochain_consensus=trace
/// ```
///
/// If the `RUST_LOG` environment variable is set, it takes precedence
/// over the configured filter.
pub fn init_logging(config: &LogConfig) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.filter));

    let layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false);

    if config.timestamps {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(layer.without_time())
            .init();
    }
}
