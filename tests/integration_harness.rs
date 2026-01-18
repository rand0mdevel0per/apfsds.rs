//! Integration Test Harness
#![allow(dead_code)]
//!
//! Provides shared utilities for spawning daemon/client processes
//! and managing test fixtures.

use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Test configuration
#[allow(dead_code)]
pub struct TestConfig {
    pub daemon_bind: SocketAddr,
    pub management_bind: SocketAddr,
    pub client_socks_bind: SocketAddr,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            daemon_bind: "127.0.0.1:35347".parse().unwrap(),
            management_bind: "127.0.0.1:35348".parse().unwrap(),
            client_socks_bind: "127.0.0.1:31080".parse().unwrap(),
        }
    }
}

/// Spawn a daemon process for testing
pub fn spawn_daemon(config: &TestConfig, node_id: u64) -> std::io::Result<Child> {
    let config_content = format!(
        r#"
[server]
bind = "{}"
mode = "handler"

[raft]
node_id = {}

[monitoring]
prometheus_bind = "127.0.0.1:0"
"#,
        config.daemon_bind, node_id
    );

    // Write temp config
    let config_path = format!("tests/daemon_{}.toml", node_id);
    std::fs::write(&config_path, config_content)?;

    Command::new("cargo")
        .args(["run", "-p", "apfsds-daemon", "--", "--config", &config_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Spawn a client process for testing
pub fn spawn_client(config: &TestConfig, endpoint: &str) -> std::io::Result<Child> {
    let config_content = format!(
        r#"
[client]
mode = "socks5"

[client.socks5]
bind = "{}"

[connection]
endpoints = ["{}"]
"#,
        config.client_socks_bind, endpoint
    );

    let config_path = "tests/client_test.toml";
    std::fs::write(config_path, config_content)?;

    Command::new("cargo")
        .args(["run", "-p", "apfsds-client", "--", "--config", config_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Wait for a TCP port to become available
pub async fn wait_for_port(addr: SocketAddr, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

/// Cleanup test artifacts
pub fn cleanup() {
    let _ = std::fs::remove_file("tests/daemon_1.toml");
    let _ = std::fs::remove_file("tests/daemon_2.toml");
    let _ = std::fs::remove_file("tests/daemon_3.toml");
    let _ = std::fs::remove_file("tests/client_test.toml");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = TestConfig::default();
        assert_eq!(config.daemon_bind.port(), 35347);
    }
}
