# Deployment Guide

This guide covers deploying APFSDS in production environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Single Node Setup](#single-node-setup)
- [3-Node Cluster](#3-node-cluster)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Docker Compose](#docker-compose)
- [Monitoring](#monitoring)
- [Scaling](#scaling)

---

## Prerequisites

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 2 GB | 8+ GB |
| Disk | 20 GB SSD | 100+ GB NVMe |
| Network | 100 Mbps | 1+ Gbps |

### Software Requirements

- Linux (Ubuntu 22.04+, Debian 12+, RHEL 9+) or Windows Server 2019+
- PostgreSQL 14+ (for user management)
- ClickHouse 23+ (optional, for analytics)

---

## Single Node Setup

### 1. Install Binary

```bash
# Download latest release
curl -LO https://github.com/rand0mdevel0per/apfsds.rs/releases/latest/download/apfsdsd-linux-amd64
chmod +x apfsdsd-linux-amd64
sudo mv apfsdsd-linux-amd64 /usr/local/bin/apfsdsd
```

### 2. Create Configuration

```bash
sudo mkdir -p /etc/apfsds /var/lib/apfsds
sudo cat > /etc/apfsds/daemon.toml << 'EOF'
[server]
bind = "0.0.0.0:25347"
mode = "handler"

[raft]
node_id = 1

[storage]
disk_path = "/var/lib/apfsds"

[monitoring]
prometheus_bind = "0.0.0.0:9090"
EOF
```

### 3. Create Systemd Service

```bash
sudo cat > /etc/systemd/system/apfsdsd.service << 'EOF'
[Unit]
Description=APFSDS Daemon
After=network.target

[Service]
Type=simple
User=apfsds
ExecStart=/usr/local/bin/apfsdsd --config /etc/apfsds/daemon.toml
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

sudo useradd -r -s /bin/false apfsds
sudo chown -R apfsds:apfsds /var/lib/apfsds
sudo systemctl daemon-reload
sudo systemctl enable --now apfsdsd
```

---

## 3-Node Cluster

### Network Layout

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Node 1    │  │   Node 2    │  │   Node 3    │
│ 192.168.1.1 │  │ 192.168.1.2 │  │ 192.168.1.3 │
│   (Leader)  │  │  (Follower) │  │  (Follower) │
└─────────────┘  └─────────────┘  └─────────────┘
```

### Node 1 Configuration

```toml
[server]
bind = "0.0.0.0:25347"

[raft]
node_id = 1
peers = ["192.168.1.2:25347", "192.168.1.3:25347"]
```

### Node 2 Configuration

```toml
[server]
bind = "0.0.0.0:25347"

[raft]
node_id = 2
peers = ["192.168.1.1:25347", "192.168.1.3:25347"]
```

### Node 3 Configuration

```toml
[server]
bind = "0.0.0.0:25347"

[raft]
node_id = 3
peers = ["192.168.1.1:25347", "192.168.1.2:25347"]
```

### Verify Cluster

```bash
# Check cluster status on any node
curl http://192.168.1.1:25348/admin/stats
```

---

## Kubernetes Deployment

### Prerequisites

- Kubernetes 1.25+
- Helm 3.10+
- Persistent Volume provisioner

### Install via Helm

```bash
# Add repository
helm repo add apfsds https://raw.githubusercontent.com/rand0mdevel0per/apfsds.rs/master/deploy/repo
helm repo update

# Install with default values
helm install apfsds apfsds/apfsds

# Install with custom values
helm install apfsds apfsds/apfsds \
  --set deployment.replicas=3 \
  --set storage.clickhouse.enabled=true \
  --set server.handler.location="US-East"
```

### Custom Values

Create `values.yaml`:

```yaml
deployment:
  replicas: 3
  mode: handler

server:
  bind: "0.0.0.0:25347"
  handler:
    location: "US-East"

storage:
  clickhouse:
    enabled: true
  database:
    host: postgres-service
    port: 5432

monitoring:
  prometheus:
    enabled: true
    port: 9090

resources:
  requests:
    memory: "512Mi"
    cpu: "250m"
  limits:
    memory: "2Gi"
    cpu: "2"
```

```bash
helm install apfsds apfsds/apfsds -f values.yaml
```

### Verify Deployment

```bash
kubectl get pods -l app=apfsds
kubectl logs -f deployment/apfsds
```

---

## Docker Compose

### docker-compose.yml

```yaml
version: '3.8'

services:
  apfsds-handler:
    image: ghcr.io/rand0mdevel0per/apfsds:latest
    ports:
      - "25347:25347"   # Main
      - "25348:25348"   # Management
      - "9090:9090"     # Prometheus
    volumes:
      - ./daemon.toml:/etc/apfsds/daemon.toml:ro
      - apfsds-data:/var/lib/apfsds
    environment:
      - RUST_LOG=info
    depends_on:
      - postgres
      - clickhouse

  postgres:
    image: postgres:16
    environment:
      POSTGRES_DB: apfsds
      POSTGRES_USER: apfsds
      POSTGRES_PASSWORD: secret
    volumes:
      - pg-data:/var/lib/postgresql/data

  clickhouse:
    image: clickhouse/clickhouse-server:24
    volumes:
      - ch-data:/var/lib/clickhouse

volumes:
  apfsds-data:
  pg-data:
  ch-data:
```

```bash
docker-compose up -d
```

---

## Monitoring

### Prometheus Metrics

The daemon exposes metrics at `:9090/metrics`:

```
# Active connections
apfsds_active_connections

# Bytes transferred
apfsds_bytes_rx_total
apfsds_bytes_tx_total

# Request latency histogram
apfsds_request_duration_seconds_bucket

# Raft metrics
apfsds_raft_term
apfsds_raft_committed_index
apfsds_raft_applied_index
```

### Grafana Dashboard

Import the dashboard from `deploy/grafana-dashboard.json`.

### Alerting Rules

```yaml
groups:
  - name: apfsds
    rules:
      - alert: HighErrorRate
        expr: rate(apfsds_errors_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning

      - alert: RaftLeaderLost
        expr: apfsds_raft_is_leader == 0
        for: 1m
        labels:
          severity: critical
```

---

## Scaling

### Horizontal Scaling (Handlers)

Add more handler nodes to the Raft cluster:

```bash
# On new node
./apfsdsd --config daemon.toml

# Via Management API
curl -X POST http://leader:25348/admin/cluster/membership \
  -H "Content-Type: application/json" \
  -d '{"members": [1, 2, 3, 4]}'
```

### Exit Node Scaling

Deploy exit nodes in multiple regions:

```toml
[[exit_nodes]]
name = "exit-us-west"
endpoint = "us-west.example.com:25347"
weight = 1.0
location = "US-West"

[[exit_nodes]]
name = "exit-eu-central"
endpoint = "eu.example.com:25347"
weight = 1.0
location = "EU-Frankfurt"
```

---

## Security Hardening

See [Security Guide](security.md) for:
- TLS configuration
- Firewall rules
- Key management
- Audit logging
