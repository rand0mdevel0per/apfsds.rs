//! HTTP and WebSocket handler

use crate::config::DaemonConfig;
use crate::exit_forwarder::ExitForwarder;
use anyhow::Result;
use apfsds_raft::RaftNode;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::{Request, Response, body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info};
use std::sync::LazyLock;
use crate::metrics::Metrics;

/// Global metrics instance
static METRICS: LazyLock<Metrics> = LazyLock::new(Metrics::new);

use crate::billing::BillingAggregator;
use crate::connection_registry::ConnectionRegistry;
use apfsds_storage::postgres::PgClient;
// Need ProxyFrame

/// Run as handler (main proxy server)
pub async fn run_handler(
    config: &DaemonConfig,
    exit_forwarder: Arc<ExitForwarder>,
    raft_node: Arc<RaftNode>,
    pg_client: PgClient,
    billing: Arc<BillingAggregator>,
    registry: Arc<ConnectionRegistry>,
) -> Result<()> {
    let listener = TcpListener::bind(config.server.bind).await?;
    info!("Handler listening on {}", config.server.bind);

    let config = Arc::new(config.clone());

    loop {
        let (stream, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        let config = config.clone();
        let exit_forwarder = exit_forwarder.clone();
        let raft_node = raft_node.clone();
        let pg_client = pg_client.clone();
        let billing = billing.clone();
        let registry = registry.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let service = service_fn(move |req| {
                let config = config.clone();
                let exit_forwarder = exit_forwarder.clone();
                let raft_node = raft_node.clone();
                let pg_client = pg_client.clone();
                let billing = billing.clone();
                let registry = registry.clone();
                async move {
                    handle_request(
                        req,
                        addr,
                        &config,
                        exit_forwarder,
                        raft_node,
                        pg_client,
                        billing,
                        registry,
                    )
                    .await
                }
            });

            if let Err(e) = http1::Builder::new()
                .serve_connection(io, service)
                .with_upgrades()
                .await
            {
                // error!("Connection error from {}: {}", addr, e);
            }
        });
    }
}

/// Handle HTTP request
async fn handle_request(
    req: Request<Incoming>,
    addr: SocketAddr,
    config: &DaemonConfig,
    exit_forwarder: Arc<ExitForwarder>,
    raft_node: Arc<RaftNode>,
    pg_client: PgClient,
    billing: Arc<BillingAggregator>,
    registry: Arc<ConnectionRegistry>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    // trace!("Request from {}: {} {}", addr, req.method(), path);

    let response = match path {
        "/retrieve-token" => handle_retrieve_token(req, config, pg_client).await,
        "/connect" => {
            handle_connect(req, config, exit_forwarder, raft_node, billing, registry).await
        }
        "/health" => handle_health().await,
        "/ready" => handle_ready().await,
        _ => handle_decoy(req).await,
    };

    match response {
        Ok(resp) => Ok(resp),
        Err(e) => {
            error!("Request error: {}", e);
            Ok(Response::builder()
                .status(500)
                .body(Full::new(Bytes::from("Internal Server Error")))
                .unwrap())
        }
    }
}

