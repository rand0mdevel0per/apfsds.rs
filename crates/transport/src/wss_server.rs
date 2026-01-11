//! WebSocket server for handling client connections

use std::net::SocketAddr;
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Error, Debug)]
pub enum WssServerError {
    #[error("Bind failed: {0}")]
    BindFailed(String),

    #[error("Accept failed: {0}")]
    AcceptFailed(String),

    #[error("Upgrade failed: {0}")]
    UpgradeFailed(String),
}

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct WssServerConfig {
    /// Bind address
    pub bind: SocketAddr,

    /// Maximum connections
    pub max_connections: usize,

    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for WssServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:25347".parse().unwrap(),
            max_connections: 10000,
            timeout_secs: 300,
        }
    }
}

/// WebSocket server (placeholder - full implementation in daemon)
pub struct WssServer {
    listener: TcpListener,
    config: WssServerConfig,
}

impl WssServer {
    /// Create a new WebSocket server
    pub async fn bind(config: WssServerConfig) -> Result<Self, WssServerError> {
        let listener = TcpListener::bind(config.bind)
            .await
            .map_err(|e| WssServerError::BindFailed(e.to_string()))?;

        info!("WebSocket server listening on {}", config.bind);

        Ok(Self { listener, config })
    }

    /// Get the bound address
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept next connection (raw TCP - upgrade happens in handler)
    pub async fn accept(&self) -> Result<(tokio::net::TcpStream, SocketAddr), WssServerError> {
        self.listener
            .accept()
            .await
            .map_err(|e| WssServerError::AcceptFailed(e.to_string()))
    }
}
