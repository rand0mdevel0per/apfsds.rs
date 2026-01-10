//! Connection pool for WebSocket connections

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::{WssClient, WssClientConfig, WssClientError};

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("Pool exhausted")]
    PoolExhausted,

    #[error("Connection failed: {0}")]
    ConnectionFailed(#[from] WssClientError),

    #[error("Pool is closed")]
    PoolClosed,
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Number of connections to maintain
    pub pool_size: usize,

    /// Server endpoints (will round-robin)
    pub endpoints: Vec<String>,

    /// Authorization token
    pub token: Option<String>,

    /// Reconnect on failure
    pub auto_reconnect: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 6,
            endpoints: Vec::new(),
            token: None,
            auto_reconnect: true,
        }
    }
}

/// A managed connection in the pool
pub struct PooledConnection {
    client: WssClient,
    endpoint: String,
    id: usize,
}

impl PooledConnection {
    pub fn client(&self) -> &WssClient {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut WssClient {
        &mut self.client
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

/// Connection pool for WebSocket connections
pub struct ConnectionPool {
    config: ConnectionPoolConfig,
    connections: Vec<RwLock<Option<WssClient>>>,
    robin_counter: AtomicUsize,
    closed: AtomicBool,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let mut connections = Vec::with_capacity(config.pool_size);
        for _ in 0..config.pool_size {
            connections.push(RwLock::new(None));
        }

        Self {
            config,
            connections,
            robin_counter: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
        }
    }

    /// Initialize all connections
    pub async fn connect_all(&self) -> Result<(), PoolError> {
        if self.config.endpoints.is_empty() {
            return Err(PoolError::ConnectionFailed(WssClientError::InvalidUrl(
                "No endpoints configured".to_string(),
            )));
        }

        for i in 0..self.config.pool_size {
            let endpoint = &self.config.endpoints[i % self.config.endpoints.len()];
            self.connect_slot(i, endpoint).await?;
        }

        info!(
            "Connection pool initialized with {} connections",
            self.config.pool_size
        );

        Ok(())
    }

    /// Connect a specific slot
    async fn connect_slot(&self, slot: usize, endpoint: &str) -> Result<(), PoolError> {
        let config = WssClientConfig {
            url: endpoint.to_string(),
            token: self.config.token.clone(),
            ..Default::default()
        };

        let mut client = WssClient::connect(config).await?;

        // Send initial frames
        client.send_initial_frames().await?;

        let mut guard = self.connections[slot].write();
        *guard = Some(client);

        debug!("Connected slot {} to {}", slot, endpoint);

        Ok(())
    }

    /// Get the next connection (round-robin)
    pub fn get_slot(&self) -> usize {
        let slot = self.robin_counter.fetch_add(1, Ordering::Relaxed) % self.config.pool_size;
        slot
    }

    /// Execute an operation on a connection
    pub async fn with_connection<F, T>(&self, f: F) -> Result<T, PoolError>
    where
        F: FnOnce(&mut WssClient) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, WssClientError>> + Send + '_>>,
    {
        if self.closed.load(Ordering::Relaxed) {
            return Err(PoolError::PoolClosed);
        }

        let slot = self.get_slot();
        let mut guard = self.connections[slot].write();

        match guard.as_mut() {
            Some(client) => {
                let result = f(client).await;
                match result {
                    Ok(v) => Ok(v),
                    Err(e) => {
                        warn!("Connection error on slot {}: {}", slot, e);
                        // Mark for reconnection
                        *guard = None;
                        Err(PoolError::ConnectionFailed(e))
                    }
                }
            }
            None => Err(PoolError::PoolExhausted),
        }
    }

    /// Close all connections
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);

        for i in 0..self.connections.len() {
            let mut guard = self.connections[i].write();
            if let Some(mut client) = guard.take() {
                let _ = client.close().await;
            }
        }

        info!("Connection pool closed");
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let mut active = 0;
        for conn in &self.connections {
            if conn.read().is_some() {
                active += 1;
            }
        }

        PoolStats {
            pool_size: self.config.pool_size,
            active_connections: active,
            total_requests: self.robin_counter.load(Ordering::Relaxed),
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub pool_size: usize,
    pub active_connections: usize,
    pub total_requests: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config() {
        let config = ConnectionPoolConfig::default();
        assert_eq!(config.pool_size, 6);
        assert!(config.auto_reconnect);
    }

    #[test]
    fn test_round_robin() {
        let config = ConnectionPoolConfig {
            pool_size: 4,
            endpoints: vec!["ws://test".to_string()],
            ..Default::default()
        };

        let pool = ConnectionPool::new(config);

        assert_eq!(pool.get_slot(), 0);
        assert_eq!(pool.get_slot(), 1);
        assert_eq!(pool.get_slot(), 2);
        assert_eq!(pool.get_slot(), 3);
        assert_eq!(pool.get_slot(), 0); // Wraps around
    }
}
