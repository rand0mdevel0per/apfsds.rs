//! Daemon configuration

use anyhow::Result;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::Path;

/// Daemon configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Exit nodes configuration
    #[serde(default)]
    pub exit_nodes: Vec<ExitNodeConfig>,

    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// Monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
}

impl DaemonConfig {
    /// Load configuration from file
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: DaemonConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            exit_nodes: Vec::new(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Server mode
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Bind address
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Location name
    #[serde(default)]
    pub location: Option<String>,

    /// Maximum connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

fn default_mode() -> String {
    "handler".to_string()
}

fn default_bind() -> SocketAddr {
    "0.0.0.0:25347".parse().unwrap()
}

fn default_max_connections() -> usize {
    10000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            bind: default_bind(),
            location: None,
            max_connections: default_max_connections(),
        }
    }
}

/// Exit node configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ExitNodeConfig {
    /// Node name
    pub name: String,

    /// Endpoint address
    pub endpoint: String,

    /// Weight for load balancing
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Location description
    #[serde(default)]
    pub location: Option<String>,
}

fn default_weight() -> f64 {
    1.0
}

/// Storage configuration
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Path to tmpfs
    #[serde(default = "default_tmpfs_path")]
    pub tmpfs_path: String,

    /// tmpfs size limit
    #[serde(default = "default_tmpfs_size")]
    pub tmpfs_size: usize,

    /// Path to disk storage
    #[serde(default = "default_disk_path")]
    pub disk_path: String,

    /// Segment size limit
    #[serde(default = "default_segment_size")]
    pub segment_size_limit: usize,

    /// Compaction threshold
    #[serde(default = "default_compaction_threshold")]
    pub compaction_threshold: usize,
}

fn default_tmpfs_path() -> String {
    "/dev/shm/apfsds".to_string()
}

fn default_tmpfs_size() -> usize {
    512 * 1024 * 1024 // 512MB
}

fn default_disk_path() -> String {
    "/var/lib/apfsds".to_string()
}

fn default_segment_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_compaction_threshold() -> usize {
    10
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            tmpfs_path: default_tmpfs_path(),
            tmpfs_size: default_tmpfs_size(),
            disk_path: default_disk_path(),
            segment_size_limit: default_segment_size(),
            compaction_threshold: default_compaction_threshold(),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    /// Server secret key (hex)
    #[serde(default)]
    pub server_sk: Option<String>,

    /// HMAC secret (hex)
    #[serde(default)]
    pub hmac_secret: Option<String>,

    /// Token TTL in seconds
    #[serde(default = "default_token_ttl")]
    pub token_ttl: u64,

    /// Key rotation interval in seconds
    #[serde(default = "default_rotation_interval")]
    pub key_rotation_interval: u64,

    /// Grace period for key rotation
    #[serde(default = "default_grace_period")]
    pub grace_period: u64,
}

fn default_token_ttl() -> u64 {
    60 // 60 seconds
}

fn default_rotation_interval() -> u64 {
    604800 // 7 days
}

fn default_grace_period() -> u64 {
    600 // 10 minutes
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            server_sk: None,
            hmac_secret: None,
            token_ttl: default_token_ttl(),
            key_rotation_interval: default_rotation_interval(),
            grace_period: default_grace_period(),
        }
    }
}

/// Monitoring configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    /// Prometheus metrics bind address
    #[serde(default = "default_prometheus_bind")]
    pub prometheus_bind: SocketAddr,

    /// Enable Prometheus
    #[serde(default = "default_true")]
    pub prometheus_enabled: bool,
}

fn default_prometheus_bind() -> SocketAddr {
    "0.0.0.0:9090".parse().unwrap()
}

fn default_true() -> bool {
    true
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            prometheus_bind: default_prometheus_bind(),
            prometheus_enabled: default_true(),
        }
    }
}
