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

    /// Raft configuration
    #[serde(default)]
    pub raft: RaftConfig,

    /// Exit nodes configuration
    #[serde(default)]
    pub exit_nodes: Vec<ExitNodeConfig>,

    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,

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

    /// Load and merge configuration from file (incremental update)
    /// 
    /// Only non-default values from the new config will overwrite existing values.
    /// Lists (like exit_nodes) will be merged by name/endpoint.
    pub async fn load_merge(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await?;
        let other: DaemonConfig = toml::from_str(&content)?;
        self.merge(other);
        Ok(())
    }

    /// Merge another config into this one (incremental)
    /// 
    /// Rules:
    /// - Scalar values: overwrite if the new value differs from default
    /// - Option values: overwrite if Some
    /// - Vec values: merge by key (name/endpoint)
    pub fn merge(&mut self, other: DaemonConfig) {
        // Server config
        if other.server.mode != default_mode() {
            self.server.mode = other.server.mode;
        }
        if other.server.bind != default_bind() {
            self.server.bind = other.server.bind;
        }
        if other.server.location.is_some() {
            self.server.location = other.server.location;
        }
        if other.server.max_connections != default_max_connections() {
            self.server.max_connections = other.server.max_connections;
        }

        // Raft config
        if other.raft.node_id != 1 {
            self.raft.node_id = other.raft.node_id;
        }
        if !other.raft.peers.is_empty() {
            // Merge peers by value (simple strings)
            for peer in other.raft.peers {
                if !self.raft.peers.contains(&peer) {
                    self.raft.peers.push(peer);
                }
            }
        }

        // Exit nodes: merge by name
        for node in other.exit_nodes {
            if let Some(existing) = self.exit_nodes.iter_mut().find(|n| n.name == node.name) {
                // Update existing node
                existing.endpoint = node.endpoint;
                existing.weight = node.weight;
                existing.group_id = node.group_id;
            } else {
                // Add new node
                self.exit_nodes.push(node);
            }
        }

        // Security config - only if provided
        if other.security.server_sk.is_some() {
            self.security.server_sk = other.security.server_sk;
        }
        if other.security.hmac_secret.is_some() {
            self.security.hmac_secret = other.security.hmac_secret;
        }
        if other.security.token_ttl != default_token_ttl() {
            self.security.token_ttl = other.security.token_ttl;
        }
        if other.security.key_rotation_interval != default_rotation_interval() {
            self.security.key_rotation_interval = other.security.key_rotation_interval;
        }

        // Database config
        if other.database.url != default_postgres_url() {
            self.database.url = other.database.url;
        }

        // Monitoring
        if other.monitoring.prometheus_bind != default_prometheus_bind() {
            self.monitoring.prometheus_bind = other.monitoring.prometheus_bind;
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            raft: RaftConfig::default(),
            exit_nodes: Vec::new(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            database: DatabaseConfig::default(),
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

/// Raft configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RaftConfig {
    /// Node ID
    #[serde(default = "default_node_id")]
    pub node_id: u64,

    /// Peer addresses (node_id -> address)
    #[serde(default)]
    pub peers: Vec<String>,

    /// Election timeout (min, max) in ms
    #[serde(default = "default_election_timeout")]
    pub election_timeout: (u64, u64),

    /// Heartbeat interval in ms
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
}

fn default_node_id() -> u64 {
    1
}

fn default_election_timeout() -> (u64, u64) {
    (150, 300)
}

fn default_heartbeat_interval() -> u64 {
    50
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: default_node_id(),
            peers: Vec::new(),
            election_timeout: default_election_timeout(),
            heartbeat_interval: default_heartbeat_interval(),
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

    /// Group ID for routing (default: 0)
    #[serde(default)]
    pub group_id: i32,
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

    /// ClickHouse backup configuration (for Phase 2)
    #[serde(default)]
    pub clickhouse: ClickHouseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClickHouseConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_clickhouse_url")]
    pub url: String,

    #[serde(default = "default_clickhouse_db")]
    pub database: String,

    #[serde(default = "default_clickhouse_table")]
    pub table: String,

    #[serde(default)]
    pub username: Option<String>,

    #[serde(default)]
    pub password: Option<String>,
}

fn default_clickhouse_url() -> String {
    "http://localhost:8123".to_string()
}

fn default_clickhouse_db() -> String {
    "apfsds".to_string()
}

fn default_clickhouse_table() -> String {
    "connections".to_string()
}

impl Default for ClickHouseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: default_clickhouse_url(),
            database: default_clickhouse_db(),
            table: default_clickhouse_table(),
            username: None,
            password: None,
        }
    }
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
            clickhouse: ClickHouseConfig::default(),
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

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    #[serde(default = "default_postgres_url")]
    pub url: String,
}

fn default_postgres_url() -> String {
    "postgres://postgres:postgres@localhost:5432/apfsds".to_string()
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_postgres_url(),
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
