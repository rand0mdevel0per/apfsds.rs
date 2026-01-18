# apfsds-daemon

Server daemon for APFSDS (apfsdsd).

## Features

- **Handler Mode**: Client connection handling, authentication, routing
- **Exit Mode**: Traffic egress to internet
- **Raft Cluster**: Distributed consensus for HA
- **Management API**: REST API for administration
- **Metrics**: Prometheus-compatible endpoint

## Installation

```bash
cargo install apfsds-daemon
```

## Usage

```bash
# Run as handler
apfsdsd --config daemon.toml

# Run as exit node
apfsdsd --config daemon.toml --exit
```

## Configuration

```toml
[server]
bind = "0.0.0.0:25347"
mode = "handler"

[raft]
node_id = 1
peers = ["192.168.1.2:25347", "192.168.1.3:25347"]

[storage]
disk_path = "/var/lib/apfsds"

[monitoring]
prometheus_bind = "0.0.0.0:9090"
```

## Endpoints

| Port | Purpose |
|------|---------|
| 25347 | Main server (client connections) |
| 25348 | Management API |
| 9090 | Prometheus metrics |

## License

MIT
