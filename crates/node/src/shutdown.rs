//! Graceful shutdown coordination for VeloChain node.
//!
//! Provides a shutdown signal that can be shared across subsystems,
//! ensuring all components save state and stop cleanly.

use std::sync::Arc;
use tokio::sync::{broadcast, Notify};
use tracing::info;

/// Coordinates graceful shutdown across all node subsystems.
#[derive(Clone)]
pub struct ShutdownController {
    /// Notify all waiters that shutdown has been requested.
    notify: Arc<Notify>,
    /// Broadcast sender for shutdown signal.
    tx: broadcast::Sender<()>,
    /// Whether shutdown has been initiated.
    initiated: Arc<std::sync::atomic::AtomicBool>,
}

impl ShutdownController {
    /// Create a new shutdown controller.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self {
            notify: Arc::new(Notify::new()),
            tx,
            initiated: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Trigger shutdown. All waiters will be notified.
    pub fn shutdown(&self) {
        if !self
            .initiated
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            info!("Shutdown signal sent to all subsystems");
            self.notify.notify_waiters();
            let _ = self.tx.send(());
        }
    }

    /// Check if shutdown has been initiated.
    pub fn is_shutdown(&self) -> bool {
        self.initiated.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Wait for the shutdown signal. Returns immediately if already shut down.
    pub async fn wait_for_shutdown(&self) {
        if self.is_shutdown() {
            return;
        }
        self.notify.notified().await;
    }

    /// Get a broadcast receiver for the shutdown signal.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    /// Install OS signal handlers (SIGINT, SIGTERM) that trigger shutdown.
    pub fn install_signal_handlers(&self) {
        let controller = self.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");
                let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
                tokio::select! {
                    _ = sigint.recv() => {
                        info!("Received SIGINT, initiating graceful shutdown...");
                    }
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, initiating graceful shutdown...");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c().await.expect("Ctrl+C handler");
                info!("Received Ctrl+C, initiating graceful shutdown...");
            }
            controller.shutdown();
        });
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}