/// Handle token retrieval request with 200ms constant-time response
async fn handle_retrieve_token(
    req: Request<Incoming>,
    config: &DaemonConfig,
    _pg_client: PgClient,
) -> Result<Response<Full<Bytes>>> {
    use apfsds_crypto::{Aes256GcmCipher, HmacAuthenticator, X25519KeyPair};
    use apfsds_protocol::{AuthRequest, AuthResponse};
    use http_body_util::BodyExt;

    let start = std::time::Instant::now();

    // Result holder for constant-time response
    let result: Result<Vec<u8>, &'static str> = async {
        // Read body
        let body = req
            .into_body()
            .collect()
            .await
            .map_err(|_| "Failed to read body")?
            .to_bytes();

        if body.len() < 32 {
            return Err("Body too short");
        }

        // First 32 bytes are client's ephemeral X25519 public key
        let client_pk: [u8; 32] = body[..32].try_into().map_err(|_| "Invalid public key")?;

        // Rest is AES-GCM encrypted payload
        let encrypted = &body[32..];

        // Derive shared secret using server's X25519 key
        let server_sk = config
            .security
            .server_sk
            .as_ref()
            .and_then(|s| hex::decode(s).ok())
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .unwrap_or([42u8; 32]); // Default for testing

        let server_x25519 = X25519KeyPair::from_secret(&server_sk);
        let shared_secret = server_x25519.diffie_hellman(&client_pk);

        // Decrypt
        let aes = Aes256GcmCipher::new(&shared_secret);
        let decrypted = aes.decrypt(encrypted).map_err(|_| "Decryption failed")?;

        // Parse AuthRequest
        let auth_req: AuthRequest =
            rkyv::from_bytes::<AuthRequest, rkyv::rancor::Error>(&decrypted)
                .map_err(|_| "Invalid auth request")?;

        // Verify HMAC
        let hmac_secret = config
            .security
            .hmac_secret
            .as_ref()
            .and_then(|s| hex::decode(s).ok())
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .unwrap_or([43u8; 32]);

        let hmac = HmacAuthenticator::new(hmac_secret);
        hmac.verify_with_timestamp(
            &auth_req.hmac_base,
            auth_req.timestamp,
            &auth_req.hmac_signature,
        )
        .map_err(|_| "HMAC verification failed")?;

        // Extract user_id from hmac_base
        let user_id = std::str::from_utf8(&auth_req.hmac_base)
            .ok()
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        // Generate token
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let token_payload = apfsds_protocol::TokenPayload {
            user_id,
            nonce: auth_req.nonce,
            issued_at: now,
            valid_until: now + config.security.token_ttl * 1000,
        };

        let token_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&token_payload)
            .map_err(|_| "Token serialization failed")?
            .to_vec();

        // Build response
        let response = AuthResponse {
            token: token_bytes,
            valid_until: token_payload.valid_until,
            warning: None, // TODO: Check emergency mode
        };

        let response_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&response)
            .map_err(|_| "Response serialization failed")?
            .to_vec();

        // Encrypt response with shared secret
        let encrypted_response = aes
            .encrypt(&response_bytes)
            .map_err(|_| "Response encryption failed")?;

        Ok(encrypted_response)
    }
    .await;

    // Constant-time: always wait until 200ms have passed
    let elapsed = start.elapsed();
    if elapsed < std::time::Duration::from_millis(200) {
        tokio::time::sleep(std::time::Duration::from_millis(200) - elapsed).await;
    }

    match result {
        Ok(data) => {
            METRICS.auth_successes.inc();
            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "application/octet-stream")
                .body(Full::new(Bytes::from(data)))
                .unwrap())
        }
        Err(_) => {
            METRICS.auth_failures.inc();
            // Return same status to avoid timing oracle
            Ok(Response::builder()
                .status(401)
                .header("Content-Type", "application/octet-stream")
                .body(Full::new(Bytes::from("Unauthorized")))
                .unwrap())
        }
    }
}

