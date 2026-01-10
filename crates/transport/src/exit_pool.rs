//! Exit node pool with health checking and load balancing
//!
//! Manages multiple exit nodes and distributes traffic.

use crate::exit_client::{ExitClient, ExitClientConfig, ExitClientError, SharedExitClient};
use apfsds_protocol::PlainPacket;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for exit pool
#[derive(Debug, Clone)]
pub struct ExitPoolConfig {
    /// List of exit node URLs
    pub exit_nodes: Vec<String>,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Per-client timeout
    pub client_timeout: Duration,

    /// Use HTTP/2
    pub http2: bool,
}

impl Default for ExitPoolConfig {
    fn default() -> Self {
        Self {
            exit_nodes: vec!["http://127.0.0.1:8081".to_string()],
            health_check_interval: Duration::from_secs(10),
            client_timeout: Duration::from_secs(10),
            http2: true,
        }
    }
}

/// Pool of exit node clients with load balancing
pub struct ExitPool {
    clients: RwLock<Vec<SharedExitClient>>,
    next_index: AtomicUsize,
    config: ExitPoolConfig,
}

impl ExitPool {
    /// Create a new exit pool
    pub fn new(config: ExitPoolConfig) -> Result<Self, ExitClientError> {
        let mut clients = Vec::new();

        for url in &config.exit_nodes {
            let client_config = ExitClientConfig {
                base_url: url.clone(),
                timeout: config.client_timeout,
                http2: config.http2,
            };

            let client = ExitClient::new(client_config)?;
            clients.push(Arc::new(client));
        }

        info!("Created exit pool with {} nodes", clients.len());

        Ok(Self {
            clients: RwLock::new(clients),
            next_index: AtomicUsize::new(0),
            config,
        })
    }

    /// Forward a packet using round-robin selection
    pub async fn forward(&self, packet: &PlainPacket) -> Result<(), ExitClientError> {
        let clients = self.clients.read().await;

        if clients.is_empty() {
            return Err(ExitClientError::ConnectionFailed(
                "No exit nodes available".to_string(),
            ));
        }

        // Round-robin with health awareness
        let start_index = self.next_index.fetch_add(1, Ordering::Relaxed) % clients.len();
        let mut attempts = 0;

        while attempts < clients.len() {
            let index = (start_index + attempts) % clients.len();
            let client = &clients[index];

            if client.is_healthy() {
                match client.forward(packet).await {
                    Ok(()) => {
                        debug!("Forwarded via exit node {}", client.base_url());
                        return Ok(());
                    }
                    Err(e) => {
                        warn!("Exit node {} failed: {}", client.base_url(), e);
                        attempts += 1;
                    }
                }
            } else {
                attempts += 1;
            }
        }

        Err(ExitClientError::ConnectionFailed(
            "All exit nodes failed".to_string(),
        ))
    }

    /// Run health check on all nodes
    pub async fn health_check_all(&self) {
        let clients = self.clients.read().await;
        let mut healthy_count = 0;

        for client in clients.iter() {
            if client.health_check().await {
                healthy_count += 1;
            } else {
                warn!("Exit node {} is unhealthy", client.base_url());
            }
        }

        debug!("{}/{} exit nodes healthy", healthy_count, clients.len());
    }

    /// Start background health checker
    pub fn start_health_checker(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;
                self.health_check_all().await;
            }
        })
    }

    /// Get count of healthy nodes
    pub async fn healthy_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.iter().filter(|c| c.is_healthy()).count()
    }

    /// Get total node count
    pub async fn total_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Add a new exit node dynamically
    pub async fn add_node(&self, url: String) -> Result<(), ExitClientError> {
        let client_config = ExitClientConfig {
            base_url: url.clone(),
            timeout: self.config.client_timeout,
            http2: self.config.http2,
        };

        let client = ExitClient::new(client_config)?;
        let mut clients = self.clients.write().await;
        clients.push(Arc::new(client));

        info!("Added exit node: {}", url);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exit_pool_creation() {
        let config = ExitPoolConfig::default();
        let pool = ExitPool::new(config).unwrap();
        assert_eq!(pool.total_count().await, 1);
    }
}
