# APFSDS

> "The M829A2 APFSDS-T can penetrate over 600mm of reinforced steel. Truth pierces all veils; no wall stands eternal."

**A** **P**rivacy-preserving **F**orwarding **S**ystem with **D**istributed **S**torage

## Features

- 🔐 **Multi-layer Encryption**: Ed25519 + AES-256-GCM + HMAC
- 🚀 **Zero-copy Serialization**: rkyv for high-performance frame processing
- 📦 **Distributed Storage**: MVCC + Raft consensus + ClickHouse backup
- 🎭 **Traffic Obfuscation**: Chrome WSS emulation, fake SSE/JSON, smart padding
- ⚡ **Split Architecture**: Handler ↔ Exit separation
- 🚨 **Emergency Mode**: crates.io yank-based trigger

## Project Structure

```
apfsds/
├── crates/
│   ├── protocol/       # Frame definitions and serialization
│   ├── crypto/         # Encryption and signing
│   ├── transport/      # WebSocket client/server
│   ├── obfuscation/    # Traffic obfuscation
│   └── storage/        # MVCC storage engine
├── client/             # Client binary (apfsds)
├── daemon/             # Server binary (apfsdsd)
├── helm-chart/         # Kubernetes deployment (TODO)
└── scripts/            # Install scripts (TODO)
```

## Quick Start

### Prerequisites

- Rust nightly (2024 edition)
- Linux or Windows

### Build

```bash
# Clone the repository
git clone https://github.com/user/apfsds
cd apfsds

# Build all crates
cargo build --release

# Run tests
cargo test --workspace
```

### Client

```bash
# Copy example config
cp config.example.toml config.toml

# Edit configuration
vim config.toml

# Run client
./target/release/apfsds --config config.toml
```

### Daemon

```bash
# Copy example config
cp daemon.example.toml daemon.toml

# Run as handler
./target/release/apfsdsd --config daemon.toml

# Run as exit node
./target/release/apfsdsd --config daemon.toml --exit
```

## Development Status

### Phase 1: Core Infrastructure ✅
- [x] Workspace setup
- [x] Protocol crate (ProxyFrame, Auth)
- [x] Crypto crate (Ed25519, AES-256-GCM, HMAC)
- [x] Transport crate (WSS client/server)
- [x] Obfuscation crate (XOR mask, padding, compression)
- [x] Storage crate (MVCC segments, B-link tree)
- [x] Client skeleton (SOCKS5, emergency mode)
- [x] Daemon skeleton (HTTP handler, metrics)

### Phase 2: Distributed System (TODO)
- [ ] Raft integration
- [ ] ClickHouse backup
- [ ] Exit node forwarding

### Phase 3: Security Polish (TODO)
- [ ] Full authentication flow
- [ ] Key rotation
- [ ] DoH over WSS

### Phase 4: Operations (TODO)
- [ ] Helm chart
- [ ] One-click install script
- [ ] Documentation

## License

MIT OR Apache-2.0
