# Security Model

This document describes the security architecture and threat mitigation strategies of APFSDS.

## Table of Contents

- [Threat Model](#threat-model)
- [Cryptographic Design](#cryptographic-design)
- [Authentication](#authentication)
- [Traffic Obfuscation](#traffic-obfuscation)
- [Emergency Mode](#emergency-mode)
- [Operational Security](#operational-security)

---

## Threat Model

### Adversary Capabilities

APFSDS is designed to resist the following adversary capabilities:

| Capability | Mitigation |
|------------|------------|
| Deep Packet Inspection (DPI) | Traffic obfuscation, WSS masking |
| Traffic Analysis | Padding, timing randomization, fake traffic |
| Active MITM | Mutual authentication, certificate pinning |
| Passive Collection | End-to-end encryption, forward secrecy |
| Server Compromise | Split architecture, minimal logging |
| Codebase Seizure | Emergency mode, remote kill-switch |

### Trust Boundaries

```
┌──────────────────────────────────────────┐
│           Trusted: Client Device          │
│  ┌─────────────────────────────────────┐ │
│  │        Untrusted: Network           │ │
│  │  ┌─────────────────────────────────┐│ │
│  │  │  Semi-trusted: Handler Cluster  ││ │
│  │  │  ┌───────────────────────────┐  ││ │
│  │  │  │ Untrusted: Exit Traffic   │  ││ │
│  │  │  └───────────────────────────┘  ││ │
│  │  └─────────────────────────────────┘│ │
│  └─────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

---

## Cryptographic Design

### Key Exchange

**Post-Quantum Security:**
1. **ML-KEM-768 (Kyber)**: Post-quantum key encapsulation mechanism
2. **Hybrid Mode**: ML-KEM-768 + X25519 for defense-in-depth
3. **Forward Secrecy**: Fresh ephemeral keys per session

**Classical Fallback:**
1. **Ephemeral X25519**: Each session generates fresh keypairs
2. **ECDH**: Shared secret derived from curve25519
3. **HKDF**: Key derivation for symmetric keys

```
Client                           Server
   │                                │
   │──── ClientHello (ephemeral) ──►│
   │                                │
   │◄─── ServerHello (ephemeral) ───│
   │                                │
   │     Shared Secret = X25519(    │
   │       client_private,          │
   │       server_public            │
   │     )                          │
   │                                │
   │     AES Key = HKDF(shared)     │
```

### Encryption

| Layer | Algorithm | Purpose |
|-------|-----------|---------|
| Key Exchange (PQ) | ML-KEM-768 | Post-quantum key encapsulation |
| Key Exchange | X25519 | Forward secrecy (classical) |
| Payload | AES-256-GCM | Authenticated encryption |
| Signing | Ed25519 / ML-DSA-65 | Identity verification |
| MAC | HMAC-SHA256 | Token integrity |

### Forward Secrecy

- New ephemeral keys per session
- Keys rotated periodically (configurable, default 7 days)
- Old keys retained briefly for graceful transition

---

## Authentication

### Token-Based Authentication

1. Client presents token in `AuthRequest`
2. Server validates HMAC and expiration
3. Server checks replay filter (XOR-based)
4. Success: Client receives `AuthResponse` with session token

### Token Structure

```
┌────────────────────────────────────────────────────┐
│  User ID (8 bytes)                                 │
├────────────────────────────────────────────────────┤
│  Valid Until (8 bytes, Unix timestamp)             │
├────────────────────────────────────────────────────┤
│  Nonce (16 bytes)                                  │
├────────────────────────────────────────────────────┤
│  HMAC-SHA256 (32 bytes)                            │
└────────────────────────────────────────────────────┘
```

### Replay Protection

```rust
// XOR-based filter for efficient replay detection
struct ReplayFilter {
    entries: DashMap<u64, u64>,  // conn_id -> last_nonce
    counter: AtomicU64,
}
```

---

## Traffic Obfuscation

### WebSocket Masking

All traffic appears as standard WSS:

```
GET /ws HTTP/1.1
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...
```

### Frame Obfuscation

```
┌─────────────┬───────┬─────────────┬─────────┬────────────┐
│ Length (2B) │ Flags │ UUID (16B)  │ Payload │ CRC32 (4B) │
└─────────────┴───────┴─────────────┴─────────┴────────────┘
       │                     │
       └──────── XOR Masked with Session Key ───────────┘
```

### Padding Strategies

| Strategy | Description |
|----------|-------------|
| Random | 1-256 bytes random padding |
| Fixed Block | Pad to 512-byte boundaries |
| Traffic Shaping | Mimic HTTP content-length patterns |

### Fake Traffic

- Periodic SSE keepalives
- Fake JSON API responses
- Random data bursts

---

## Emergency Mode

### Trigger Mechanisms

1. **crates.io Signal**: Monitor specific package version
2. **DNS Canary**: Resolve "canary.example.com"
3. **Manual Trigger**: API endpoint `/admin/emergency`

### Response Actions

When triggered:
1. Drop all active connections
2. Clear in-memory state
3. (Optional) Wipe local storage
4. Exit process

### Configuration

```toml
[security.emergency]
auto_trigger_dns = true
dns_domain = "signal.example.com"
check_interval = 300
crates_trigger = "apfsds-signal"
```

---

## Operational Security

### Logging Policy

| Level | What is logged |
|-------|----------------|
| ERROR | Crash reports, security events |
| WARN | Connection failures, auth rejections |
| INFO | Startup/shutdown, config changes |
| DEBUG | Protocol details (prod: disabled) |
| TRACE | Raw packets (never in prod) |

**NOT logged**: User content, IP addresses, connection metadata

### Key Management

1. Keys stored encrypted at rest
2. Master key from HSM or environment
3. Automatic rotation with grace period
4. Secure deletion of expired keys

### Hardening Checklist

- [ ] Run as non-root user
- [ ] Enable firewall (only expose 25347)
- [ ] Use tmpfs for hot data
- [ ] Disable core dumps
- [ ] Monitor audit logs
- [ ] Regular security updates
- [ ] Separate handler and exit networks

### Recommended Firewall Rules

```bash
# Handler node
iptables -A INPUT -p tcp --dport 25347 -j ACCEPT  # Main
iptables -A INPUT -p tcp --dport 25348 -s 10.0.0.0/8 -j ACCEPT  # Management (internal)
iptables -A INPUT -j DROP

# Exit node
iptables -A INPUT -p tcp --dport 25347 -s <handler_ips> -j ACCEPT
iptables -A OUTPUT -j ACCEPT  # Egress allowed
```

---

## Vulnerability Disclosure

Report security issues to: rand0mk4cas@gmail.com

We follow responsible disclosure practices and aim to respond within 48 hours.