/// Handle WebSocket connect request
async fn handle_connect(
    req: Request<Incoming>,
    _config: &DaemonConfig,
    exit_forwarder: Arc<ExitForwarder>,
    raft_node: Arc<RaftNode>,
    billing: Arc<BillingAggregator>,
    registry: Arc<ConnectionRegistry>,
) -> Result<Response<Full<Bytes>>> {
    // Check for WebSocket upgrade
    let is_upgrade = req
        .headers()
        .get("upgrade")
        .map(|v| v.to_str().ok())
        .flatten()
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    if !is_upgrade {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Expected WebSocket upgrade")))
            .unwrap());
    }

    // Auth (Stub) - Extract User/Group logic here
    // For now assuming group 0.
    let user_id = 1;
    let group_id = 0;

    // Spawn WebSocket handler
    tokio::task::spawn(async move {
        use apfsds_obfuscation::{PaddingStrategy, XorMask};
        use apfsds_protocol::{ControlMessage, ProxyFrame};
        use tokio::net::UdpSocket;

        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                let mut ws_stream = match accept_async(TokioIo::new(upgraded)).await {
                    Ok(ws) => ws,
                    Err(e) => {
                        error!("WS accept error: {}", e);
                        return;
                    }
                };

                info!("Client connected (User {})", user_id);
                METRICS.active_connections.inc();

                // Conn ID allocation
                let conn_id = fastrand::u64(..);

                // Send Conn ID to client (Key Exchange)
                if let Err(e) = ws_stream
                    .send(Message::Binary(conn_id.to_le_bytes().to_vec().into()))
                    .await
                {
                    error!("Failed to send handshake: {}", e);
                    return;
                }

                // Session key for XOR mask
                let session_key = conn_id;
                let xor_mask = XorMask::new(session_key);
                let padding = PaddingStrategy::default();

                let (mut ws_tx, mut ws_rx) = ws_stream.split();

                // Registry Channel
                let (registry_tx, mut registry_rx) = mpsc::unbounded_channel();
                let dns_registry_tx = registry_tx.clone(); // Clone for DNS listener
                registry.register(conn_id, registry_tx);

                // DNS Socket (Per connection)
                let dns_socket = match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        error!("Failed to bind DNS socket: {}", e);
                        return;
                    }
                };

                // Task: Registry Rx/DNS -> WS Tx (with obfuscation)
                let registry_clone = registry.clone();
                let tx_task = tokio::spawn(async move {
                    let xor_mask = XorMask::new(session_key);
                    let padding = PaddingStrategy::default();

                    while let Some(frame) = registry_rx.recv().await {
                        // Serialize frame
                        let frame_bytes = match rkyv::to_bytes::<rkyv::rancor::Error>(&frame) {
                            Ok(b) => b.to_vec(),
                            Err(e) => {
                                error!("Frame serialization error: {}", e);
                                continue;
                            }
                        };

                        // Obfuscate
                        let padded = padding.pad(&frame_bytes);
                        let masked = xor_mask.apply(&padded);

                        if let Err(e) = ws_tx.send(Message::Binary(masked.clone().into())).await {
                            debug!("WS send error: {}", e);
                            break;
                        }
                        METRICS.frames_sent.inc();
                        METRICS.frame_size.observe(masked.len() as f64);
                    }
                    debug!("WS Tx loop ended");
                });

                // Task: WS Rx -> Exit/DNS (with de-obfuscation)
                let exit_forwarder = exit_forwarder.clone();
                let dns_socket_clone = dns_socket.clone();

                // DNS Response Listener Task
                let dns_listener = tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        match dns_socket_clone.recv_from(&mut buf).await {
                            Ok((len, _)) => {
                                let response = buf[..len].to_vec();
                                let msg = ControlMessage::DohResponse { response };
                                if let Ok(payload) = rkyv::to_bytes::<rkyv::rancor::Error>(&msg) {
                                    let mut frame = ProxyFrame::new_control(payload.to_vec());
                                    frame.conn_id = conn_id; // Route to this client
                                    let _ = dns_registry_tx.send(frame);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                while let Some(msg) = ws_rx.next().await {
                    match msg {
                        Ok(Message::Binary(data)) => {
                            METRICS.frames_received.inc();
                            METRICS.frame_size.observe(data.len() as f64);
                            
                            // De-obfuscate
                            let unmasked = xor_mask.apply(&data);
                            let unpadded = match PaddingStrategy::unpad(&unmasked) {
                                Some(data) => data,
                                None => continue,
                            };

                            // Parse ProxyFrame
                            let frame = match rkyv::from_bytes::<ProxyFrame, rkyv::rancor::Error>(
                                &unpadded,
                            ) {
                                Ok(f) => f,
                                Err(e) => {
                                    error!("Invalid frame: {}", e);
                                    continue;
                                }
                            };

                            if frame.flags.is_control {
                                if let Ok(ctrl) = rkyv::from_bytes::<
                                    ControlMessage,
                                    rkyv::rancor::Error,
                                >(&frame.payload)
                                {
                                    match ctrl {
                                        ControlMessage::DohQuery { query } => {
                                            // Forward to Google DNS
                                            // Note: We use the connection-specific socket
                                            let _ = dns_socket.send_to(&query, "8.8.8.8:53").await;
                                        }
                                        _ => {}
                                    }
                                }
                            } else {
                                // Data Frame -> Exit Node
                                if let Err(e) = exit_forwarder.forward(&frame, group_id).await {
                                    error!("Forward error: {}", e);
                                    break;
                                }
                                billing
                                    .record_usage(user_id, frame.payload.len() as u64)
                                    .await;
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Err(_) => break,
                        _ => {}
                    }
                }

                registry_clone.unregister(conn_id);
                let _ = tx_task.await;
                let _ = dns_listener.await;
                METRICS.active_connections.dec();
                info!("Client disconnected (User {})", user_id);
            }
            Err(e) => error!("Upgrade error: {}", e),
        }
    });

    Ok(Response::builder()
        .status(101)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Accept", "auth-mock")
        .body(Full::new(Bytes::new()))
        .unwrap())
}

/// Handle health check
async fn handle_health() -> Result<Response<Full<Bytes>>> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(r#"{"status":"healthy"}"#)))
        .unwrap())
}

/// Handle readiness check
async fn handle_ready() -> Result<Response<Full<Bytes>>> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(r#"{"status":"ready"}"#)))
        .unwrap())
}

/// Handle decoy traffic (return static/proxy responses)
async fn handle_decoy(req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Welcome</title></head>
<body>
<h1>Welcome to our website</h1>
<p>This is a normal website. Nothing to see here.</p>
</body>
</html>"#;

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Full::new(Bytes::from(html)))
        .unwrap())
}

/// Run as exit node (simple forwarder) is deprecated/moved, but kept stub if needed by old calls
/// But we updated main.rs to use exit_node::run
pub async fn run_exit(_config: &DaemonConfig) -> Result<()> {
    panic!("Use exit_node::run instead");
}
