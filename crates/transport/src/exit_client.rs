//! Exit node client for Handler → Exit communication
//!
//! Uses HTTP/2 + rkyv serialization for high performance.

use crate::SharedPacketDispatcher;
use apfsds_protocol::PlainPacket;
use bytes::{Buf, Bytes, BytesMut};
use futures::StreamExt;
use reqwest::Client;
use rkyv::rancor::Error as RkyvError;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

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

    /// Subscribe to return traffic stream
    pub fn subscribe(self: Arc<Self>, handler_id: u64, dispatcher: SharedPacketDispatcher) {
        tokio::spawn(async move {
            let url = format!("{}/stream?handler_id={}", self.config.base_url, handler_id);
            let mut backoff = Duration::from_secs(1);

            loop {
                info!("Connecting to exit node stream at {}", url);
                match self.client.get(&url).send().await {
                    Ok(mut resp) => {
                        if !resp.status().is_success() {
                            warn!("Stream failed HTTP {}", resp.status());
                            tokio::time::sleep(backoff).await;
                            continue;
                        }

                        self.healthy
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        backoff = Duration::from_secs(1);

                        // let mut stream = resp.bytes_stream();
                        let mut buffer = BytesMut::new();

                        loop {
                            match resp.chunk().await {
                                Ok(Some(chunk)) => {
                                    buffer.extend_from_slice(&chunk);

                                    // Process frames (Length + Payload)
                                    loop {
                                        if buffer.len() < 4 {
                                            break;
                                        }

                                        let mut len_bytes = [0u8; 4];
                                        len_bytes.copy_from_slice(&buffer[..4]);
                                        let len = u32::from_le_bytes(len_bytes) as usize;

                                        if buffer.len() < 4 + len {
                                            break; // Wait for more data
                                        }

                                        // Consume header
                                        buffer.advance(4);
                                        // Extract payload
                                        let payload = buffer.split_to(len);

                                        // Deserialize PlainPacket
                                        match rkyv::from_bytes::<PlainPacket, rkyv::rancor::Error>(
                                            &payload,
                                        ) {
                                            Ok(packet) => {
                                                dispatcher.dispatch(packet).await;
                                            }
                                            Err(e) => {
                                                error!("Stream deserialization error: {}", e);
                                            }
                                        }
                                    }
                                }
                                Ok(None) => {
                                    break; // EOF
                                }
                                Err(e) => {
                                    error!("Stream read error: {}", e);
                                    break;
                                }
                            }
                        }
                        warn!("Stream disconnected");
                    }
                    Err(e) => {
                        error!("Failed to connect stream: {}", e);
                        self.mark_unhealthy();
                    }
                }

                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
            }
        });
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
