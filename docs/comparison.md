# APFSDS vs Existing Solutions - Technical Comparison

## Executive Summary

This document provides a comprehensive technical comparison between APFSDS and existing proxy/VPN solutions, focusing on architecture, security, performance, and operational characteristics.

**Last Updated:** 2026-01-25
**Version:** 1.0

---

## Comparison Matrix

### Solutions Compared

1. **APFSDS** - This project
2. **Shadowsocks** - Lightweight SOCKS5 proxy
3. **V2Ray/Xray** - Modular proxy platform
4. **Trojan** - TLS-based proxy
5. **WireGuard** - Modern VPN protocol
6. **Outline** - Shadowsocks-based solution by Jigsaw

---

## 1. Architecture Comparison

### APFSDS
- **Type:** Split Handler/Exit architecture
- **Protocol:** WebSocket over TLS with custom framing
- **State Management:** Distributed (Raft consensus)
- **Storage:** MVCC with tmpfs + disk + ClickHouse
- **Scalability:** Horizontal (K8s-native)

### Shadowsocks
- **Type:** Simple client-server
- **Protocol:** SOCKS5 with custom encryption
- **State Management:** Stateless
- **Storage:** None (ephemeral)
- **Scalability:** Vertical (single instance)

### V2Ray/Xray
- **Type:** Modular proxy platform
- **Protocol:** VMess/VLESS/Trojan (multiple)
- **State Management:** Stateless
- **Storage:** None (ephemeral)
- **Scalability:** Vertical with load balancing

### Trojan
- **Type:** Client-server with TLS
- **Protocol:** TLS with fallback to HTTP
- **State Management:** Stateless
- **Storage:** None (ephemeral)
- **Scalability:** Vertical

### WireGuard
- **Type:** VPN tunnel
- **Protocol:** Custom UDP-based
- **State Management:** Kernel-level
- **Storage:** None (ephemeral)
- **Scalability:** Vertical

---

## 2. Security Features

| Feature | APFSDS | Shadowsocks | V2Ray | Trojan | WireGuard |
|---------|--------|-------------|-------|--------|-----------|
| **Encryption** | AES-256-GCM | AES/ChaCha20 | AES-128-GCM | TLS 1.3 | ChaCha20 |
| **Key Exchange** | X25519 ECDH | Pre-shared | UUID-based | TLS handshake | Curve25519 |
| **Forward Secrecy** | ✓ (rotation) | ✗ | ✗ | ✓ (TLS) | ✗ |
| **Authentication** | Multi-layer | Password | UUID | Password | Public key |
| **Replay Protection** | ✓ (XOR filter) | ✗ | ✗ | ✗ | ✓ (nonce) |
| **Emergency Mode** | ✓ (DNS/crates) | ✗ | ✗ | ✗ | ✗ |

**Security Analysis:**

- **APFSDS Strengths:** Multi-layer auth, forward secrecy with rotation, replay protection
- **Shadowsocks Weakness:** No forward secrecy, vulnerable to replay attacks
- **V2Ray Strength:** Multiple protocol support provides flexibility
- **Trojan Strength:** Leverages TLS 1.3 security
- **WireGuard Strength:** Cryptographically sound, formally verified

---

## 3. Traffic Obfuscation

| Feature | APFSDS | Shadowsocks | V2Ray | Trojan | WireGuard |
|---------|--------|-------------|-------|--------|-----------|
| **DPI Resistance** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| **Traffic Pattern** | WSS (browser-like) | Custom encrypted | Multiple | HTTPS | UDP (obvious) |
| **Padding** | ✓ (adaptive) | ✗ | ✓ (optional) | ✗ | ✗ |
| **Timing Jitter** | ✓ | ✗ | ✗ | ✗ | ✗ |
| **Fake Traffic** | ✓ (SSE/JSON) | ✗ | ✗ | ✗ | ✗ |
| **Fallback Content** | ✓ (Nginx) | ✗ | ✓ (configurable) | ✓ (HTTP) | ✗ |

**Obfuscation Analysis:**

- **APFSDS:** Most sophisticated obfuscation with WSS + padding + timing + fake traffic
- **Trojan:** Excellent TLS mimicry with HTTP fallback
- **V2Ray:** Flexible with plugins (WebSocket, mKCP, etc.)
- **Shadowsocks:** Basic encryption, increasingly detectable
- **WireGuard:** No obfuscation, easily identified as VPN traffic

## 4. Performance Characteristics

