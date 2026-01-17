# Architecture

This document describes the high-level architecture and design principles of APFSDS.

## Table of Contents

- [Overview](#overview)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Crate Dependencies](#crate-dependencies)
- [Design Decisions](#design-decisions)

---

## Overview

APFSDS is a privacy-preserving proxy system designed for:

- **Censorship resistance**: Traffic obfuscation defeats DPI
- **High availability**: Raft consensus ensures no single point of failure
- **Performance**: Zero-copy serialization with rkyv
- **Security**: Multi-layer encryption with forward secrecy

### System Diagram

```
                                 ┌─────────────────────┐
                                 │      Internet       │
                                 └──────────▲──────────┘
                                            │
┌───────────────────────────────────────────┼───────────────────────────────────┐
│                                           │                                    │
│  Exit Node Pool                           │                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐│                                    │
│  │ Exit US  │  │ Exit EU  │  │Exit Asia ││                                    │
│  └────▲─────┘  └────▲─────┘  └────▲─────┘│                                    │
│       │             │             │       │                                    │
│       └─────────────┼─────────────┘       │                                    │
│                     │ mTLS QUIC           │                                    │
│                     ▼                     │                                    │
│  ┌─────────────────────────────────────┐  │                                    │
│  │          Handler Cluster            │  │                                    │
│  │  ┌─────────┐ ┌─────────┐ ┌────────┐ │  │                                    │
│  │  │ Node 1  │◄┤  Raft   ├►│ Node 3 │ │  │                                    │
│  │  │ (Leader)│ │Consensus│ │        │ │  │                                    │
│  │  └────┬────┘ └────┬────┘ └────┬───┘ │  │                                    │
│  │       └───────────┼───────────┘     │  │                                    │
│  │                   ▼                 │  │                                    │
│  │            ┌───────────┐            │  │                                    │
│  │            │  Storage  │            │  │                                    │
│  │            │ WAL + CH  │            │  │                                    │
│  │            └───────────┘            │  │                                    │
│  └─────────────────▲───────────────────┘  │                                    │
│                    │ WSS (Obfuscated)     │                                    │
│                    │                      │                                    │
└────────────────────┼──────────────────────┘                                    │
                     │                                                           │
    ┌────────────────┴────────────────┐                                          │
    │           Client                │                                          │
    │  ┌───────┐ ┌─────┐ ┌─────────┐ │                                          │
    │  │SOCKS5 │ │ TUN │ │EmergMode│ │                                          │
    │  └───────┘ └─────┘ └─────────┘ │                                          │
    └─────────────────────────────────┘                                          │
```

---

## Core Components

### Client (`client/`)

The user-facing application that tunnels traffic:

| Module | Purpose |
|--------|---------|
| `socks5.rs` | SOCKS5 proxy server |
| `tun_device.rs` | TUN interface for VPN mode |
| `wss.rs` | WebSocket connection to handler |
| `local_dns.rs` | DNS interception and forwarding |
| `emergency.rs` | Kill-switch monitoring |

### Daemon (`daemon/`)

The server component running as handler or exit:

| Module | Purpose |
|--------|---------|
| `handler.rs` | Client connection handling, auth |
| `exit_node.rs` | Traffic egress, NAT |
| `management.rs` | Admin API, dashboard |
| `auth.rs` | Token verification |
| `key_rotation.rs` | Periodic key refresh |

### Protocol (`crates/protocol/`)

Wire format definitions:

```rust
pub struct ProxyFrame {
    pub conn_id: u64,
    pub flags: FrameFlags,
    pub payload: Bytes,
}

pub enum ControlMessage {
    DohQuery { domain: String },
    DohResponse { records: Vec<u8> },
    Ping,
    Pong,
    KeyRotation { new_public_key: [u8; 32] },
    Emergency { level: EmergencyLevel },
}
```

### Crypto (`crates/crypto/`)

Cryptographic primitives:

- `ecdh.rs` - X25519 key exchange
- `aes_gcm.rs` - AES-256-GCM encryption
- `hmac_auth.rs` - HMAC-SHA256 tokens
- `xor_filter.rs` - Replay protection

### Transport (`crates/transport/`)

Network layer abstractions:

- `wss.rs` - WebSocket client/server
- `quic.rs` - QUIC transport (handler ↔ exit)
- `ssh.rs` - SSH tunnel fallback

### Obfuscation (`crates/obfuscation/`)

Traffic masking:

- `masker.rs` - XOR masking with rolling key
- `padding.rs` - Size obfuscation
- `timing.rs` - Timing randomization

### Storage (`crates/storage/`)

Persistence layer:

- `engine.rs` - MVCC storage engine
- `segment.rs` - Log-structured segments
- `wal.rs` - Write-ahead log
- `clickhouse_backup.rs` - Analytics export

### Raft (`crates/raft/`)

Distributed consensus:

- `node.rs` - Raft node wrapper
- `storage.rs` - RaftStorage implementation
- `network.rs` - Peer communication

---

## Data Flow

### Client → Handler → Exit → Internet

1. **Client**: Application sends to SOCKS5 proxy
2. **Encryption**: Payload encrypted with AES-256-GCM
3. **Obfuscation**: XOR mask + padding applied
4. **Transport**: Sent over WSS to handler
5. **Handler**: Verifies auth, records in storage
6. **Routing**: Selects exit node based on policy
7. **Exit**: Decrypts, forwards to destination
8. **Response**: Reverse path back to client

### Authentication Flow

```
Client                          Handler
   │                               │
   │──── AuthRequest ─────────────►│
   │     (encrypted with           │
   │      server public key)       │
   │                               │
   │                               ├─► Verify token
   │                               ├─► Check replay filter
   │                               ├─► Create session
   │                               │
   │◄──── AuthResponse ────────────│
   │      (session token)          │
   │                               │
   │════ Encrypted Data ══════════►│
```

---

## Crate Dependencies

```
                    ┌─────────────┐
                    │   daemon    │
                    └──────┬──────┘
                           │
       ┌───────────────────┼───────────────────┐
       │                   │                   │
       ▼                   ▼                   ▼
┌────────────┐      ┌───────────┐       ┌───────────┐
│  transport │      │    raft   │       │  storage  │
└─────┬──────┘      └─────┬─────┘       └─────┬─────┘
      │                   │                   │
      ├───────────────────┼───────────────────┤
      │                   │                   │
      ▼                   ▼                   ▼
┌───────────┐       ┌───────────┐       ┌───────────┐
│obfuscation│       │  protocol │       │   crypto  │
└───────────┘       └───────────┘       └───────────┘
```

---

## Design Decisions

### Why Rkyv?

Zero-copy deserialization eliminates allocation overhead:

```rust
// Traditional (serde): Allocates new memory
let frame: ProxyFrame = serde_json::from_slice(&data)?;

// Rkyv: Zero-copy, works directly on buffer
let frame = rkyv::access::<ProxyFrame>(&data)?;
```

### Why Raft?

- Simple to reason about
- Strong consistency guarantees
- Well-tested with `async-raft`
- Good for small clusters (3-7 nodes)

### Why Split Handler/Exit?

- **Defense in depth**: Exit nodes don't see user tokens
- **Reduced attack surface**: Handler cluster is HA
- **Flexibility**: Exit nodes can be ephemeral

### Why WebSocket?

- Passes through most firewalls/proxies
- Easily obfuscated as browser traffic
- Supports TLS naturally
- Bidirectional streaming

---

## Future Considerations

- **QUIC for client connection**: Better performance, UDP support
- **Multi-region consensus**: Geo-distributed Raft groups
- **Zero-knowledge proofs**: For anonymous authentication
- **Hardware security**: HSM integration for key storage
