<div align="center">

# 🚀 APFSDS

**A Privacy-preserving Forwarding System with Distributed Storage**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/rand0mdevel0per/apfsds.rs)

*"The M829A2 APFSDS-T can penetrate over 600mm of reinforced steel. Truth pierces all veils; no wall stands eternal."*

[Documentation](docs/) • [Getting Started](#-quick-start) • [Architecture](docs/architecture.md) • [API Reference](docs/api.md)

</div>

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🔐 **Multi-layer Encryption** | X25519 key exchange + AES-256-GCM + Ed25519 signatures |
| 🚀 **Zero-copy Serialization** | `rkyv` for ultra-fast frame processing |
| 📦 **Distributed Consensus** | Raft-based cluster with WAL persistence |
| 🎭 **Traffic Obfuscation** | WSS masking, fake SSE/JSON, smart padding |
| ⚡ **Split Architecture** | Handler ↔ Exit node separation |
| 🌐 **Multiple Transports** | WebSocket, QUIC, SSH tunneling |
| 🚨 **Emergency Mode** | Remote kill-switch via crates.io |
| 📊 **Observability** | Prometheus metrics, ClickHouse analytics |

## 📁 Project Structure

```
apfsds/
├── crates/
│   ├── protocol/        # Wire protocol & frame definitions
│   ├── crypto/          # Cryptographic primitives
│   ├── transport/       # Network transports (WSS, QUIC, SSH)
│   ├── obfuscation/     # Traffic obfuscation layer
│   ├── storage/         # MVCC storage engine
│   └── raft/            # Distributed consensus
├── daemon/              # Server binary (apfsdsd)
├── client/              # Client binary (apfsds)
├── cli/                 # Management CLI
├── helm-chart/          # Kubernetes deployment
├── deploy/              # Deployment scripts
├── docs/                # Documentation
└── tests/               # Integration tests
```

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.85+ (2024 edition)
- **Platform**: Linux, Windows, or macOS

### Installation

```bash
# Clone repository
git clone https://github.com/rand0mdevel0per/apfsds.rs.git
cd apfsds

# Build release binaries
cargo build --release

# Optional: Install globally
sudo cp target/release/apfsdsd /usr/local/bin/
sudo cp target/release/apfsds /usr/local/bin/
```

### One-liner Install

```bash
curl -sSL https://raw.githubusercontent.com/rand0mdevel0per/apfsds.rs/master/deploy/install.sh | bash
```

### Running the Daemon

```bash
# Start as handler node
./target/release/apfsdsd --config daemon.toml

# Start as exit node
./target/release/apfsdsd --config daemon.toml --exit

# Access dashboard
open http://localhost:25348/
```

### Running the Client

```bash
# SOCKS5 mode (default)
./target/release/apfsds --config client.toml

# Configure your browser to use SOCKS5 proxy at 127.0.0.1:1080
```

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | System design, components, data flow |
| [Configuration](docs/configuration.md) | Full configuration reference |
| [API Reference](docs/api.md) | Management API endpoints |
| [User Guide](docs/user-guide.md) | Installation and usage guide |
| [Deployment](docs/deployment.md) | Kubernetes & production deployment |
| [Security](docs/security.md) | Security model and threat mitigation |
| [Contributing](CONTRIBUTING.md) | How to contribute |

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Client                                │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │ SOCKS5  │  │   TUN   │  │   DNS   │  │ Emergency Mode  │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────────┬────────┘ │
└───────┼────────────┼────────────┼────────────────┼──────────┘
        │            │            │                │
        └────────────┴──────┬─────┴────────────────┘
                            │ Obfuscated WSS
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Handler Cluster                           │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                      │
│  │ Node 1  │◄─┤  Raft   ├─►│ Node 3  │  (Consensus)         │
│  └────┬────┘  └────┬────┘  └────┬────┘                      │
│       └────────────┼────────────┘                            │
│                    ▼                                         │
│             ┌────────────┐                                   │
│             │  Storage   │  (WAL + ClickHouse)               │
│             └──────┬─────┘                                   │
└────────────────────┼────────────────────────────────────────┘
                     │ mTLS QUIC
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                      Exit Nodes                              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                      │
│  │ Exit-US │  │Exit-EU  │  │Exit-Asia│                      │
│  └────┬────┘  └────┬────┘  └────┬────┘                      │
└───────┼────────────┼────────────┼───────────────────────────┘
        │            │            │
        └────────────┴──────┬─────┘
                            │
                            ▼
                       🌐 Internet
```

## 🔧 Configuration

### Daemon (`daemon.toml`)

```toml
[server]
bind = "0.0.0.0:25347"
mode = "handler"  # or "exit"

[raft]
node_id = 1
peers = ["192.168.1.2:25347", "192.168.1.3:25347"]

[storage]
disk_path = "/var/lib/apfsds"

[security]
key_rotation_interval = 86400  # 24 hours
```

### Client (`client.toml`)

```toml
[client]
mode = "socks5"

[client.socks5]
bind = "127.0.0.1:1080"

[connection]
endpoints = ["wss://handler.example.com:25347/v1/connect"]
```

See [Configuration Guide](docs/configuration.md) for full reference.

## 🐳 Kubernetes Deployment

```bash
# Add Helm repository
helm repo add apfsds https://raw.githubusercontent.com/rand0mdevel0per/apfsds.rs/master/deploy/repo

# Install
helm install apfsds apfsds/apfsds \
  --set deployment.replicas=3 \
  --set storage.clickhouse.enabled=true
```

See [Deployment Guide](docs/deployment.md) for production setup.

## 🧪 Testing

```bash
# Unit tests
cargo test --workspace

# Integration tests (requires running daemon)
cargo test -p apfsds-tests --test handshake -- --ignored
cargo test -p apfsds-tests --test raft_cluster -- --ignored

# VMware multi-node tests
./deploy/vmware_deploy.sh
```

## 📦 Crates

| Crate | Description | crates.io |
|-------|-------------|-----------|
| `apfsds-protocol` | Wire protocol definitions | [![](https://img.shields.io/crates/v/apfsds-protocol.svg)](https://crates.io/crates/apfsds-protocol) |
| `apfsds-crypto` | Cryptographic primitives | [![](https://img.shields.io/crates/v/apfsds-crypto.svg)](https://crates.io/crates/apfsds-crypto) |
| `apfsds-obfuscation` | Traffic obfuscation | [![](https://img.shields.io/crates/v/apfsds-obfuscation.svg)](https://crates.io/crates/apfsds-obfuscation) |
| `apfsds-transport` | Network transports | [![](https://img.shields.io/crates/v/apfsds-transport.svg)](https://crates.io/crates/apfsds-transport) |
| `apfsds-storage` | MVCC storage engine | [![](https://img.shields.io/crates/v/apfsds-storage.svg)](https://crates.io/crates/apfsds-storage) |
| `apfsds-raft` | Raft consensus | [![](https://img.shields.io/crates/v/apfsds-raft.svg)](https://crates.io/crates/apfsds-raft) |

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) first.

```bash
# Fork and clone
git clone https://github.com/YOUR_USERNAME/apfsds.rs.git

# Create feature branch
git checkout -b feature/amazing-feature

# Make changes and test
cargo test --workspace

# Submit PR
```

## 📄 License

**MIT License** ([LICENSE-MIT](LICENSE-MIT))

---

<div align="center">

**[⬆ Back to Top](#-apfsds)**

Made with ❤️ by the APFSDS Team

</div>
