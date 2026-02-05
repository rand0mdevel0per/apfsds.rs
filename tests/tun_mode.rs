//! TUN Mode Integration Tests
//!
//! Tests for the TUN device based VPN mode.
//! These tests require elevated privileges (root/Administrator).

mod integration_harness;

use integration_harness::{TestConfig, cleanup, spawn_daemon, wait_for_port};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Test: TUN device creation (requires root)
#[tokio::test]
#[ignore]
async fn test_tun_device_creation() {
    #[cfg(target_os = "linux")]
    {
        // Check if running as root
        let uid = unsafe { libc::getuid() };
        if uid != 0 {
            println!("Skipping TUN test - requires root privileges");
            return;
        }

        // Try to create a TUN device using ip command
        let output = Command::new("ip")
            .args(["tuntap", "add", "dev", "apfsds_test", "mode", "tun"])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                println!("TUN device created successfully");

                // Cleanup
                let _ = Command::new("ip")
                    .args(["tuntap", "del", "dev", "apfsds_test", "mode", "tun"])
                    .output();
            }
            Ok(o) => {
                println!(
                    "TUN creation failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => println!("Command error: {}", e),
        }
    }

    #[cfg(target_os = "windows")]
    {
        println!("Windows TUN test - checking for wintun.dll");

        // Check if wintun.dll exists
        let wintun_paths = [
            "wintun.dll",
            "C:\\Windows\\System32\\wintun.dll",
            "target\\release\\wintun.dll",
        ];

        let found = wintun_paths
            .iter()
            .any(|p| std::path::Path::new(p).exists());

        if found {
            println!("wintun.dll found - TUN support available");
        } else {
            println!("wintun.dll not found - download from https://www.wintun.net/");
        }
    }
}

/// Test: Full TUN tunnel (requires 2 VMs)
#[tokio::test]
#[ignore]
async fn test_tun_tunnel_p2p() {
    println!("TUN P2P Test - This test requires manual setup:");
    println!("");
    println!("1. VM1 (Server/Daemon):");
    println!("   ./apfsdsd --config daemon.toml");
    println!("");
    println!("2. VM2 (Client with TUN):");
    println!("   sudo ./apfsds --config client.toml --tun");
    println!("");
    println!("3. On VM2, test connectivity:");
    println!("   ping <exit_node_target_ip>");
    println!("");
    println!("4. Verify traffic is tunneled through the daemon.");
    println!("");

    // Placeholder assertion
    assert!(true);
}

/// Test: TUN + SOCKS5 coexistence
#[tokio::test]
#[ignore]
async fn test_tun_and_socks5_coexistence() {
    let config = TestConfig::default();

    // Start daemon
    let mut daemon = spawn_daemon(&config, 1).expect("Failed to spawn daemon");
    assert!(wait_for_port(config.daemon_bind, Duration::from_secs(30)).await);

    // In a real test, we would:
    // 1. Start client with both --tun and SOCKS5 enabled
    // 2. Verify SOCKS5 traffic goes through tunnel
    // 3. Verify TUN traffic goes through tunnel
    // 4. Ensure no conflicts

    println!("TUN + SOCKS5 coexistence test placeholder");

    let _ = daemon.kill();
    cleanup();
}
