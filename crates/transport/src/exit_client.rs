//! Exit node client for Handler → Exit communication
//! 
//! Uses HTTP/2 + rkyv serialization for high performance.

use apfsds_protocol::PlainPacket;
use reqwest::Client;
use rkyv::rancor::Error as RkyvError;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, trace};

/// Exit client errors
#[derive(Error, Debug)]
pub enum ExitClientError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Exit node unhealthy")]
    Unhealthy,
}

/// Configuration for exit client
#[derive(Debug, Clone)]
pub struct ExitClientConfig {
    /// Exit node base URL (e.g., "http://exit-1.internal:8081")
    pub base_url: String,

    /// Request timeout
    pub timeout: Duration,

    /// Enable HTTP/2
    pub http2: bool,
}

impl Default for ExitClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8081".to_string(),
            timeout: Duration::from_secs(10),
            http2: true,
        }
    }
}

/// Client for communicating with exit nodes
pub struct ExitClient {
    client: Client,
    config: ExitClientConfig,
    healthy: std::sync::atomic::AtomicBool,
}

impl ExitClient {
    /// Create a new exit client
    pub fn new(config: ExitClientConfig) -> Result<Self, ExitClientError> {
        let mut builder = Client::builder()
            .timeout(config.timeout)
            .pool_max_idle_per_host(10);

        if config.http2 {
            builder = builder.http2_prior_knowledge();
        }

        let client = builder
            .build()
            .map_err(|e| ExitClientError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            client,
            config,
            healthy: std::sync::atomic::AtomicBool::new(true),
        })
    }

    /// Forward a packet to the exit node
    pub async fn forward(&self, packet: &PlainPacket) -> Result<(), ExitClientError> {
        if !self.is_healthy() {
            return Err(ExitClientError::Unhealthy);
        }

        // Serialize with rkyv
        let bytes = rkyv::to_bytes::<RkyvError>(packet)
            .map_err(|e| ExitClientError::SerializationError(e.to_string()))?;

        let url = format!("{}/forward", self.config.base_url);
        trace!("Forwarding packet to {}", url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| {
                self.mark_unhealthy();
                ExitClientError::RequestFailed(e.to_string())
            })?;

        if !response.status().is_success() {
            error!("Exit node returned error: {}", response.status());
            return Err(ExitClientError::RequestFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        debug!("Packet forwarded successfully");
        Ok(())
    }

    /// Check health of exit node
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.healthy
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                true
            }
            _ => {
                self.healthy
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                false
            }
        }
    }

    /// Check if client is marked healthy
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Mark as unhealthy
    fn mark_unhealthy(&self) {
        self.healthy
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}

/// Shared exit client
pub type SharedExitClient = Arc<ExitClient>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_client_config_default() {
        let config = ExitClientConfig::default();
        assert!(config.http2);
        assert_eq!(config.timeout, Duration::from_secs(10));
    }
}
