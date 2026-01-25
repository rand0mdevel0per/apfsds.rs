# APFSDS System Testing Plan

## Document Information

**Version:** 1.0
**Date:** 2026-01-25
**Status:** Draft

---

## 1. Testing Objectives

### Primary Goals
1. Verify all core functionality works as specified
2. Validate security mechanisms and encryption
3. Measure performance under various loads
4. Test obfuscation effectiveness
5. Ensure high availability and fault tolerance

### Success Criteria
- ✓ All unit tests pass (>95% coverage)
- ✓ Integration tests pass (>90% coverage)
- ✓ Performance meets specifications
- ✓ Security audit passes
- ✓ Obfuscation defeats common DPI tools

---

## 2. Testing Phases

### Phase 1: Unit Testing (Week 1-2)
- Individual module testing
- Code coverage analysis
- Static analysis

### Phase 2: Integration Testing (Week 3-4)
- Component interaction testing
- End-to-end flows
- API testing

### Phase 3: Performance Testing (Week 5)
- Load testing
- Stress testing
- Latency measurements

### Phase 4: Security Testing (Week 6)
- Penetration testing
- Cryptographic validation
- Vulnerability scanning

### Phase 5: Obfuscation Testing (Week 7)
- DPI evasion testing
- Traffic analysis resistance
- Pattern detection testing


---

## 3. Unit Testing Details

### 3.1 Protocol Module Tests

**Test Cases:**
- ✓ ProxyFrame serialization/deserialization
- ✓ PlainPacket validation (magic number check)
- ✓ Frame checksum verification
- ✓ UUID uniqueness validation

**Tools:** `cargo test`, `proptest` for property-based testing

### 3.2 Crypto Module Tests

**Test Cases:**
- ✓ AES-256-GCM encryption/decryption
- ✓ Ed25519 signing/verification
- ✓ X25519 key exchange
- ✓ HMAC token generation/validation
- ✓ Replay filter functionality

**Tools:** `cargo test`, test vectors from RFC standards

### 3.3 Storage Module Tests

**Test Cases:**
- ✓ MVCC transaction isolation
- ✓ Segment sealing and rotation
- ✓ B-link tree operations
- ✓ WAL recovery
- ✓ ClickHouse sync

**Tools:** `cargo test`, integration tests with real storage


---

## 4. Integration Testing Details

### 4.1 Client-Handler Flow

**Test Scenarios:**
1. Authentication handshake (retrieve token → connect)
2. SOCKS5 proxy connection
3. TUN device mode
4. DoH query over WSS
5. Connection pool management

**Expected Results:**
- Token retrieved within 200ms
- WebSocket upgrade successful
- Data flows bidirectionally
- DNS queries resolved correctly

### 4.2 Handler-Exit Flow

**Test Scenarios:**
1. PlainPacket forwarding
2. Exit node selection (weighted)
3. Health check mechanism
4. Failover to backup exit

**Expected Results:**
- Packets forwarded correctly
- Load balancing works
- Unhealthy exits excluded
- Automatic failover <5s


---

## 5. Performance Testing Details

### 5.1 Load Testing

**Tools:** `wrk`, `hey`, custom load generator

**Test Scenarios:**
- 100 concurrent connections
- 1,000 concurrent connections  
- 10,000 concurrent connections

**Metrics to Measure:**
- Requests per second (RPS)
- P50/P95/P99 latency
- Memory usage per connection
- CPU utilization

**Acceptance Criteria:**
- 10k connections: <10ms P95 latency
- Memory: <1GB for 10k connections
- CPU: <80% at peak load


---

## 6. Security Testing Details

### 6.1 Cryptographic Validation

**Test Cases:**
- Verify AES-256-GCM implementation against test vectors
- Validate Ed25519 signatures
- Test X25519 key exchange
- Replay attack prevention

**Tools:** `cargo test`, OpenSSL test vectors

### 6.2 Penetration Testing

**Attack Scenarios:**
- Man-in-the-middle attempts
- Replay attacks
- Token theft/reuse
- Timing attacks on authentication

**Expected Results:**
- All attacks should fail
- No information leakage
- Constant-time operations verified


---

## 7. Obfuscation Testing

### 7.1 DPI Evasion Testing

**Tools:** Wireshark, tcpdump, custom DPI simulators

**Test Methods:**
- Capture traffic and analyze patterns
- Compare with legitimate HTTPS/WSS traffic
- Test against known DPI signatures

**Success Criteria:**
- Traffic indistinguishable from browser WSS
- No obvious patterns in packet sizes
- Timing appears natural

### 7.2 Traffic Analysis Resistance

**Test Scenarios:**
- Statistical analysis of packet sizes
- Inter-packet timing analysis
- Connection duration patterns

**Expected Results:**
- Packet size distribution matches target profile
- Timing jitter prevents correlation
- No distinguishing characteristics

---

## 8. Testing Tools & Environment

**Required Infrastructure:**
- 3-node K8s cluster (handler)
- 2 exit nodes (different regions)
- Load testing clients (10+ machines)
- Network monitoring tools

**Estimated Resources:**
- Budget: $500-1000 for testing period
- Time: 7 weeks full testing cycle
- Team: 2-3 engineers

