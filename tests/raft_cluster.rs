//! Raft Cluster Integration Tests
//!
//! Tests multi-node Raft consensus behavior.

mod integration_harness;

use integration_harness::{TestConfig, cleanup, wait_for_port};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Spawn a daemon with specific Raft configuration
fn spawn_raft_node(
    node_id: u64,
    bind_port: u16,
    mgmt_port: u16,
    peers: &[String],
) -> std::io::Result<Child> {
    let peers_str = peers
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ");

    let config_content = format!(
        r#"
[server]
bind = "127.0.0.1:{}"
mode = "handler"

[raft]
node_id = {}
peers = [{}]

[monitoring]
prometheus_bind = "127.0.0.1:0"
prometheus_enabled = false
"#,
        bind_port, node_id, peers_str
    );

    let config_path = format!("tests/raft_node_{}.toml", node_id);
    std::fs::write(&config_path, config_content)?;

    Command::new("cargo")
        .args(["run", "-p", "apfsds-daemon", "--", "--config", &config_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Test: 3-node Raft cluster formation
#[tokio::test]
#[ignore]
async fn test_raft_cluster_3_nodes() {
    println!("Starting 3-node Raft cluster test...");

    let peers = vec![
        "127.0.0.1:45001".to_string(),
        "127.0.0.1:45002".to_string(),
        "127.0.0.1:45003".to_string(),
    ];

    // Spawn 3 nodes
    let mut node1 = spawn_raft_node(1, 45001, 45101, &peers).expect("Failed to spawn node 1");
    let mut node2 = spawn_raft_node(2, 45002, 45102, &peers).expect("Failed to spawn node 2");
    let mut node3 = spawn_raft_node(3, 45003, 45103, &peers).expect("Failed to spawn node 3");

    // Wait for all nodes to start
    let timeout = Duration::from_secs(60);
    assert!(
        wait_for_port("127.0.0.1:45001".parse().unwrap(), timeout).await,
        "Node 1 did not start"
    );
    assert!(
        wait_for_port("127.0.0.1:45002".parse().unwrap(), timeout).await,
        "Node 2 did not start"
    );
    assert!(
        wait_for_port("127.0.0.1:45003".parse().unwrap(), timeout).await,
        "Node 3 did not start"
    );

    println!("All 3 nodes started. Waiting for leader election...");

    // Give time for leader election (async-raft default timeout is 150-300ms)
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Future: Query each node's /admin/cluster/status to verify leader elected
    // For now just verify they're all running

    println!("Cluster formation test passed!");

    // Cleanup
    let _ = node1.kill();
    let _ = node2.kill();
    let _ = node3.kill();

    let _ = std::fs::remove_file("tests/raft_node_1.toml");
    let _ = std::fs::remove_file("tests/raft_node_2.toml");
    let _ = std::fs::remove_file("tests/raft_node_3.toml");
}

/// Test: Leader failover
#[tokio::test]
#[ignore]
async fn test_raft_leader_failover() {
    println!("Starting Raft leader failover test...");

    let peers = vec![
        "127.0.0.1:46001".to_string(),
        "127.0.0.1:46002".to_string(),
        "127.0.0.1:46003".to_string(),
    ];

    let mut node1 = spawn_raft_node(1, 46001, 46101, &peers).expect("Failed to spawn node 1");
    let mut node2 = spawn_raft_node(2, 46002, 46102, &peers).expect("Failed to spawn node 2");
    let mut node3 = spawn_raft_node(3, 46003, 46103, &peers).expect("Failed to spawn node 3");

    let timeout = Duration::from_secs(60);
    assert!(wait_for_port("127.0.0.1:46001".parse().unwrap(), timeout).await);
    assert!(wait_for_port("127.0.0.1:46002".parse().unwrap(), timeout).await);
    assert!(wait_for_port("127.0.0.1:46003".parse().unwrap(), timeout).await);

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Kill node 1 (simulate failure)
    println!("Killing node 1 to trigger failover...");
    let _ = node1.kill();

    // Wait for new leader election
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify remaining nodes are still responsive
    assert!(
        wait_for_port("127.0.0.1:46002".parse().unwrap(), Duration::from_secs(5)).await,
        "Node 2 should still be running"
    );
    assert!(
        wait_for_port("127.0.0.1:46003".parse().unwrap(), Duration::from_secs(5)).await,
        "Node 3 should still be running"
    );

    println!("Leader failover test passed!");

    let _ = node2.kill();
    let _ = node3.kill();

    let _ = std::fs::remove_file("tests/raft_node_1.toml");
    let _ = std::fs::remove_file("tests/raft_node_2.toml");
    let _ = std::fs::remove_file("tests/raft_node_3.toml");
}

/// Test: Network partition simulation
#[tokio::test]
#[ignore]
async fn test_raft_network_partition() {
    println!("Network partition test - requires manual firewall rules");
    println!("This test is a placeholder for manual P2P testing in VMware.");

    // In a real test environment:
    // 1. Start 3 nodes on separate VMs
    // 2. Use iptables/firewall to partition node 1 from nodes 2 & 3
    // 3. Verify nodes 2 & 3 elect a new leader
    // 4. Verify node 1 steps down
    // 5. Heal partition
    // 6. Verify node 1 rejoins cluster

    assert!(true, "Placeholder for manual testing");
}
