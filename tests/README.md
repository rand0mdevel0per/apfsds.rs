# APFSDS Integration Tests

This directory contains integration and end-to-end tests for the APFSDS system.

## Test Categories

### 1. Handshake Tests (`handshake.rs`)
Tests the authentication flow between client and daemon:
- WebSocket upgrade
- Token validation
- Management API health

### 2. SOCKS5 Tunnel Tests (`socks5_tunnel.rs`)
Tests the SOCKS5 proxy functionality:
- SOCKS5 handshake
- CONNECT to external hosts
- Concurrent connections

### 3. Raft Cluster Tests (`raft_cluster.rs`)
Tests distributed consensus:
- 3-node cluster formation
- Leader failover
- Network partition recovery

### 4. TUN Mode Tests (`tun_mode.rs`)
Tests the VPN-like TUN device mode:
- TUN device creation (requires root)
- P2P tunnel connectivity
- SOCKS5 + TUN coexistence

## Running Tests

### Unit Tests (Fast)
```bash
cargo test --workspace
```

### Integration Tests (Slow)
Integration tests are marked with `#[ignore]` and require manual invocation:

```bash
# Run all integration tests
cargo test --test handshake -- --ignored
cargo test --test socks5_tunnel -- --ignored
cargo test --test raft_cluster -- --ignored
cargo test --test tun_mode -- --ignored

# Run a specific test
cargo test --test handshake test_daemon_accepts_websocket -- --ignored
```

### VMware P2P Testing
For testing across multiple VMs:

1. **Deploy nodes:**
   ```bash
   export NODE1_IP=192.168.1.101
   export NODE2_IP=192.168.1.102
   export NODE3_IP=192.168.1.103
   export SSH_USER=ubuntu
   ./deploy/vmware_deploy.sh
   ```

2. **Simulate network partition:**
   ```bash
   # Isolate node 1
   ./deploy/partition_test.sh isolate 192.168.1.101

   # Watch for leader election (check node 2 or 3 dashboard)
   curl http://192.168.1.102:25348/admin/stats

   # Heal partition
   ./deploy/partition_test.sh heal
   ```

3. **Manual TUN test:**
   - On VM1: Start daemon
   - On VM2: Start client with `--tun`
   - On VM2: `ping` through the tunnel

## Test Configuration

Tests use port ranges to avoid conflicts:
- Integration tests: 35000-35999
- Raft cluster tests: 45000-46999

## Requirements

- `reqwest` crate (for HTTP tests)
- `futures` crate (for concurrent tests)
- Root/Administrator privileges (for TUN tests)
- SSH access to VMs (for VMware tests)
