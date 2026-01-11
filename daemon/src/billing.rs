use apfsds_storage::postgres::PgClient;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Aggregates user usage and flushes to database periodically
pub struct BillingAggregator {
    pg_client: PgClient,
    usage: Arc<Mutex<HashMap<i64, u64>>>,
    flush_interval: Duration,
}

impl BillingAggregator {
    pub fn new(pg_client: PgClient) -> Self {
        Self {
            pg_client,
            usage: Arc::new(Mutex::new(HashMap::new())),
            flush_interval: Duration::from_secs(60),
        }
    }

    /// Record usage for a user
    pub async fn record_usage(&self, user_id: i64, bytes: u64) {
        let mut usage = self.usage.lock().await;
        *usage.entry(user_id).or_default() += bytes;
    }

    /// Start the flush loop
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.flush_interval);
            loop {
                interval.tick().await;
                self.flush().await;
            }
        })
    }

    /// Flush aggregated usage to database
    async fn flush(&self) {
        let mut usage_map = {
            let mut usage = self.usage.lock().await;
            if usage.is_empty() {
                return;
            }
            // Swap with empty map
            std::mem::take(&mut *usage)
        };

        info!("Flushing billing for {} users", usage_map.len());

        for (user_id, bytes) in usage_map.drain() {
            // Update balance and log usage
            // We do this individually for now. In high load, use batch update.
            if let Err(e) = self.pg_client.record_usage(user_id, bytes).await {
                error!("Failed to record usage for user {}: {}", user_id, e);
                // Re-queue? simpler to just log error for Phase 3.
                // In prod, we should re-queue or have WAL.
            }
        }
    }
}
