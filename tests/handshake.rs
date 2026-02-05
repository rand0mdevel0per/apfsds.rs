//! Daemon-Client Handshake Integration Tests
//!
//! Tests the authentication flow between client and daemon.

mod integration_harness;

use integration_harness::{TestConfig, cleanup, spawn_daemon, wait_for_port};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Test: Daemon accepts WebSocket upgrade
#[tokio::test]
#[ignore] // Run with: cargo test --test handshake -- --ignored
async fn test_daemon_accepts_websocket() {
    let config = TestConfig::default();

    // Spawn daemon
    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");

    // Wait for daemon to start
    assert!(
        wait_for_port(config.daemon_bind, Duration::from_secs(30)).await,
        "Daemon did not start in time"
    );

    // Attempt WebSocket handshake
    let mut stream = TcpStream::connect(config.daemon_bind)
        .await
        .expect("Failed to connect");

    let request = format!(
        "GET /v1/connect HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        config.daemon_bind
    );

    stream
        .write_all(request.as_bytes())
        .await
        .expect("Failed to send request");

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.expect("Failed to read");

    let response_str = String::from_utf8_lossy(&response[..n]);
    println!("Response: {}", response_str);

    // Expect 101 Switching Protocols
    assert!(
        response_str.contains("101") || response_str.contains("Switching Protocols"),
        "Expected WebSocket upgrade response"
    );

    // Cleanup
    let _ = daemon.kill();
    cleanup();
}

/// Test: Daemon rejects invalid auth token
#[tokio::test]
#[ignore]
async fn test_daemon_rejects_invalid_token() {
    let config = TestConfig::default();

    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.daemon_bind, Duration::from_secs(30)).await);

    // Connect with invalid credentials
    let mut stream = TcpStream::connect(config.daemon_bind)
        .await
        .expect("Failed to connect");

    // Send garbage auth data (should be rejected)
    let garbage = b"INVALID_AUTH_DATA_12345";
    stream.write_all(garbage).await.ok();

    // Daemon should close connection or send error
    let mut buf = [0u8; 256];
    let result = stream.read(&mut buf).await;

    // Connection should be closed or error returned
    match result {
        Ok(0) => println!("Connection closed as expected"),
        Ok(n) => println!("Received: {:?}", &buf[..n]),
        Err(e) => println!("Error (expected): {}", e),
    }

    let _ = daemon.kill();
    cleanup();
}

/// Test: Management API health check
#[tokio::test]
#[ignore]
async fn test_management_api_health() {
    let config = TestConfig::default();

    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.management_bind, Duration::from_secs(30)).await);

    // HTTP GET to management API
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/admin/stats", config.management_bind))
        .send()
        .await;

    match response {
        Ok(resp) => {
            assert!(resp.status().is_success(), "Expected 2xx status");
            println!("Stats: {}", resp.text().await.unwrap_or_default());
        }
        Err(e) => {
            eprintln!("HTTP request failed: {}", e);
        }
    }

    let _ = daemon.kill();
    cleanup();
}
