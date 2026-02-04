# Configuration Reference

This document provides a complete reference for all configuration options in APFSDS.

## Table of Contents

- [Daemon Configuration](#daemon-configuration)
- [Client Configuration](#client-configuration)
- [Environment Variables](#environment-variables)

---

## Daemon Configuration

The daemon is configured via `daemon.toml`. Below is a complete reference.

### Server Section

```toml
[server]
bind = "0.0.0.0:25347"     # Listen address
mode = "handler"            # "handler" or "exit"
location = "US-East"        # Geographic location (optional)
reverse_mode = false        # Enable reverse connection mode (exit-node only)
handler_endpoint = "handler.example.com:25347"  # Handler to connect to (reverse mode)
preferred_group_id = 1      # Preferred proxy group (optional, reverse mode)
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `bind` | String | `0.0.0.0:25347` | Address to bind the main server |
| `mode` | String | `handler` | Operating mode: `handler` (controller) or `exit` (egress) |
| `location` | String | - | Geographic location for geo-routing |
| `reverse_mode` | bool | `false` | Enable reverse connection mode (for exit-nodes without public IP) |
| `handler_endpoint` | String | - | Handler endpoint to connect to (required when `reverse_mode = true`) |
| `preferred_group_id` | i32 | - | Preferred proxy group ID (optional, auto-selects by load if not set) |

### Raft Section

```toml
[raft]
node_id = 1                            # Unique node identifier
peers = ["192.168.1.2:25347", "192.168.1.3:25347"]
data_dir = "/var/lib/apfsds/raft"      # WAL storage directory
heartbeat_interval = 100               # ms
election_timeout_min = 150             # ms
election_timeout_max = 300             # ms
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `node_id` | u64 | `1` | Unique node ID in the Raft cluster |
| `peers` | String[] | `[]` | List of peer node addresses |
| `data_dir` | String | `./data` | Directory for WAL files |
| `heartbeat_interval` | u64 | `100` | Heartbeat interval in milliseconds |
| `election_timeout_min` | u64 | `150` | Minimum election timeout (ms) |
| `election_timeout_max` | u64 | `300` | Maximum election timeout (ms) |

### Storage Section

```toml
[storage]
disk_path = "/var/lib/apfsds"
tmpfs_path = "/dev/shm/apfsds"         # Optional: High-speed temp storage
tmpfs_size = 536870912                 # 512 MB

[storage.clickhouse]
enabled = true
url = "http://localhost:8123"
database = "apfsds"
username = "default"
password = ""
flush_interval = 5000                  # ms
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `disk_path` | String | `./data` | Primary storage directory |
| `tmpfs_path` | String | - | Optional tmpfs for hot data |
| `tmpfs_size` | u64 | `536870912` | tmpfs size limit in bytes |
| `clickhouse.enabled` | bool | `false` | Enable ClickHouse backup |
| `clickhouse.url` | String | - | ClickHouse HTTP interface URL |
| `clickhouse.database` | String | `apfsds` | Database name |
| `clickhouse.flush_interval` | u64 | `5000` | Batch flush interval (ms) |

### Database Section

```toml
[database]
url = "postgres://user:pass@localhost:5432/apfsds"
max_connections = 20
acquire_timeout = 3                    # seconds
```

### Security Section

```toml
[security]
token_ttl = 86400                      # Token validity (seconds)
key_rotation_interval = 604800         # 7 days
grace_period = 3600                    # Old key acceptance (seconds)

[security.emergency]
auto_trigger_dns = true
dns_domain = "signal.example.com"
check_interval = 300                   # seconds
crates_trigger = "apfsds-signal"       # Package to monitor
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `token_ttl` | u64 | `86400` | Auth token lifetime (seconds) |
| `key_rotation_interval` | u64 | `604800` | Key rotation period |
| `grace_period` | u64 | `3600` | Grace period for old keys |
| `emergency.auto_trigger_dns` | bool | `true` | Enable DNS-based emergency trigger |
| `emergency.crates_trigger` | String | - | crates.io package for kill-switch |

### Exit Nodes Section

```toml
[[exit_nodes]]
name = "exit-us-1"
endpoint = "203.0.113.1:25347"
weight = 1.0
location = "US-West"
group_id = 0                           # User group allowed to use this exit

[[exit_nodes]]
name = "exit-eu-1"
endpoint = "198.51.100.1:25347"
weight = 0.5
location = "EU-Frankfurt"
group_id = 1
```

### Monitoring Section

```toml
[monitoring]
prometheus_bind = "0.0.0.0:9090"
prometheus_enabled = true
```

---

## Client Configuration

The client is configured via `client.toml`.

### Client Section

```toml
[client]
mode = "socks5"                        # "socks5" or "tun"

[client.socks5]
bind = "127.0.0.1:1080"
udp_enabled = true

[client.tun]
name = "apfsds0"
address = "10.0.0.2/24"
mtu = 1500

[client.dns]
enabled = true
bind = "127.0.0.1:5353"
upstream = "1.1.1.1:53"
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mode` | String | `socks5` | Operating mode: `socks5` or `tun` |
| `socks5.bind` | String | `127.0.0.1:1080` | SOCKS5 server bind address |
| `socks5.udp_enabled` | bool | `true` | Enable SOCKS5 UDP association |
| `tun.name` | String | `apfsds0` | TUN interface name |
| `tun.address` | String | `10.0.0.2/24` | TUN interface IP/mask |
| `dns.enabled` | bool | `true` | Enable local DNS proxy |

### Connection Section

```toml
[connection]
endpoints = [
    "wss://handler1.example.com:25347/v1/connect",
    "wss://handler2.example.com:25347/v1/connect"
]
reconnect_delay = 1000                 # ms
max_reconnect_delay = 30000            # ms
keepalive_interval = 30000             # ms
```

### Emergency Section

```toml
[emergency]
enabled = true
check_interval = 300                   # seconds
```

---

## Environment Variables

These environment variables override configuration file values:

| Variable | Description |
|----------|-------------|
| `APFSDS_CONFIG` | Path to configuration file |
| `APFSDS_LOG_LEVEL` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `APFSDS_NODE_ID` | Override Raft node ID |
| `APFSDS_BIND` | Override bind address |
| `DATABASE_URL` | PostgreSQL connection string |
| `CLICKHOUSE_URL` | ClickHouse connection string |

---

## Example Configurations

### Minimal Handler

```toml
[server]
bind = "0.0.0.0:25347"

[raft]
node_id = 1
```

### 3-Node Cluster

See [Deployment Guide](deployment.md#3-node-cluster) for production cluster setup.

### Exit Node (Traditional Mode)

```toml
[server]
bind = "0.0.0.0:25347"
mode = "exit"
location = "US-West"
```

### Exit Node (Reverse Connection Mode)

For exit-nodes without public IP addresses:

```toml
[server]
mode = "exit"
reverse_mode = true
handler_endpoint = "handler.example.com:25347"
location = "exit-node-us-1"

# Option 1: Auto-select group by load (omit preferred_group_id)
# The exit-node will automatically join the group with lowest load

# Option 2: Manually specify group
preferred_group_id = 1  # Join group 1 (Premium)
```

**Available Groups:**
- `0`: Default group
- `1`: Premium group
- `2`: Asia group

If the specified `preferred_group_id` doesn't exist, the exit-node will fall back to auto-selection.

