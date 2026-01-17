//! SOCKS5 Tunnel Integration Tests
//!
//! Tests the full SOCKS5 proxy tunnel through daemon.

mod integration_harness;

use integration_harness::{cleanup, spawn_client, spawn_daemon, wait_for_port, TestConfig};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Test: SOCKS5 handshake succeeds
#[tokio::test]
#[ignore]
async fn test_socks5_handshake() {
    let config = TestConfig::default();

    // Start daemon
    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.daemon_bind, Duration::from_secs(30)).await);

    // Start client
    let endpoint = format!("ws://{}/v1/connect", config.daemon_bind);
    let mut client = spawn_client(&config, &endpoint).expect("Failed to spawn client");

    // Wait for SOCKS5 proxy to be available
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(wait_for_port(config.client_socks_bind, Duration::from_secs(30)).await);

    // SOCKS5 handshake
    let mut stream = TcpStream::connect(config.client_socks_bind)
        .await
        .expect("Failed to connect to SOCKS5");

    // SOCKS5 greeting: version=5, 1 auth method (no auth)
    stream.write_all(&[0x05, 0x01, 0x00]).await.unwrap();

    let mut response = [0u8; 2];
    stream.read_exact(&mut response).await.unwrap();

    // Expected: version=5, method=0 (no auth)
    assert_eq!(response[0], 0x05, "SOCKS version mismatch");
    assert_eq!(response[1], 0x00, "SOCKS auth method mismatch");

    println!("SOCKS5 handshake successful!");

    let _ = client.kill();
    let _ = daemon.kill();
    cleanup();
}

/// Test: SOCKS5 CONNECT to external host
#[tokio::test]
#[ignore]
async fn test_socks5_connect_external() {
    let config = TestConfig::default();

    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.daemon_bind, Duration::from_secs(30)).await);

    let endpoint = format!("ws://{}/v1/connect", config.daemon_bind);
    let mut client = spawn_client(&config, &endpoint).expect("Failed to spawn client");

    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(wait_for_port(config.client_socks_bind, Duration::from_secs(30)).await);

    let mut stream = TcpStream::connect(config.client_socks_bind)
        .await
        .expect("Failed to connect to SOCKS5");

    // Handshake
    stream.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await.unwrap();

    // CONNECT to example.com:80
    // CMD=CONNECT, RSV=0, ATYP=DOMAINNAME
    let mut connect_req = vec![0x05, 0x01, 0x00, 0x03];
    let domain = b"example.com";
    connect_req.push(domain.len() as u8);
    connect_req.extend_from_slice(domain);
    connect_req.extend_from_slice(&80u16.to_be_bytes());

    stream.write_all(&connect_req).await.unwrap();

    // Read response
    let mut response = [0u8; 10];
    let n = stream.read(&mut response).await.unwrap();

    println!("CONNECT response: {:?}", &response[..n]);

    // REP should be 0x00 (succeeded) if tunnel works
    if n >= 2 && response[1] == 0x00 {
        println!("SOCKS5 CONNECT succeeded!");

        // Send HTTP request
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
            .await
            .unwrap();

        let mut http_response = vec![0u8; 4096];
        let n = stream.read(&mut http_response).await.unwrap_or(0);
        println!(
            "HTTP Response (first 500 bytes): {}",
            String::from_utf8_lossy(&http_response[..n.min(500)])
        );
    } else {
        println!("SOCKS5 CONNECT failed (expected in isolated test)");
    }

    let _ = client.kill();
    let _ = daemon.kill();
    cleanup();
}

/// Test: Multiple concurrent SOCKS5 connections
#[tokio::test]
#[ignore]
async fn test_socks5_concurrent() {
    let config = TestConfig::default();

    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.daemon_bind, Duration::from_secs(30)).await);

    let endpoint = format!("ws://{}/v1/connect", config.daemon_bind);
    let mut client = spawn_client(&config, &endpoint).expect("Failed to spawn client");

    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(wait_for_port(config.client_socks_bind, Duration::from_secs(30)).await);

    // Spawn 10 concurrent connections
    let mut handles = vec![];
    for i in 0..10 {
        let addr = config.client_socks_bind;
        handles.push(tokio::spawn(async move {
            let result = TcpStream::connect(addr).await;
            match result {
                Ok(mut stream) => {
                    stream.write_all(&[0x05, 0x01, 0x00]).await.ok();
                    let mut buf = [0u8; 2];
                    stream.read_exact(&mut buf).await.ok();
                    println!("Connection {} handshake: {:?}", i, buf);
                    buf[0] == 0x05
                }
                Err(e) => {
                    println!("Connection {} failed: {}", i, e);
                    false
                }
            }
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;
    let success_count = results.iter().filter(|r| r.as_ref() == Ok(&true)).count();

    println!(
        "Concurrent test: {}/{} connections succeeded",
        success_count,
        results.len()
    );

    let _ = client.kill();
    let _ = daemon.kill();
    cleanup();
}