| Metric | APFSDS | Shadowsocks | V2Ray | Trojan | WireGuard |
|--------|--------|-------------|-------|--------|-----------|
| **Latency Overhead** | ~5-10ms | ~2-5ms | ~5-15ms | ~3-8ms | ~1-3ms |
| **Throughput** | High | Very High | Medium | High | Very High |
| **CPU Usage** | Medium | Low | Medium-High | Low | Very Low |
| **Memory Usage** | High (512MB+) | Low (50MB) | Medium (100MB) | Low (50MB) | Very Low (20MB) |
| **Concurrent Connections** | 10k+ | 10k+ | 5k+ | 10k+ | 1k+ |

**Performance Notes:**

- **APFSDS:** Higher overhead due to obfuscation, but scales horizontally
- **WireGuard:** Best raw performance, kernel-level implementation
- **Shadowsocks:** Lightweight, minimal overhead
- **V2Ray:** Flexible but resource-intensive with multiple protocols
- **Trojan:** Good balance of performance and obfuscation

---

## 5. Deployment Complexity

| Aspect | APFSDS | Shadowsocks | V2Ray | Trojan | WireGuard |
|--------|--------|-------------|-------|--------|-----------|
| **Setup Difficulty** | ⭐⭐⭐⭐ | ⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐ |
| **Configuration** | Complex (TOML) | Simple (JSON) | Complex (JSON) | Medium (JSON) | Simple (conf) |
| **Dependencies** | K8s/Docker | None | None | None | Kernel module |
| **Maintenance** | Medium | Low | Medium | Low | Low |
| **Monitoring** | ✓ (Prometheus) | Manual | Manual | Manual | Manual |

**Deployment Notes:**

- **APFSDS:** Most complex, requires K8s knowledge, but provides HA
- **Shadowsocks:** Simplest, single binary deployment
- **WireGuard:** Simple but requires kernel support
- **V2Ray:** Flexible but configuration can be complex
- **Trojan:** Moderate complexity, needs TLS certificates


---

## 6. Cost Analysis (Monthly)

### Small Scale (100 users)

| Solution | Infrastructure | Bandwidth | Total |
|----------|---------------|-----------|-------|
| **APFSDS** | $35-50 (split) | $20-30 | **$55-80** |
| **Shadowsocks** | $5-10 (VPS) | $10-20 | **$15-30** |
| **V2Ray** | $10-15 (VPS) | $10-20 | **$20-35** |
| **Trojan** | $10-15 (VPS) | $10-20 | **$20-35** |
| **WireGuard** | $5-10 (VPS) | $10-20 | **$15-30** |

### Medium Scale (1000 users)

| Solution | Infrastructure | Bandwidth | Total |
|----------|---------------|-----------|-------|
| **APFSDS** | $100-150 (K8s) | $200-300 | **$300-450** |
| **Shadowsocks** | $50-80 (multi-VPS) | $200-300 | **$250-380** |
| **V2Ray** | $80-120 (multi-VPS) | $200-300 | **$280-420** |

**Cost Notes:**

- **APFSDS:** Higher initial cost, but better scalability and HA
- **Shadowsocks/WireGuard:** Most cost-effective for small deployments
- **V2Ray:** Moderate cost with good flexibility


---

## 7. Use Case Recommendations

### When to Choose APFSDS

✅ **Best for:**
- Organizations needing high availability
- Scenarios requiring sophisticated obfuscation
- Deployments with >500 concurrent users
- Environments with strict DPI/traffic analysis
- Teams with K8s/DevOps expertise

❌ **Not ideal for:**
- Personal use (1-10 users)
- Resource-constrained environments
- Quick setup requirements
- Users without technical expertise

### When to Choose Alternatives

**Shadowsocks:** Personal use, simple setup, cost-sensitive
**V2Ray:** Flexibility needed, moderate scale
**Trojan:** Good obfuscation, simpler than APFSDS
**WireGuard:** VPN use case, maximum performance

---

## 8. Summary

**APFSDS Unique Advantages:**
1. ⭐ Best-in-class traffic obfuscation
2. ⭐ High availability with Raft consensus
3. ⭐ Emergency shutdown mechanism
4. ⭐ Built-in monitoring and analytics
5. ⭐ Horizontal scalability

**Trade-offs:**
- Higher complexity
- More resource usage
- Steeper learning curve
- Higher operational cost

**Verdict:** APFSDS is a sophisticated solution for organizations requiring maximum obfuscation and high availability, at the cost of increased complexity.

