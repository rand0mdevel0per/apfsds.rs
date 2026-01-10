# APFSDS Technical Specifications

> "The M829A2 APFSDS-T can penetrate over 600mm of reinforced steel. Truth pierces all veils; no wall stands eternal."

**Project Name:** APFSDS (Armor-Piercing Fin-Stabilized Discarding Sabot)  
**Version:** 0.1.0  
**Last Updated:** 2026-01-10

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Architecture Design](#architecture-design)
3. [Core Modules](#core-modules)
4. [Security Mechanisms](#security-mechanisms)
5. [Traffic Obfuscation](#traffic-obfuscation)
6. [Performance Optimization](#performance-optimization)
7. [Storage Engine](#storage-engine)
8. [Deployment](#deployment)
9. [Configuration](#configuration)
10. [Limitations & Risks](#limitations--risks)
11. [Development Roadmap](#development-roadmap)

---

## Project Overview

### Purpose

APFSDS is a next-generation network protocol designed for privacy-preserving communication. It provides:

- **Strong encryption** with multiple authentication layers
- **Traffic obfuscation** to blend with legitimate HTTPS/WebSocket traffic
- **High availability** through distributed architecture
- **Flexible deployment** supporting split Handler/Exit architecture

### Design Philosophy

1. **No obvious signatures** - Statistical characteristics hidden in noise
2. **Behavioral consistency** - Abnormal requests handled like legitimate services
3. **Graceful degradation** - Partial failures don't affect overall availability
4. **Observability** - Operational debugging without external exposure

### Key Features

- Multi-layer encryption (Ed25519 + AES-256 + TOTP)
- MVCC storage with Raft consensus
- DoH-over-WSS (DNS leak prevention + small packet obfuscation)
- Emergency shutdown mechanism (DNS TXT + steganography)
- Split architecture (Handler in low-cost region, Exit near target)
- Cloudflare integration (Tunnel between Handler ↔ Exit)

---

## Architecture Design

### Overall Topology

```
┌──────────────────────────────────────────────────────────────┐
│                         Client                                │
│  ┌─────────┐    ┌──────────────┐    ┌──────────────┐        │
│  │  Apps   │───→│ SOCKS5/TUN   │───→│ WSS Client   │        │
│  │(Browser)│    │(127.0.0.1:   │    │ - rkyv       │        │
│  │         │    │ 1080)        │    │ - zstd       │        │
│  └─────────┘    └──────────────┘    │ - SIMD XOR   │        │
│                                      └──────┬───────┘        │
│                                             │                 │
│  ┌──────────────────────────────────────────▼──────────┐    │
│  │ DoH Resolver (DNS over WSS)                         │    │
│  │ - Small packet obfuscation                          │    │
│  │ - DNS leak prevention                               │    │
│  └─────────────────────────────────────────────────────┘    │
└───────────────────────────┬──────────────────────────────────┘
                            │ TLS 1.3 + WSS
                            │ (Chrome handshake emulation)
                   ┌────────▼─────────┐
                   │   Cloudflare     │
                   │   - Anycast      │
                   │   - DDoS defense │
                   │   - Zero Trust   │
                   └────────┬─────────┘
                            │
        ┌───────────────────┴───────────────────┐
        │                                       │
┌───────▼──────┐                     ┌──────────▼────────┐
│ Nginx        │                     │ Direct/CF bypass  │
│ - TLS term   │                     └──────────┬────────┘
│ - Static     │                                │
│ - Proxy      │                                │
└───────┬──────┘                                │
        │ :25347                                │
        └───────────────────┬───────────────────┘
                            │
┌───────────────────────────▼───────────────────────────────┐
│              Handler/Daemon (K3s Cluster)                 │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Pod 1        Pod 2        Pod 3                    │  │
│  │  ┌─────┐     ┌─────┐     ┌─────┐                  │  │
│  │  │Daemon│────→│Daemon│────→│Daemon│                 │  │
│  │  └──┬──┘     └──┬──┘     └──┬──┘                  │  │
│  │     └──────────┬┴───────────┘                      │  │
│  │                │                                     │  │
│  │      ┌─────────▼──────────┐                        │  │
│  │      │ tmpfs (512Mi)      │                        │  │
│  │      │ - MVCC state       │                        │  │
│  │      │ - B-link index     │                        │  │
│  │      │ - Connection table │                        │  │
│  │      └─────────┬──────────┘                        │  │
│  │                │ overflow                           │  │
│  │      ┌─────────▼──────────┐                        │  │
│  │      │ Disk (SSD)         │                        │  │
│  │      │ - SSTable          │                        │  │
│  │      │ - Compaction       │                        │  │
│  │      └────────────────────┘                        │  │
│  └────────────────────────────────────────────────────┘  │
│                                                           │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Raft Consensus                                      │  │
│  │ - WAL sync                                          │  │
│  │ - Leader election                                   │  │
│  └────────────┬───────────────────────────────────────┘  │
│               │ async backup                             │
│  ┌────────────▼───────────────────────────────────────┐  │
│  │ ClickHouse                                          │  │
│  │ - Historical data                                   │  │
│  │ - Analytics queries                                 │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────┬─────────────────────────────────┘
                          │ Cloudflare Tunnel
                          │ (encrypted channel)
         ┌────────────────┼────────────────┐
         │                │                │
┌────────▼──────┐  ┌──────▼──────┐  ┌─────▼────────┐
│Exit (Tokyo)   │  │Exit (SG)    │  │Exit (Frankfurt)│
│- Forwarding   │  │- Forwarding │  │- Forwarding  │
│- Health check │  │- Health check│ │- Health check│
└───────┬───────┘  └──────┬──────┘  └──────┬───────┘
        │                 │                │
        └─────────────────┴────────────────┘
                          │
                    Target Website
```

### Data Flow

#### Outbound (Client → Target)

```
1. Application Data
   ↓
2. SOCKS5/TUN (client)
   ↓
3. ProxyFrame encapsulation
   - rkyv serialization
   - zstd compression (if >1KB)
   - SIMD XOR mask
   - Padding
   ↓
4. WebSocket Binary Frame
   - Client → Server: MUST mask
   - Proper framing (RFC 6455)
   ↓
5. TLS 1.3 encryption
   ↓
6. Cloudflare Network
   ↓
7. Nginx (SSL termination)
   ↓
8. Handler/Daemon
   - Authentication
   - Decryption
   - MVCC state update
   ↓
9. HTTP extraction → localhost:20396 (Nginx cache)
   - Cache hit → return cached
   - Cache miss → forward
   ↓
10. Cloudflare Tunnel (Handler → Exit)
    - Plaintext TCP (internal network)
    ↓
11. Exit Node
    - Simple forwarding
    - NAT translation
    ↓
12. Target Website
```

#### Inbound (Target → Client)

```
1. Target Response
   ↓
2. Exit Node
   - Receive response
   - Associate with conn_id
   ↓
3. Cloudflare Tunnel (Exit → Handler)
   ↓
4. Handler/Daemon
   - Lookup connection state (MVCC)
   - Encapsulate into ProxyFrame
   - Encrypt
   ↓
5. WebSocket Binary Frame
   ↓
6. TLS 1.3
   ↓
7. Cloudflare Network
   ↓
8. Client WSS
   - Decrypt
   - Decompress
   - Parse ProxyFrame
   ↓
9. SOCKS5/TUN
   ↓
10. Application
```

### Layer Breakdown

#### Layer 1: Transport Selection
- Primary: WebSocket over TLS (80% traffic)
- Fallback: SSH-like tunnel (15% traffic)
- Emergency: QUIC/DoH (5% traffic)

#### Layer 2: Traffic Obfuscation
- Main channel: rkyv frames in WS binary
- Noise channel: Simulated API responses (SSE/JSON)
- Decoy channel: Real file downloads/uploads

#### Layer 3: Connection Management
- Multi-connection pool (4-8 concurrent)
- Random reconnect (60-180s, configurable)
- Token rotation

#### Layer 4: Server-side
- Nginx frontend (decoy traffic)
- Daemon (specific endpoints)
- Docker containers (real SSH/Web services)

---

## Core Modules

### 1. Client Module

**Responsibilities:**
- SOCKS5/HTTP proxy server
- Optional TUN device management
- ProxyFrame encapsulation
- WSS connection management
- DoH resolver

**Implementation:**

```rust
// Client configuration
struct ClientConfig {
    mode: ClientMode,  // SOCKS5 | TUN | Transparent
    socks5_bind: SocketAddr,
    tun_device: Option<String>,
    tun_address: Option<IpAddr>,
    
    // Authentication
    credentials: Credentials,
    
    // Connection pool
    pool_size: usize,  // 4-8
    reconnect_interval: Range<u64>,  // [60, 180]s
    
    // DoH
    doh_enabled: bool,
    doh_servers: Vec<String>,
}

enum ClientMode {
    SOCKS5,
    TUN,
    Transparent,  // iptables-based
}

struct Credentials {
    client_sk: [u8; 32],       // Ed25519 secret key
    server_pk: [u8; 32],       // Server's public key
    hmac_secret: [u8; 32],
    token_endpoints: Vec<String>,
    conn_endpoints: Vec<String>,
}
```

**SOCKS5 Implementation:**

```rust
async fn socks5_server(config: &ClientConfig) -> Result<()> {
    let listener = TcpListener::bind(&config.socks5_bind).await?;
    
    loop {
        let (stream, addr) = listener.accept().await?;
        
        spawn(async move {
            // SOCKS5 handshake
            let target = socks5_handshake(&stream).await?;
            
            // Create ProxyFrame
            let conn_id = gen_conn_id();
            let frame = ProxyFrame {
                conn_id,
                rip: target.ip().octets(),
                payload: vec![],
                uuid: gen_uuid(),
                timestamp: now(),
                flags: FrameFlags::default(),
                ...
            };
            
            // Get connection from pool
            let wss_conn = CONN_POOL.get().await?;
            
            // Send initial frame
            send_frame(&wss_conn, frame).await?;
            
            // Bidirectional copy
            let (mut read, mut write) = stream.split();
            let (wss_read, wss_write) = wss_conn.split();
            
            tokio::select! {
                _ = copy_to_wss(&mut read, wss_write, conn_id) => {},
                _ = copy_from_wss(wss_read, &mut write, conn_id) => {},
            }
        });
    }
}
```

**TUN Implementation:**

```rust
async fn tun_mode(config: &ClientConfig) -> Result<()> {
    // Create TUN device
    let tun = tun::create(&config.tun_device.unwrap())?;
    tun.set_address(&config.tun_address.unwrap())?;
    
    // Add route
    add_route("0.0.0.0/0", tun.gateway())?;
    
    loop {
        // Read IP packet from TUN
        let packet = tun.read().await?;
        
        // Parse packet
        let ip_packet = parse_ip_packet(&packet)?;
        
        // Create ProxyFrame
        let frame = ProxyFrame {
            conn_id: gen_conn_id_from_tuple(&ip_packet.tuple()),
            rip: ip_packet.dst.octets(),
            payload: packet,
            ...
        };
        
        // Send via WSS
        send_frame_via_pool(frame).await?;
    }
}
```

**DoH Implementation:**

```rust
async fn resolve_dns_via_wss(domain: &str) -> Result<IpAddr> {
    // Build DoH query
    let query = build_doh_query(domain);
    
    // Encapsulate as control frame
    let frame = ProxyFrame {
        conn_id: 0,  // Special ID for DNS
        payload: query,
        flags: FrameFlags { is_control: true, .. },
        ...
    };
    
    // SIMD XOR mask
    let masked = simd_xor_mask(&frame.to_bytes());
    
    // Send via WSS
    let response = send_and_wait(masked).await?;
    
    // Parse response
    parse_doh_response(&response)
}
```

### 2. Transport Layer

**WebSocket Client:**

```rust
struct WSSClient {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    conn_id_map: DashMap<u64, Sender<Bytes>>,
}

impl WSSClient {
    async fn connect(endpoint: &str, token: &str) -> Result<Self> {
        // Build request with Chrome-like headers
        let request = Request::builder()
            .uri(endpoint)
            .header("User-Agent", CHROME_UA)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "gzip, deflate, br")
            .header("Sec-WebSocket-Version", "13")
            .header("Authorization", format!("Bearer {}", token))
            .body(())?;
        
        let (stream, _) = connect_async(request).await?;
        
        // Send initial fake frames (emulate negotiation)
        send_fake_frames(&stream).await?;
        
        Ok(Self {
            stream,
            conn_id_map: DashMap::new(),
        })
    }
    
    async fn send_frame(&self, frame: ProxyFrame) -> Result<()> {
        // Serialize
        let bytes = rkyv::to_bytes::<_, 1024>(&frame)?;
        
        // Compress if large
        let compressed = if bytes.len() > 1024 {
            zstd::encode_all(&bytes[..], 3)?
        } else {
            bytes.into_vec()
        };
        
        // XOR mask
        let masked = simd_xor_mask(&compressed);
        
        // Padding
        let padded = add_padding(masked, frame.payload.len());
        
        // Send as WebSocket Binary frame
        self.stream.send(Message::Binary(padded)).await?;
        
        Ok(())
    }
}
```

**Padding Strategy:**

```rust
fn calculate_padding(payload_len: usize) -> usize {
    // Target sizes mimicking real API responses
    const TARGETS: &[usize] = &[512, 1024, 2048, 4096, 8192];
    
    // Find next target
    let target = TARGETS.iter()
        .find(|&&t| t > payload_len)
        .unwrap_or(&8192);
    
    // Add jitter (±10%)
    let jitter = fastrand::usize(0..target / 10);
    
    target - payload_len + jitter
}

fn add_padding(mut data: Vec<u8>, original_len: usize) -> Vec<u8> {
    let padding_len = calculate_padding(original_len);
    
    // Random padding (not zeros)
    let padding: Vec<u8> = (0..padding_len)
        .map(|_| fastrand::u8(..))
        .collect();
    
    data.extend_from_slice(&padding);
    data
}
```

**SIMD XOR Mask:**

```rust
#[inline]
fn simd_xor_mask(data: &[u8]) -> Vec<u8> {
    let mask = get_dynamic_mask();
    let mut result = vec![0u8; data.len()];
    
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::*;
        
        let mut i = 0;
        let len = data.len();
        
        // Process 32 bytes at a time
        while i + 32 <= len {
            let data_vec = _mm256_loadu_si256(
                data[i..].as_ptr() as *const __m256i
            );
            let mask_vec = _mm256_loadu_si256(
                mask[i..].as_ptr() as *const __m256i
            );
            let xor_vec = _mm256_xor_si256(data_vec, mask_vec);
            _mm256_storeu_si256(
                result[i..].as_mut_ptr() as *mut __m256i,
                xor_vec
            );
            i += 32;
        }
        
        // Process remaining bytes
        for j in i..len {
            result[j] = data[j] ^ mask[j];
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (i, &byte) in data.iter().enumerate() {
            result[i] = byte ^ mask[i % mask.len()];
        }
    }
    
    result
}

fn get_dynamic_mask() -> Vec<u8> {
    // Generate mask based on current time + session key
    let seed = (now() / 1000) as u64 ^ SESSION_KEY;
    let mut rng = StdRng::seed_from_u64(seed);
    
    (0..8192).map(|_| rng.gen()).collect()
}
```

### 3. Handler/Daemon Module

**Main Structure:**

```rust
struct Daemon {
    config: DaemonConfig,
    storage: Arc<StorageEngine>,
    raft: Arc<RaftNode>,
    exit_pool: Arc<ExitNodePool>,
    auth: Arc<Authenticator>,
}

impl Daemon {
    async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>> {
        match req.uri().path() {
            "/retrieve-token" => self.handle_retrieve_token(req).await,
            "/connect" => self.handle_connect(req).await,
            _ => self.handle_decoy(req).await,
        }
    }
    
    async fn handle_retrieve_token(&self, req: Request<Body>) -> Result<Response<Body>> {
        let start = Instant::now();
        
        // Parse body
        let body = hyper::body::to_bytes(req.into_body()).await?;
        
        // Decrypt AES256-encrypted body
        let decrypted = decrypt_request_body(&body, &self.config.server_sk)?;
        
        // Parse authentication data
        let auth_data: AuthRequest = rkyv::from_bytes(&decrypted)?;
        
        // Verify
        let result = self.auth.verify(&auth_data);
        
        // Constant-time response (always 200ms)
        let elapsed = start.elapsed();
        if elapsed < Duration::from_millis(200) {
            sleep(Duration::from_millis(200) - elapsed).await;
        }
        
        match result {
            Ok(user_id) => {
                // Generate one-time token
                let token = self.auth.generate_token(user_id, &auth_data.nonce)?;
                
                // Check emergency mode
                let warning = if EMERGENCY_MODE.load(Ordering::Relaxed) {
                    Some(EmergencyWarning {
                        level: "emergency",
                        action: "stop",
                        trigger_after: now() + random_range(0, 3600),
                    })
                } else {
                    None
                };
                
                let response = TokenResponse {
                    token,
                    valid_until: now() + 60,
                    warning,
                };
                
                Ok(Response::new(Body::from(
                    encrypt_response(&response, &auth_data.client_pk)?
                )))
            },
            Err(_) => {
                // Log failed attempt
                log_failed_auth(req.remote_addr());
                
                Ok(Response::builder()
                    .status(401)
                    .body(Body::from("Unauthorized"))?)
            }
        }
    }
    
    async fn handle_connect(&self, req: Request<Body>) -> Result<Response<Body>> {
        // Extract token
        let token = extract_bearer_token(&req)?;
        
        // Verify token (one-time use)
        let user_id = self.auth.verify_and_consume_token(&token)?;
        
        // Upgrade to WebSocket
        let upgraded = hyper::upgrade::on(req).await?;
        let ws = WebSocketStream::from_raw_socket(
            upgraded,
            Role::Server,
            None,
        ).await;
        
        // Handle WebSocket connection
        spawn(self.handle_wss_connection(ws, user_id));
        
        Ok(Response::builder()
            .status(101)
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .body(Body::empty())?)
    }
    
    async fn handle_wss_connection(
        &self,
        mut ws: WebSocketStream<Upgraded>,
        user_id: u64,
    ) -> Result<()> {
        loop {
            tokio::select! {
                Some(msg) = ws.next() => {
                    let msg = msg?;
                    
                    match msg {
                        Message::Binary(data) => {
                            // Remove padding
                            let unpadded = remove_padding(&data);
                            
                            // XOR unmask
                            let unmasked = simd_xor_mask(&unpadded);
                            
                            // Decompress
                            let decompressed = if is_compressed(&unmasked) {
                                zstd::decode_all(&unmasked[..])?
                            } else {
                                unmasked
                            };
                            
                            // Deserialize ProxyFrame
                            let frame: &ArchivedProxyFrame = 
                                unsafe { rkyv::archived_root(&decompressed) };
                            
                            // Validate
                            self.validate_frame(frame)?;
                            
                            // Update MVCC state
                            self.storage.update_connection(frame).await?;
                            
                            // Extract HTTP request if present
                            if let Some(http_req) = extract_http_request(frame) {
                                self.cache_http_request(http_req).await?;
                            }
                            
                            // Forward to exit node
                            self.forward_to_exit(frame).await?;
                        },
                        Message::Text(text) => {
                            // Noise traffic (ignore or log)
                            trace!("Received noise: {}", text);
                        },
                        Message::Ping(data) => {
                            ws.send(Message::Pong(data)).await?;
                        },
                        Message::Close(_) => break,
                        _ => {}
                    }
                },
                _ = sleep(Duration::from_secs(300)) => {
                    // Timeout (5 minutes idle)
                    break;
                }
            }
        }
        
        Ok(())
    }
}
```

**Authentication:**

```rust
struct Authenticator {
    secret: [u8; 32],
    time_step: u64,  // 30s
    allowed_drift: i32,  // ±1 window
    nonce_cache: Arc<ReplayCache>,
}

impl Authenticator {
    fn verify(&self, auth: &AuthRequest) -> Result<u64> {
        // Check timestamp
        let now = now();
        if (now as i64 - auth.timestamp as i64).abs() > 30_000 {
            return Err("Timestamp out of range");
        }
        
        // Check nonce (replay protection)
        if !self.nonce_cache.check_and_insert(&auth.nonce, auth.timestamp) {
            return Err("Nonce reused");
        }
        
        // Verify HMAC
        let expected = self.compute_hmac(&auth.hmac_base, auth.timestamp);
        if !expected.ct_eq(&auth.hmac_signature).into() {
            return Err("Invalid HMAC");
        }
        
        // Extract user_id from hmac_base
        let user_id = extract_user_id(&auth.hmac_base)?;
        
        Ok(user_id)
    }
    
    fn generate_token(&self, user_id: u64, nonce: &[u8; 32]) -> Result<String> {
        let payload = TokenPayload {
            user_id,
            nonce: *nonce,
            issued_at: now(),
            valid_until: now() + 60_000,
        };
        
        let bytes = rkyv::to_bytes(&payload)?;
        
        // Sign with Ed25519
        let signature = self.sign(&bytes)?;
        
        // Combine and base64 encode
        let mut token = Vec::new();
        token.extend_from_slice(&bytes);
        token.extend_from_slice(&signature);
        
        Ok(base64::encode(&token))
    }
    
    fn verify_and_consume_token(&self, token: &str) -> Result<u64> {
        let decoded = base64::decode(token)?;
        
        // Split payload and signature
        let (payload_bytes, signature) = decoded.split_at(decoded.len() - 64);
        
        // Verify signature
        if !self.verify_signature(payload_bytes, signature)? {
            return Err("Invalid signature");
        }
        
        // Deserialize payload
        let payload: &ArchivedTokenPayload = 
            unsafe { rkyv::archived_root(payload_bytes) };
        
        // Check expiration
        if now() > payload.valid_until {
            return Err("Token expired");
        }
        
        // Check if already used (atomic CAS)
        let key = format!("token:{}", base64::encode(&payload.nonce));
        if !TOKEN_CACHE.insert(key, true, 60).await? {
            return Err("Token already used");
        }
        
        Ok(payload.user_id)
    }
}
```

**HTTP Caching:**

```rust
async fn cache_http_request(&self, req: HttpRequest) -> Result<()> {
    // Forward to localhost:20396 (Nginx cache instance)
    let client = reqwest::Client::new();
    
    let response = client
        .request(req.method, format!("http://127.0.0.1:20396{}", req.path))
        .headers(req.headers)
        .body(req.body)
        .send()
        .await?;
    
    // Nginx will cache this request
    // Return response back through WSS
    
    Ok(())
}
```

### 4. Storage Engine

**MVCC Implementation:**

```rust
struct StorageEngine {
    active_segment: RwLock<Segment>,
    sealed_segments: Vec<Segment>,
    index: Arc<BLinkTree>,
    raft: Arc<RaftNode>,
    clickhouse: ClickHouseClient,
    
    // Configuration
    segment_size_limit: usize,
    compaction_threshold: usize,
    cleanup_interval: u64,
}

#[derive(Archive, Serialize, Deserialize)]
struct ConnRecord {
    conn_id: u64,
    metadata: ConnMeta,
    created_at: u64,
    last_active: u64,
    access_count: u32,
    txid: u64,  // MVCC transaction ID
}

struct ConnMeta {
    client_addr: [u8; 16],
    nat_entry: (u16, u16),
    assigned_pod: u32,
    stream_states: Vec<StreamState>,
}

impl StorageEngine {
    async fn update_connection(&self, frame: &ArchivedProxyFrame) -> Result<()> {
        // Generate new txid
        let txid = self.raft.get_next_txid().await?;
        
        // Create record
        let record = ConnRecord {
            conn_id: frame.conn_id,
            metadata: extract_metadata(frame),
            last_active: now(),
            txid,
            ...
        };
        
        // Write to active segment
        let mut segment = self.active_segment.write().await;
        let offset = segment.append(record)?;
        
        // Update B-link tree (lock-free)
        let ptr = SegmentPtr { segment_id: segment.id, offset };
        self.index.insert(frame.conn_id, ptr);
        
        // Replicate via Raft
        self.raft.propose(RaftCommand::Insert {
            conn_id: frame.conn_id,
            txid,
            metadata: record.metadata,
        }).await?;
        
        // Seal segment if needed
        if segment.size() > self.segment_size_limit {
            drop(segment);  // Release write lock
            self.seal_and_rotate().await?;
        }
        
        Ok(())
    }
    
    fn get(&self, conn_id: u64) -> Option<ConnMeta> {
        // Search B-link tree (lock-free)
        let ptr = self.index.search(conn_id)?;
        
        // Read from segment
        if ptr.segment_id == self.active_segment.read().id {
            self.active_segment.read().read_at(ptr.offset)
        } else {
            self.sealed_segments.iter()
                .find(|s| s.id == ptr.segment_id)?
                .read_at(ptr.offset)
        }
    }
    
    async fn seal_and_rotate(&self) -> Result<()> {
        let mut active = self.active_segment.write().await;
        
        // Build bloom filter and index
        active.build_bloom_filter();
        active.build_index_block();
        active.is_sealed = true;
        
        let sealed = std::mem::replace(&mut *active, Segment::new());
        
        // Async persistence
        let clickhouse = self.clickhouse.clone();
        spawn(async move {
            sealed.flush_to_clickhouse(&clickhouse).await?;
        });
        
        // Trigger compaction check
        if self.sealed_segments.len() > self.compaction_threshold {
            self.compact().await?;
        }
        
        Ok(())
    }
    
    async fn compact(&self) -> Result<()> {
        // Select segments to merge
        let candidates = self.select_compact_candidates();
        
        // Merge (with MVCC cleaner)
        let merged = Segment::merge(candidates, |record| {
            // Keep only visible versions
            now() - record.last_active < 3600  // 1 hour TTL
        });
        
        // Atomic replacement
        let mut segments = self.sealed_segments.write().await;
        segments.retain(|s| !candidates.contains(&s.id));
        segments.push(merged);
        
        Ok(())
    }
}
```

**B-link Tree (Lock-free):**

```rust
struct BLinkNode {
    keys: Vec<u64>,
    values: Vec<SegmentPtr>,
    right_link: Option<NodeId>,
    high_key: u64,
    level: u8,
}

struct BLinkTree {
    root: AtomicU64,
    nodes: DashMap<NodeId, BLinkNode>,
}

impl BLinkTree {
    fn search(&self, key: u64) -> Option<SegmentPtr> {
        let mut node_id = self.root.load(Ordering::Acquire);
        
        loop {
            let node = self.nodes.get(&node_id)?;
            
            // Check if key exceeds node range
            if key > node.high_key {
                // Follow right_link (B-link's key feature)
                if let Some(right_id) = node.right_link {
                    node_id = right_id;
                    continue;
                }
            }
            
            // Search within node
            if node.level == 0 {
                // Leaf node
                return node.values.get(
                    node.keys.binary_search(&key).ok()?
                ).cloned();
            } else {
                // Internal node
                let pos = node.keys.binary_search(&key)
                    .unwrap_or_else(|p| p.saturating_sub(1));
                node_id = node.values[pos].segment_id;
            }
        }
    }
    
    fn insert(&self, key: u64, value: SegmentPtr) {
        // Lock-free insertion with SMO (Structure Modification Operation)
        // Implementation omitted for brevity (standard B-link algorithm)
    }
}
```

**Raft Integration:**

```rust
struct RaftNode {
    raft: Arc<RawNode<MemStorage>>,
    apply_ch: Receiver<Vec<Entry>>,
    storage_engine: Arc<StorageEngine>,
}

impl RaftNode {
    async fn propose(&self, cmd: RaftCommand) -> Result<()> {
        let data = rkyv::to_bytes(&cmd)?;
        self.raft.propose(vec![], data.into())?;
        Ok(())
    }
    
    async fn apply_loop(&self) -> Result<()> {
        loop {
            let entries = self.apply_ch.recv().await?;
            
            for entry in entries {
                if entry.data.is_empty() {
                    continue;  // Empty entry (leadership change)
                }
                
                let cmd: &ArchivedRaftCommand = 
                    unsafe { rkyv::archived_root(&entry.data) };
                
                match cmd {
                    RaftCommand::Insert { conn_id, txid, metadata } => {
                        // Apply to local storage
                        self.storage_engine.apply_insert(
                            *conn_id, *txid, metadata
                        ).await?;
                    },
                    RaftCommand::Delete { conn_id } => {
                        self.storage_engine.apply_delete(*conn_id).await?;
                    },
                }
            }
        }
    }
    
    async fn get_next_txid(&self) -> Result<u64> {
        // Atomic increment
        Ok(GLOBAL_TXID.fetch_add(1, Ordering::SeqCst))
    }
}
```

**ClickHouse Backup:**

```rust
async fn sync_to_clickhouse(&self) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        // Get WAL entries since last sync
        let entries = self.raft.get_wal_entries(self.last_synced_index)?;
        
        // Batch insert to ClickHouse
        let mut batch = Vec::new();
        for entry in entries {
            let cmd: &ArchivedRaftCommand = 
                unsafe { rkyv::archived_root(&entry.data) };
            
            if let RaftCommand::Insert { conn_id, metadata, .. } = cmd {
                batch.push(ClickHouseRow {
                    conn_id: *conn_id,
                    client_addr: metadata.client_addr,
                    timestamp: now(),
                    ...
                });
            }
        }
        
        if !batch.is_empty() {
            self.clickhouse.insert("connections", batch).await?;
            self.last_synced_index = entries.last().unwrap().index;
        }
    }
}
```

### 5. Exit Node Module

**Simple Forwarder:**

```rust
struct ExitNode {
    listener: TcpListener,
    health: Arc<AtomicBool>,
}

impl ExitNode {
    async fn run(&self) -> Result<()> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            
            spawn(async move {
                self.handle_connection(stream).await
            });
        }
    }
    
    async fn handle_connection(&self, mut stream: TcpStream) -> Result<()> {
        // Read PlainPacket
        let packet = read_plain_packet(&mut stream).await?;
        
        // Validate magic
        if packet.magic != 0xDEADBEEF {
            return Err("Invalid magic");
        }
        
        // Connect to target
        let mut target = TcpStream::connect(packet.target).await?;
        
        // Write initial payload
        target.write_all(&packet.payload).await?;
        
        // Bidirectional copy
        tokio::io::copy_bidirectional(&mut stream, &mut target).await?;
        
        Ok(())
    }
}

#[repr(C, packed)]
struct PlainPacket {
    magic: u32,          // 0xDEADBEEF
    conn_id: u64,
    target: SocketAddr,
    payload_len: u32,
    payload: Bytes,
}

async fn read_plain_packet(stream: &mut TcpStream) -> Result<PlainPacket> {
    // Read header
    let mut header = [0u8; 28];  // 4 + 8 + 16
    stream.read_exact(&mut header).await?;
    
    let magic = u32::from_le_bytes(header[0..4].try_into()?);
    let conn_id = u64::from_le_bytes(header[4..12].try_into()?);
    // ... parse target and payload_len
    
    // Read payload
    let mut payload = vec![0u8; payload_len as usize];
    stream.read_exact(&mut payload).await?;
    
    Ok(PlainPacket {
        magic,
        conn_id,
        target,
        payload_len,
        payload: payload.into(),
    })
}
```

**Health Check:**

```rust
async fn health_check_loop(exit_nodes: Arc<ExitNodePool>) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    
    loop {
        interval.tick().await;
        
        for node in exit_nodes.nodes.iter() {
            let start = Instant::now();
            
            // Send ping packet
            let result = ping_exit_node(node).await;
            
            let latency = start.elapsed();
            
            match result {
                Ok(_) => {
                    node.mark_healthy();
                    node.update_latency(latency);
                    METRICS.exit_node_latency
                        .with_label_values(&[&node.name])
                        .observe(latency.as_secs_f64());
                },
                Err(e) => {
                    warn!("Exit node {} unhealthy: {}", node.name, e);
                    node.mark_unhealthy();
                    
                    // Trigger alert
                    if node.consecutive_failures() > 3 {
                        alert!("Exit node {} is down", node.name);
                    }
                }
            }
        }
    }
}

async fn ping_exit_node(node: &ExitNode) -> Result<()> {
    let mut stream = TcpStream::connect(&node.endpoint).await?;
    
    // Send ping packet
    let ping = PlainPacket {
        magic: 0xDEADBEEF,
        conn_id: 0,
        target: "0.0.0.0:0".parse()?,
        payload_len: 0,
        payload: Bytes::new(),
    };
    
    stream.write_all(&ping.to_bytes()).await?;
    
    // Expect close
    let mut buf = [0u8; 1];
    let n = stream.read(&mut buf).await?;
    
    if n == 0 {
        Ok(())  // Connection closed = healthy
    } else {
        Err("Unexpected response")
    }
}
```

---

## Security Mechanisms

### Authentication Flow

```
1. Client → POST /retrieve-token
   Body (AES256-encrypted):
   {
     hmac_base: "user_id:timestamp:random",
     client_sk: [32 bytes],
     client_rsa_pk: [32 bytes],
     nonce: [32 bytes],  // Replay protection
     timestamp: 1234567890
   }
   
   Encryption:
   - Generate random AES256 key
   - Encrypt body with AES256-GCM
   - Encrypt AES key with server's Ed25519 public key
   - zstd compress
   
2. Server validates:
   - Decrypt AES key with server's private key
   - Decrypt body
   - Check nonce not reused (Redis/in-memory cache)
   - Verify timestamp within ±30s
   - Compute HMAC and compare (constant-time)
   - Always take 200ms to respond (prevent timing attacks)
   
3. Server → Response (same encryption):
   {
     token: "base64(payload + signature)",
     valid_until: timestamp + 60,
     warning: {  // Optional
       level: "emergency",
       action: "stop",
       trigger_after: 1234567890
     }
   }
   
4. Client → POST /connect
   Authorization: Bearer <one-time-token>
   
5. Server validates token:
   - Verify Ed25519 signature
   - Check expiration (60s)
   - Check not already used (atomic CAS in Redis)
   - If any check fails: 403 (force re-retrieve)
   
6. Server → 101 Switching Protocols
   Upgrade: websocket
   Connection: Upgrade
   
7. WebSocket connection established
```

### Key Rotation

**Scheduled Rotation:**

```rust
async fn key_rotation_task() {
    let mut interval = tokio::time::interval(
        Duration::from_secs(CONFIG.key_rotation_interval)
    );
    
    loop {
        interval.tick().await;
        
        // Generate new key pair
        let new_sk = Ed25519KeyPair::generate();
        
        // Enable grace period (10 minutes)
        GRACE_PERIOD.store(true, Ordering::Relaxed);
        NEW_KEY.store(Some(new_sk.clone()));
        
        // Notify clients via WSS
        broadcast_key_rotation(KeyRotationMessage {
            new_pk: new_sk.public_key(),
            valid_from: now() + 60,
            valid_until: now() + 600,
        }).await;
        
        // Wait for grace period
        sleep(Duration::from_secs(600)).await;
        
        // Switch to new key
        OLD_KEY.store(CURRENT_KEY.load());
        CURRENT_KEY.store(new_sk);
        GRACE_PERIOD.store(false, Ordering::Relaxed);
        
        info!("Key rotation complete");
    }
}
```

**Forced Rotation (Compromised):**

```rust
async fn force_key_rotation(reason: RotationReason) {
    match reason {
        RotationReason::Compromised | RotationReason::UserReset => {
            // Immediate rotation
            let new_sk = Ed25519KeyPair::generate();
            
            // NO grace period
            CURRENT_KEY.store(new_sk);
            OLD_KEY.store(None);
            
            // Kick all connections
            kick_all_connections().await;
            
            // Invalidate all tokens
            TOKEN_CACHE.clear().await;
            
            // Notify via emergency channel
            send_emergency_notification(EmergencyType::KeyCompromised).await;
            
            warn!("Forced key rotation: {:?}", reason);
        },
        _ => {}
    }
}
```

### Replay Protection

**Nonce Cache:**

```rust
struct ReplayCache {
    seen: DashMap<[u8; 32], u64>,  // nonce -> expire_time
    cleanup_interval: u64,
}

impl ReplayCache {
    fn check_and_insert(&self, nonce: &[u8; 32], timestamp: u64) -> bool {
        let expire = timestamp + 120_000;  // 2 minutes
        
        // Atomic insert
        self.seen.insert(*nonce, expire).is_none()
    }
    
    async fn cleanup_loop(&self) {
        let mut interval = tokio::time::interval(
            Duration::from_secs(self.cleanup_interval)
        );
        
        loop {
            interval.tick().await;
            
            let now = now();
            self.seen.retain(|_, &mut expire| expire > now);
        }
    }
}
```

**UUID Validation:**

```rust
fn validate_frame(frame: &ArchivedProxyFrame) -> Result<()> {
    // Check UUID not seen before
    if !FRAME_UUID_CACHE.check_and_insert(&frame.uuid, frame.timestamp) {
        return Err("Duplicate frame UUID");
    }
    
    // Verify checksum
    let computed = crc32fast::hash(&frame.payload);
    if computed != frame.checksum {
        return Err("Checksum mismatch");
    }
    
    // Verify timestamp
    let now = now();
    if (now as i64 - frame.timestamp as i64).abs() > 30_000 {
        return Err("Timestamp out of range");
    }
    
    Ok(())
}
```

### Time Synchronization

**Client-side:**

```rust
struct TimeSync {
    offset: AtomicI64,  // milliseconds
}

impl TimeSync {
    async fn sync_with_server(&self) -> Result<()> {
        let t1 = now();
        
        // Send sync request
        let response = request_time_sync().await?;
        
        let t4 = now();
        let t2 = response.server_recv_time;
        let t3 = response.server_send_time;
        
        // Calculate offset (NTP algorithm)
        let offset = ((t2 - t1) + (t3 - t4)) / 2;
        
        self.offset.store(offset, Ordering::Relaxed);
        
        info!("Time offset: {}ms", offset);
        
        Ok(())
    }
    
    fn adjusted_now(&self) -> u64 {
        let raw = now();
        let offset = self.offset.load(Ordering::Relaxed);
        (raw as i64 + offset) as u64
    }
}
```

---

## Traffic Obfuscation

### WebSocket Handshake Emulation

**Chrome 120 Handshake:**

```rust
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

async fn build_wss_request(endpoint: &str, token: &str) -> Request<()> {
    Request::builder()
        .uri(endpoint)
        .method("GET")
        .header("Host", extract_host(endpoint))
        .header("User-Agent", CHROME_UA)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", gen_ws_key())
        .header("Sec-WebSocket-Extensions", 
            "permessage-deflate; client_max_window_bits")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Authorization", format!("Bearer {}", token))
        // Chrome's typical header order is important!
        .body(())
        .unwrap()
}
```

**Initial Frames:**

```rust
async fn send_initial_frames(ws: &mut WebSocketStream) -> Result<()> {
    // Frame 1: Small text frame (like handshake ack)
    ws.send(Message::Text("ping".into())).await?;
    
    sleep(Duration::from_millis(random_range(10, 50))).await;
    
    // Frame 2-3: Empty padding frames
    for _ in 0..2 {
        let padding: Vec<u8> = (0..random_range(100, 500))
            .map(|_| fastrand::u8(..))
            .collect();
        ws.send(Message::Binary(padding)).await?;
        
        sleep(Duration::from_millis(random_range(5, 20))).await;
    }
    
    // Frame 4+: Start real data
    Ok(())
}
```

### Noise Traffic

**SSE Keepalive:**

```rust
async fn noise_traffic_task(ws: Arc<Mutex<WebSocketStream>>) {
    let mut interval = tokio::time::interval(
        Duration::from_secs(random_range(10, 30))
    );
    
    loop {
        interval.tick().await;
        
        // Send SSE-like text frame
        let sse = format!(
            "data: {{\"type\":\"ping\",\"timestamp\":{}}}\n\n",
            now()
        );
        
        let mut ws = ws.lock().await;
        ws.send(Message::Text(sse)).await.ok();
    }
}
```

**Fake API Responses:**

```rust
async fn send_fake_json_response(ws: &mut WebSocketStream) -> Result<()> {
    let fake_response = json!({
        "id": gen_uuid_string(),
        "object": "chat.completion.chunk",
        "created": now() / 1000,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": null
        }]
    });
    
    ws.send(Message::Text(fake_response.to_string())).await?;
    Ok(())
}
```

### Packet Size Distribution

**Target Distribution:**

```rust
// Mimics real API traffic
const SIZE_DISTRIBUTION: &[(usize, f32)] = &[
    (512, 0.40),      // 40% small packets
    (1024, 0.20),     // 20%
    (2048, 0.15),     // 15%
    (4096, 0.15),     // 15%
    (8192, 0.07),     // 7%
    (16384, 0.03),    // 3% large packets
];

fn select_target_size() -> usize {
    let r: f32 = fastrand::f32();
    let mut cumulative = 0.0;
    
    for &(size, prob) in SIZE_DISTRIBUTION {
        cumulative += prob;
        if r < cumulative {
            return size;
        }
    }
    
    8192  // fallback
}
```

### Timing Jitter

**Random Delays:**

```rust
async fn send_with_jitter(ws: &mut WebSocketStream, msg: Message) -> Result<()> {
    // Add random delay (0-50ms)
    sleep(Duration::from_millis(fastrand::u64(0..50))).await;
    
    ws.send(msg).await?;
    
    Ok(())
}

async fn batch_send_with_timing(
    ws: &mut WebSocketStream,
    frames: Vec<ProxyFrame>,
) -> Result<()> {
    for frame in frames {
        let msg = encode_frame(frame)?;
        send_with_jitter(ws, msg).await?;
        
        // Inter-frame delay (mimics processing time)
        sleep(Duration::from_micros(fastrand::u64(100..5000))).await;
    }
    
    Ok(())
}
```

---

## Performance Optimization

### Zero-Copy Deserialization

```rust
// rkyv enables zero-copy
fn parse_frame_zerocopy(bytes: &[u8]) -> Result<&ArchivedProxyFrame> {
    // No allocation, direct access to archived data
    unsafe { Ok(rkyv::archived_root::<ProxyFrame>(bytes)) }
}

// Access fields without deserialization
fn extract_conn_id(bytes: &[u8]) -> u64 {
    let frame = unsafe { rkyv::archived_root::<ProxyFrame>(bytes) };
    frame.conn_id  // Direct field access, no copying
}
```

### Buffer Pooling

```rust
lazy_static! {
    static ref BUFFER_POOL: Pool<Vec<u8>> = Pool::new(
        || Vec::with_capacity(8192),
        |buf| {
            buf.clear();
            buf.capacity() >= 8192
        }
    );
}

async fn process_frame() -> Result<()> {
    // Get buffer from pool
    let mut buf = BUFFER_POOL.get();
    
    // Use buffer
    read_frame_into(&mut buf).await?;
    
    // Buffer automatically returned to pool on drop
    Ok(())
}
```

### Batch Processing

```rust
async fn batch_forward_to_exit(
    frames: Vec<ArchivedProxyFrame>,
) -> Result<()> {
    // Group by exit node
    let mut groups: HashMap<String, Vec<_>> = HashMap::new();
    for frame in frames {
        let exit = select_exit_node(&frame)?;
        groups.entry(exit.name.clone())
            .or_insert_with(Vec::new)
            .push(frame);
    }
    
    // Send batches in parallel
    let tasks: Vec<_> = groups.into_iter()
        .map(|(exit_name, batch)| {
            spawn(async move {
                send_batch_to_exit(&exit_name, batch).await
            })
        })
        .collect();
    
    // Wait for all
    futures::future::join_all(tasks).await;
    
    Ok(())
}
```

### Connection Pooling

```rust
struct ConnectionPool {
    conns: Vec<Arc<Mutex<WebSocketStream>>>,
    robin: AtomicUsize,
}

impl ConnectionPool {
    async fn get(&self) -> Arc<Mutex<WebSocketStream>> {
        // Round-robin
        let idx = self.robin.fetch_add(1, Ordering::Relaxed) % self.conns.len();
        self.conns[idx].clone()
    }
    
    async fn send_frame(&self, frame: ProxyFrame) -> Result<()> {
        // Get connection
        let conn = self.get().await;
        
        // Send
        let mut ws = conn.lock().await;
        ws.send(encode_frame(frame)?).await?;
        
        Ok(())
    }
}
```

### Metrics

```rust
use prometheus::{Counter, Histogram, IntGauge};

lazy_static! {
    static ref METRICS: Metrics = Metrics::new();
}

struct Metrics {
    // Counters
    pub frames_sent: Counter,
    pub frames_received: Counter,
    pub auth_failures: Counter,
    
    // Histograms
    pub frame_size: Histogram,
    pub latency: Histogram,
    pub serialize_duration: Histogram,
    
    // Gauges
    pub active_connections: IntGauge,
    pub tmpfs_usage: IntGauge,
}

impl Metrics {
    fn new() -> Self {
        Self {
            frames_sent: register_counter!(
                "apfsds_frames_sent_total",
                "Total frames sent"
            ).unwrap(),
            // ... other metrics
        }
    }
    
    fn observe_frame(&self, frame: &ProxyFrame, duration: Duration) {
        self.frames_sent.inc();
        self.frame_size.observe(frame.payload.len() as f64);
        self.serialize_duration.observe(duration.as_secs_f64());
    }
}
```

---

## Deployment

### Helm Chart Structure

```
helm-chart/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── ingress.yaml
│   ├── pvc.yaml
│   ├── hpa.yaml
│   └── servicemonitor.yaml
└── values-production.yaml
```

### Values Configuration

```yaml
# values.yaml
replicaCount: 3

image:
  repository: ghcr.io/yourrepo/apfsds
  tag: "0.1.0"
  pullPolicy: IfNotPresent

deployment:
  mode: split  # split | all-in-one
  
server:
  domain: proxy.example.com
  
  # Handler configuration
  handler:
    location: nanjing
    bind: "0.0.0.0:25347"
    
  # Exit nodes
  exitNodes:
    - name: tokyo
      endpoint: "10.0.1.100:25347"
      weight: 1.0
      location: "Tokyo, Japan"
    - name: singapore
      endpoint: "10.0.1.101:25347"
      weight: 0.5
      location: "Singapore"

storage:
  tmpfs:
    enabled: true
    size: 512Mi
  
  disk:
    enabled: true
    size: 10Gi
    storageClass: fast-ssd
  
  clickhouse:
    enabled: true
    dsn: "tcp://clickhouse.clickhouse.svc.cluster.local:9000"
    database: apfsds
    retention: 7d

security:
  # Generated during install
  serverSecretKey: ""
  hmacSecret: ""
  
  tokenTTL: 3600
  keyRotationInterval: 604800  # 7 days
  gracePeriod: 600  # 10 minutes
  
  # Emergency
  emergency:
    autoTriggerOnDNS: true
    dnsCheckDomain: "_emergency.example.com"
    checkInterval: 300  # 5 minutes
    triggerDelayRange: [0, 3600]  # 0-1 hour

nginx:
  enabled: true
  
  cache:
    enabled: true
    size: 1Gi
    ttl: 3600
  
  static:
    enabled: true
    path: /var/www/html
  
  proxy:
    enabled: true
    target: https://normal-blog.example.com

cloudflare:
  tunnel:
    enabled: false
    token: ""

monitoring:
  prometheus:
    enabled: true
    port: 9090
    path: /metrics
  
  datadog:
    enabled: false
    apiKey: ""

resources:
  limits:
    cpu: 2000m
    memory: 4Gi
  requests:
    cpu: 500m
    memory: 1Gi

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

### Deployment YAML

```yaml
# templates/deployment.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: {{ include "apfsds.fullname" . }}
  labels:
    {{- include "apfsds.labels" . | nindent 4 }}
spec:
  serviceName: {{ include "apfsds.fullname" . }}-headless
  replicas: {{ .Values.replicaCount }}
  selector:
    matchLabels:
      {{- include "apfsds.selectorLabels" . | nindent 6 }}
  template:
    metadata:
      labels:
        {{- include "apfsds.selectorLabels" . | nindent 8 }}
    spec:
      containers:
      - name: daemon
        image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"
        ports:
        - containerPort: 25347
          name: handler
        - containerPort: 9090
          name: metrics
        env:
        - name: APFSDS_MODE
          value: {{ .Values.deployment.mode }}
        - name: APFSDS_SERVER_SK
          valueFrom:
            secretKeyRef:
              name: {{ include "apfsds.fullname" . }}-secrets
              key: server-secret-key
        volumeMounts:
        - name: tmpfs
          mountPath: /dev/shm/apfsds
        - name: data
          mountPath: /var/lib/apfsds
        - name: config
          mountPath: /etc/apfsds
        resources:
          {{- toYaml .Values.resources | nindent 10 }}
        livenessProbe:
          httpGet:
            path: /health
            port: 25347
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 25347
          initialDelaySeconds: 5
          periodSeconds: 5
      
      volumes:
      - name: tmpfs
        emptyDir:
          medium: Memory
          sizeLimit: {{ .Values.storage.tmpfs.size }}
      - name: config
        configMap:
          name: {{ include "apfsds.fullname" . }}-config
  
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      storageClassName: {{ .Values.storage.disk.storageClass }}
      resources:
        requests:
          storage: {{ .Values.storage.disk.size }}
```

### One-click Install Script

```bash
#!/bin/bash
# install.sh

set -e

VERSION="0.1.0"
K3S_VERSION="v1.28.5+k3s1"

echo "========================================"
echo "APFSDS Installer v${VERSION}"
echo "========================================"

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
    else
        echo "Unsupported OS"
        exit 1
    fi
}

# Install K3s
install_k3s() {
    if command -v k3s &> /dev/null; then
        echo "K3s already installed"
        return
    fi
    
    echo "Installing K3s..."
    curl -sfL https://get.k3s.io | \
        INSTALL_K3S_VERSION=${K3S_VERSION} \
        INSTALL_K3S_EXEC="--disable=traefik" \
        sh -
    
    mkdir -p ~/.kube
    sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config
    sudo chown $(id -u):$(id -g) ~/.kube/config
    export KUBECONFIG=~/.kube/config
}

# Install Helm
install_helm() {
    if command -v helm &> /dev/null; then
        echo "Helm already installed"
        return
    fi
    
    echo "Installing Helm..."
    curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
}

# Configure
configure() {
    echo ""
    echo "Configuration Wizard"
    echo "===================="
    
    read -p "Domain: " DOMAIN
    read -p "Deployment mode (split/all-in-one): " MODE
    MODE=${MODE:-all-in-one}
    
    # Generate secrets
    SERVER_SK=$(openssl rand -hex 32)
    HMAC_SECRET=$(openssl rand -hex 32)
    
    cat > values-custom.yaml <<EOF
server:
  domain: ${DOMAIN}

deployment:
  mode: ${MODE}

security:
  serverSecretKey: ${SERVER_SK}
  hmacSecret: ${HMAC_SECRET}

# ... rest of config
EOF
    
    echo "Config saved to values-custom.yaml"
}

# Deploy
deploy() {
    echo "Deploying APFSDS..."
    
    kubectl create namespace apfsds || true
    
    helm upgrade --install apfsds ./helm-chart \
        --namespace apfsds \
        --values values-custom.yaml \
        --wait \
        --timeout 10m
    
    echo ""
    echo "Deployment complete! 🎉"
    echo "Endpoint: https://${DOMAIN}"
}

main() {
    detect_os
    install_k3s
    install_helm
    configure
    deploy
}

main
```

---

## Configuration

### Client Configuration

```toml
# client.toml

[client]
mode = "socks5"  # socks5 | tun | transparent

[client.socks5]
bind = "127.0.0.1:1080"
auth = false

[client.tun]
device = "tun-apfsds"
address = "10.0.0.2/24"
mtu = 1500
ipv6 = false

[client.doh]
enabled = true
servers = ["https://cloudflare-dns.com/dns-query"]
via_wss = true  # Send DoH queries through WSS

[connection]
pool_size = 6
reconnect_interval = [60, 180]  # seconds
timeout = 30

[security]
# Obtained from subscription
credentials_path = "/etc/apfsds/credentials.json"

[obfuscation]
noise_ratio = 0.15
fake_json_enabled = true
sse_keepalive = true

[logging]
level = "info"
file = "/var/log/apfsds/client.log"
```

### Server Configuration

```toml
# server.toml

[server]
mode = "handler"  # handler | exit | all-in-one
bind = "0.0.0.0:25347"

[server.handler]
location = "nanjing"

[[server.exit_nodes]]
name = "tokyo"
endpoint = "10.0.1.100:25347"
weight = 1.0

[[server.exit_nodes]]
name = "singapore"
endpoint = "10.0.1.101:25347"
weight = 0.5

[storage]
tmpfs_path = "/dev/shm/apfsds"
tmpfs_size = 536870912  # 512MB
disk_path = "/var/lib/apfsds"

[storage.mvcc]
segment_size_limit = 10485760  # 10MB
compaction_threshold = 10
cleanup_interval = 300  # 5 minutes

[storage.clickhouse]
dsn = "tcp://localhost:9000"
database = "apfsds"
table = "connections"
batch_size = 1000
flush_interval = 60  # seconds

[raft]
node_id = 1
peers = ["10.0.0.1:26347", "10.0.0.2:26347", "10.0.0.3:26347"]

[security]
server_secret_key = "hex..."
hmac_secret = "hex..."
token_ttl = 3600
key_rotation_interval = 604800
grace_period = 600

[security.emergency]
auto_trigger_dns = true
dns_domain = "_emergency.example.com"
check_interval = 300
trigger_delay_range = [0, 3600]

[nginx]
cache_bind = "127.0.0.1:20396"
cache_size = "1g"
cache_ttl = 3600

[monitoring]
prometheus_bind = "0.0.0.0:9090"
datadog_enabled = false

[logging]
level = "info"
file = "/var/log/apfsds/server.log"
```

---

## Limitations & Risks

### Technical Limitations

1. **Performance overhead**: Encryption + compression + padding adds ~5-10ms latency
2. **Bandwidth overhead**: Padding increases traffic by ~15-30%
3. **Memory usage**: tmpfs requires 512MB-1GB per node
4. **Connection limit**: ~10,000 concurrent connections per pod (OS limit)

### Operational Risks

1. **Cloudflare dependency**: If CF blocks/limits, need fallback
2. **Exit node availability**: Single point of failure (mitigated by multiple exits)
3. **ClickHouse sync lag**: Possible data loss on node crash (Raft provides recovery)
4. **Key rotation disruption**: Brief service interruption during forced rotation

### Detection Risks

1. **Traffic volume correlation**: Large traffic spikes may attract attention
2. **Geographic clustering**: Many clients in same region connecting to same server
3. **Usage patterns**: Unusual traffic patterns (24/7 high bandwidth)
4. **Active probing**: GFW may actively probe suspected servers

### Mitigation Strategies

1. **Multiple domains**: Rotate between several domains
2. **Cloudflare CDN**: Hide real IP
3. **Traffic shaping**: Limit max bandwidth per user
4. **Usage monitoring**: Alert on suspicious patterns
5. **Emergency shutdown**: Rapid response to detection

### Cost Considerations

**All-in-one mode:**
- 1x 4-core 8GB VPS: $20-40/month
- Total: $20-40/month

**Split mode:**
- 1x 2-core 4GB Handler (domestic): $5-10/month
- 2x 1-core 2GB Exit (overseas): $15-20/month each
- Total: $35-50/month

**Proxmox VE mode (cheapest):**
- 1x 4-core 8GB VPS (domestic): $10-15/month
- 1x 1-core 2GB Exit (overseas): $10/month
- Total: $20-25/month

---

## Development Roadmap

### Phase 1: Core Implementation (4-6 weeks)

- [ ] rkyv frame definition
- [ ] WebSocket client/server
- [ ] Basic TOTP authentication
- [ ] SOCKS5 proxy
- [ ] Single-node MVCC storage

### Phase 2: Distributed System (4-6 weeks)

- [ ] Raft consensus integration
- [ ] ClickHouse backup
- [ ] Multi-pod deployment
- [ ] tmpfs shared state
- [ ] Exit node forwarding

### Phase 3: Security & Obfuscation (3-4 weeks)

- [ ] Full authentication flow
- [ ] Key rotation
- [ ] DoH over WSS
- [ ] Traffic obfuscation
- [ ] Emergency mode

### Phase 4: Operations (2-3 weeks)

- [ ] Helm chart
- [ ] One-click install script
- [ ] Monitoring & metrics
- [ ] Documentation
- [ ] Client binary releases

### Phase 5: Advanced Features (optional)

- [ ] SSH fallback transport
- [ ] QUIC transport
- [ ] Plugin system (Unix socket)
- [ ] Web UI dashboard
- [ ] Mobile client (iOS/Android)

---

## Appendix

### Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
rkyv = { version = "0.7", features = ["validation"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# Crypto
aes-gcm = "0.10"
ed25519-dalek = "2"
x25519-dalek = "2"
hmac = "0.12"
sha2 = "0.10"

# Compression
zstd = "0.13"

# Network
hyper = { version = "0.14", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
trust-dns-resolver = "0.23"

# Storage
sled = "0.34"
clickhouse = "0.11"

# Distributed
raft = "0.7"

# Monitoring
prometheus = "0.13"
tracing = "0.1"
tracing-subscriber = "0.3"

# Utils
anyhow = "1"
thiserror = "1"
dashmap = "5"
parking_lot = "0.12"
mimalloc = "0.1"
```

### Project Structure

```
apfsds/
├── crates/
│   ├── protocol/       # Frame definitions
│   ├── crypto/         # Encryption/signing
│   ├── transport/      # WSS/SSH/QUIC
│   ├── storage/        # MVCC engine
│   ├── raft/           # Raft integration
│   └── obfuscation/    # Traffic obfuscation
├── daemon/             # Server binary
├── client/             # Client binary
├── helm-chart/         # Kubernetes deployment
├── scripts/            # Install scripts
├── docs/               # Documentation
├── benches/            # Benchmarks
└── examples/           # Usage examples
```

### References

- [RFC 6455 - WebSocket Protocol](https://tools.ietf.org/html/rfc6455)
- [RFC 8446 - TLS 1.3](https://tools.ietf.org/html/rfc8446)
- [B-link Tree Paper](https://dl.acm.org/doi/10.1145/319628.319663)
- [MVCC in PostgreSQL](https://www.postgresql.org/docs/current/mvcc.html)
- [Raft Consensus](https://raft.github.io/)

---

**End of Specifications**

*Last updated: 2026-01-10*
*Version: 0.1.0*
