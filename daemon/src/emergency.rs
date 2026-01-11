//! Emergency mode monitoring
//!
//! Monitors crates.io for emergency shutdown signal.

use crates_io_api::AsyncClient;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// Emergency mode configuration
#[derive(Debug, Clone)]
pub struct EmergencyConfig {
    /// Crate name to monitor (default: "apfsds")
    pub crate_name: String,
    /// Version prefix that triggers emergency (e.g., "0.0.0-EMERGENCY")
    pub trigger_version: String,
    /// Check interval (default: 5 minutes)
    pub check_interval: Duration,
    /// Trigger delay range in seconds (random delay before shutdown)
    pub trigger_delay_range: (u64, u64),
}

impl Default for EmergencyConfig {
    fn default() -> Self {
        Self {
            crate_name: "apfsds".to_string(),
            trigger_version: "0.0.0-EMERGENCY".to_string(),
            check_interval: Duration::from_secs(300),
            trigger_delay_range: (0, 3600),
        }
    }
}

/// Emergency monitor
pub struct EmergencyMonitor {
    config: EmergencyConfig,
    triggered: AtomicBool,
    trigger_at: AtomicU64,
    shutdown_tx: broadcast::Sender<()>,
}

impl EmergencyMonitor {
    /// Create a new emergency monitor
    pub fn new(config: EmergencyConfig) -> (Arc<Self>, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let monitor = Arc::new(Self {
            config,
            triggered: AtomicBool::new(false),
            trigger_at: AtomicU64::new(0),
            shutdown_tx,
        });
        (monitor, shutdown_rx)
    }

    /// Check if emergency mode is triggered
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::Relaxed)
    }

    /// Get trigger timestamp (0 if not triggered)
    pub fn trigger_at(&self) -> u64 {
        self.trigger_at.load(Ordering::Relaxed)
    }

    /// Manually trigger emergency mode
    pub fn trigger(&self, delay_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.trigger_at.store(now + delay_secs, Ordering::Relaxed);
        self.triggered.store(true, Ordering::Relaxed);

        warn!(
            "Emergency mode triggered! Shutdown in {} seconds",
            delay_secs
        );
    }

    /// Start the monitoring loop
    pub async fn start(self: Arc<Self>) {
        info!(
            "Emergency monitor started, checking {} every {:?}",
            self.config.crate_name, self.config.check_interval
        );

        let client = AsyncClient::new(
            "apfsds-emergency-monitor (contact@example.com)",
            Duration::from_secs(10),
        );

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to create crates.io client: {}", e);
                return;
            }
        };

        loop {
            tokio::time::sleep(self.config.check_interval).await;

            // Check if already triggered
            if self.is_triggered() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now >= self.trigger_at() {
                    info!("Emergency shutdown executing");
                    let _ = self.shutdown_tx.send(());
                    break;
                }
                continue;
            }

            // Query crates.io
            match client.get_crate(&self.config.crate_name).await {
                Ok(krate) => {
                    // Check if any version matches trigger pattern
                    for version in &krate.versions {
                        if version.num.starts_with(&self.config.trigger_version) {
                            warn!(
                                "Emergency version detected: {} = {}",
                                self.config.crate_name, version.num
                            );

                            // Random delay before shutdown
                            let (min, max) = self.config.trigger_delay_range;
                            let delay = fastrand::u64(min..=max);

                            self.trigger(delay);
                            break;
                        }
                    }
                }
                Err(e) => {
                    // Log but don't fail - network issues shouldn't break the service
                    warn!("Failed to check crates.io: {}", e);
                }
            }
        }
    }

    /// Subscribe to shutdown signal
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manual_trigger() {
        let config = EmergencyConfig::default();
        let (monitor, _rx) = EmergencyMonitor::new(config);

        assert!(!monitor.is_triggered());

        monitor.trigger(60);

        assert!(monitor.is_triggered());
        assert!(monitor.trigger_at() > 0);
    }
}
