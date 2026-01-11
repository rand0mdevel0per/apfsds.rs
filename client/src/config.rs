//! Client configuration

use anyhow::Result;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::Path;

/// Client configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
    /// SOCKS5 configuration
    #[serde(default)]
    pub socks5: Socks5Config,

    /// TUN configuration
    #[serde(default)]
    pub tun: TunConfig,

    /// Connection configuration
    #[serde(default)]
    pub connection: ConnectionConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// Emergency mode configuration
    #[serde(default)]
    pub emergency: EmergencyConfig,

    /// Obfuscation configuration
    #[serde(default)]
    pub obfuscation: ObfuscationConfig,

    /// DNS configuration (Local DNS)
    #[serde(default)]
    pub dns: DnsConfig,
}

impl ClientConfig {
    /// Load configuration from file
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: ClientConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            socks5: Socks5Config::default(),
            tun: TunConfig::default(),
            connection: ConnectionConfig::default(),
            security: SecurityConfig::default(),
            emergency: EmergencyConfig::default(),
            obfuscation: ObfuscationConfig::default(),
            dns: DnsConfig::default(),
        }
    }
}

/// SOCKS5 server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Socks5Config {
    /// Bind address
    #[serde(default = "default_socks5_bind")]
    pub bind: SocketAddr,

    /// Enable authentication
    #[serde(default)]
    pub auth: bool,
}

fn default_socks5_bind() -> SocketAddr {
    "127.0.0.1:1080".parse().unwrap()
}

impl Default for Socks5Config {
    fn default() -> Self {
        Self {
            bind: default_socks5_bind(),
            auth: false,
        }
    }
}

/// TUN device configuration
#[derive(Debug, Clone, Deserialize)]
pub struct TunConfig {
    /// TUN device name
    #[serde(default = "default_tun_device")]
    pub device: String,

    /// TUN address
    #[serde(default = "default_tun_address")]
    pub address: String,

    /// MTU
    #[serde(default = "default_mtu")]
    pub mtu: u16,
}

fn default_tun_device() -> String {
    "tun-apfsds".to_string()
}

fn default_tun_address() -> String {
    "10.0.0.2/24".to_string()
}

fn default_mtu() -> u16 {
    1500
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            device: default_tun_device(),
            address: default_tun_address(),
            mtu: default_mtu(),
        }
    }
}

/// Connection pool configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    /// Number of connections to maintain
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,

    /// Server endpoints
    #[serde(default)]
    pub endpoints: Vec<String>,

    /// Token endpoint
    #[serde(default)]
    pub token_endpoint: Option<String>,

    /// Reconnect interval range (seconds)
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: (u64, u64),

    /// Connection timeout (seconds)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_pool_size() -> usize {
    6
}

fn default_reconnect_interval() -> (u64, u64) {
    (60, 180)
}

fn default_timeout() -> u64 {
    30
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            pool_size: default_pool_size(),
            endpoints: Vec::new(),
            token_endpoint: None,
            reconnect_interval: default_reconnect_interval(),
            timeout: default_timeout(),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    /// Path to credentials file
    #[serde(default)]
    pub credentials_path: Option<String>,

    /// Client secret key (hex)
    #[serde(default)]
    pub client_sk: Option<String>,

    /// Server public key (hex)
    #[serde(default)]
    pub server_pk: Option<String>,

    /// HMAC secret (hex)
    #[serde(default)]
    pub hmac_secret: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            credentials_path: None,
            client_sk: None,
            server_pk: None,
            hmac_secret: None,
        }
    }
}

/// Emergency mode configuration
#[derive(Debug, Clone, Deserialize)]
pub struct EmergencyConfig {
    /// Enable emergency mode checks
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Crate name to check on crates.io
    #[serde(default = "default_crate_name")]
    pub crate_name: String,

    /// Check interval in seconds
    #[serde(default = "default_check_interval")]
    pub check_interval: u64,
}

fn default_true() -> bool {
    true
}

fn default_crate_name() -> String {
    "apfsds".to_string()
}

fn default_check_interval() -> u64 {
    300 // 5 minutes
}

impl Default for EmergencyConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            crate_name: default_crate_name(),
            check_interval: default_check_interval(),
        }
    }
}

/// Obfuscation configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ObfuscationConfig {
    /// Noise ratio (0.0 - 1.0)
    #[serde(default = "default_noise_ratio")]
    pub noise_ratio: f32,

    /// Enable fake JSON responses
    #[serde(default = "default_true")]
    pub fake_json_enabled: bool,

    /// Enable SSE keepalive
    #[serde(default = "default_true")]
    pub sse_keepalive: bool,
}

fn default_noise_ratio() -> f32 {
    0.15
}

impl Default for ObfuscationConfig {
    fn default() -> Self {
        Self {
            noise_ratio: default_noise_ratio(),
            fake_json_enabled: default_true(),
            sse_keepalive: default_true(),
        }
    }
}

/// Local DNS configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DnsConfig {
    /// Enable local DNS server
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bind address (udp)
    #[serde(default = "default_dns_bind")]
    pub bind: SocketAddr,
}

fn default_dns_bind() -> SocketAddr {
    "127.0.0.1:53".parse().unwrap() // Default standard DNS port
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            bind: default_dns_bind(),
        }
    }
}
