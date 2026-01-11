# APFSDS Architecture

APFSDS (Advanced Protocol for Secure Distributed Systems) is a censorship-resistant distributed proxy network designed for high availability and privacy.

## Core Components

### 1. Daemon (Controller/Handler)
The Daemon is the brain of the cluster.
- **Role**: Manages user authentication, node registry, and cluster state via Raft consensus.
- **Consensus**: Uses `async-raft` to replicate state (accounts, token blacklists, exit node registry) across the controller cluster.
- **Persistence**: Hybrid storage using PostgreSQL (Relational Data) and ClickHouse (Traffic Logs/Analytics).
- **Control Plane**: Exposes a Management API and an embedded Web Dashboard.

### 2. Client
The user-facing proxy application.
- **Modes**: SOCKS5 Proxy, TUN Device (VPN-like), Mobile Library (FFI).
- **Obfuscation**: Wraps all traffic in a custom encrypted frame protocol to evade DPI.
- **Local DNS**: Intercepts UDP DNS queries and tunnels them securely to the daemon.

### 3. Exit Node
The edge node that egresses traffic to the internet.
- **Role**: Receives decrypted traffic from the Daemon/Handler and forwards it to the destination.
- **Transport**: Connects to Daemon via mutually authenticated (mTLS) QUIC/HTTP3 tunnels.

## Protocol & Security

### Secure Transport Layer
- **Handshake**: X25519 Elliptic Curve Diffie-Hellman (ECDH) for session key derivation.
- **Encryption**: AES-256-GCM for all payload encryption.
- **Authentication**: HMAC-SHA256 based Token system with replay protection (XOR Replay Filter).

### Obfuscation (Anti-DPI)
- **Structure**: `Length | Flags | UUID | Payload | Checksum`.
- **Masking**: The entire frame (including headers) is XOR-masked with a session-derived rolling key.
- **Padding**: PKCS#7 padding with random jitter to hide packet size fingerprints.
- **Noise**: The Daemon injects fake JSON responses and SSE keepalives to mimic legitimate HTTP web traffic.

### Resilience
- **Raft Consensus**: Leader election and log replication ensure no single point of failure for the control plane.
- **Emergency Mode**: Clients monitor a `crates.io` package version. If a "poison pill" version is published, clients automatically sever connections (Kill Switch).

## Data Flow

`Client (SOCKS5/TUN)` -> `[Obfuscated Tunnel]` -> `Daemon (Handler)` -> `[mTLS QUIC]` -> `Exit Node` -> `Internet`
