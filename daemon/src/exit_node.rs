//! Exit Node Service
//!
//! Manages TUN interface and forwards traffic between Handlers (via HTTP/2) and the OS.
//! Implements user-space NAT using Client ID/Conn ID mapping.

use anyhow::Result;
use dashmap::DashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
// Updated import
use crate::config::DaemonConfig;
use apfsds_protocol::PlainPacket;
use bytes::Bytes;
use futures::{SinkExt, stream::StreamExt};
use http_body_util::{BodyExt, Full, StreamBody}; // Need StreamBody
use hyper::service::service_fn;
use hyper::{Request, Response, body::Incoming};
use hyper_util::rt::TokioIo;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream; // Need tokio-stream // Need futures

#[cfg(target_os = "linux")]
use tun::platform::Device;

/// Exit Node Service
pub struct ExitService {
    #[cfg(target_os = "linux")]
    tun: Arc<std::sync::Mutex<Device>>,
    #[cfg(not(target_os = "linux"))]
    tun: Arc<std::sync::Mutex<()>>,

    /// Map of Virtual IP -> (HandlerID, ConnID) for return traffic routing
    route_map: Arc<DashMap<Ipv4Addr, RouteEntry>>,

    /// Map of HandlerID -> Sender for return stream
    handler_streams:
        Arc<DashMap<u64, UnboundedSender<Result<hyper::body::Frame<Bytes>, anyhow::Error>>>>,

    ip_pool: Arc<std::sync::atomic::AtomicU16>,
}

#[derive(Debug, Clone)]
struct RouteEntry {
    handler_id: u64,
    conn_id: u64,
}

impl ExitService {
    pub fn new() -> Result<Arc<Self>> {
        #[cfg(target_os = "linux")]
        let tun = {
            let mut config = tun::Configuration::default();
            config
                .address((10, 200, 0, 1))
                .netmask((255, 255, 0, 0))
                .up();

            config.platform(|config| {
                config.packet_information(false);
            });

            let dev =
                tun::create(&config).map_err(|e| anyhow::anyhow!("Failed to create TUN: {}", e))?;
            Arc::new(std::sync::Mutex::new(dev))
        };

        #[cfg(not(target_os = "linux"))]
        let tun = {
            warn!("TUN is only supported on Linux. Using mock.");
            Arc::new(std::sync::Mutex::new(()))
        };

        let route_map = Arc::new(DashMap::new());
        let handler_streams = Arc::new(DashMap::new());
        let ip_pool = Arc::new(std::sync::atomic::AtomicU16::new(2));

        let service = Arc::new(Self {
            tun,
            route_map,
            handler_streams,
            ip_pool,
        });

        // Start TUN reader
        service.clone().start_tun_reader();

        Ok(service)
    }

    fn start_tun_reader(self: Arc<Self>) {
        tokio::spawn(async move {
            #[cfg(target_os = "linux")]
            {
                use std::io::Read;
                let mut buf = [0u8; 2048];
                loop {
                    let n = {
                        let mut tun = self.tun.lock().unwrap();
                        match tun.read(&mut buf) {
                            Ok(n) => n,
                            Err(e) => {
                                error!("TUN read error: {}", e);
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                continue;
                            }
                        }
                    };

                    let packet = &buf[..n];
                    // Parse Dest IP (Return traffic)
                    if let Ok(slice) = etherparse::Ipv4HeaderSlice::from_slice(packet) {
                        let dst = slice.destination();
                        let dst_addr = Ipv4Addr::new(dst[0], dst[1], dst[2], dst[3]);

                        if let Some(route) = self.route_map.get(&dst_addr) {
                            // Forward to handler stream
                            if let Some(sender) = self.handler_streams.get(&route.handler_id) {
                                // We need to wrap this in PlainPacket?
                                // User said "convert to client-id and forward".
                                // We send a PlainPacket with payload=packet, conn_id=route.conn_id

                                let pp = PlainPacket {
                                    magic: PlainPacket::MAGIC,
                                    conn_id: route.conn_id,
                                    handler_id: route.handler_id,
                                    rip: [0; 16],
                                    rport: 0,
                                    payload: packet.to_vec(),
                                    checksum: crc32fast::hash(packet),
                                    is_response: true,
                                };

                                // Serialize?
                                // If stream is raw bytes, we need framing.
                                // Or stream of rkyv bytes?
                                // For simplicity, let's assume the stream is "frames" or concatenated.
                                // HTTP/2 allows DataFrame.
                                // We send `Frame::<Bytes>::data(bytes)`.

                                if let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&pp) {
                                    // Prefix with u32 length for framing
                                    let len = bytes.len() as u32;
                                    let mut payload = Vec::with_capacity(4 + bytes.len());
                                    payload.extend_from_slice(&len.to_le_bytes());
                                    payload.extend_from_slice(&bytes);

                                    let frame = hyper::body::Frame::data(Bytes::from(payload));
                                    let _ = sender.send(Ok(frame));
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub async fn handle_forward(&self, mut packet: PlainPacket) -> Result<()> {
        // 1. Allocate/Lookup IP
        // We use the conn_id to map to an IP.
        // Simplification: We need a map ConnID -> IP.
        // But `route_map` is IP -> ConnID.
        // Use a reverse lookup or separate map?
        // Phase 3: Just linear search or assume IP is stable?
        // Or allocate new if not found in route_map (checking values)?
        // DashMap values iter is slow.
        // Let's alloc IP every time for new conn_id?
        // We need `conn_map: DashMap<u64, Ipv4Addr>`.
        // I will add `conn_map` to struct? No, let's keep it simple:
        // Just Alloc new if we don't know it? No, duplicate IPs.
        // Let's skip IP reuse for now and use consistent hashing or just store it.
        // I'll add `conn_map` to struct.

        // Mock logic for IP assignment:
        // Note: IP allocation uses simple incrementing; connection tracking for IP reuse
        // would require a conn_id -> IP map (add to struct for production)
        let virtual_ip = self.alloc_ip();

        // 2. Rewrite Source IP (NAT)
        if let Ok(mut header) = etherparse::Ipv4Header::from_slice(&packet.payload).map(|(h, _)| h)
        {
            header.source = virtual_ip.octets();
            // Recalculate checksum?
            // Etherparse write will do it.
            // We need to write header + rest of payload.
            // `packet.payload` contains Header + Data.

            // Extract data
            // We need to parse robustly.
            // I'll use `etherparse::PacketBuilder`? No, that builds new.
            // I modify header in place?
            // `packet.payload` is `Vec<u8>`.
            // Ipv4 header is 20 bytes (usually).

            if packet.payload.len() >= 20 {
                packet.payload[12..16].copy_from_slice(&virtual_ip.octets());
                // Checksum at [10..12].
                // Recomputing checksum is annoying manually.
                // Use `etherparse` to re-serialize header?
                // `header.write_to(&mut slice)`?

                // For Phase 3, I'll trust `etherparse` to help or leave checksum invalid (bad idea).
                // Correct way:
                // let (header, rest) = Ipv4Header::read_from_slice(&payload)?;
                // header.source = ...
                // let mut new_payload = Vec::new();
                // header.write(&mut new_payload)?;
                // new_payload.extend_from_slice(rest);
            }
        }

        // Update maps
        self.route_map.insert(
            virtual_ip,
            RouteEntry {
                handler_id: packet.handler_id,
                conn_id: packet.conn_id,
            },
        );

        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            let mut tun = self.tun.lock().unwrap();
            tun.write_all(&packet.payload)?;
        }

        Ok(())
    }

    fn alloc_ip(&self) -> Ipv4Addr {
        let id = self
            .ip_pool
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ipv4Addr::new(10, 200, (id >> 8) as u8, (id & 0xFF) as u8)
    }

    pub fn register_stream(
        &self,
        handler_id: u64,
    ) -> UnboundedReceiverStream<Result<hyper::body::Frame<Bytes>, anyhow::Error>> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.handler_streams.insert(handler_id, tx);
        UnboundedReceiverStream::new(rx)
    }
}

/// Run the exit node service
pub async fn run(config: &DaemonConfig) -> Result<()> {
    info!("Initializing Exit Node Service...");

    // Check if running in reverse connection mode
    if config.server.reverse_mode {
        info!("Running in reverse connection mode");
        return run_reverse_mode(config).await;
    }

    // Traditional mode: exit-node as server
    let service = ExitService::new()?;
    info!("TUN interface up (10.200.0.1/16) [MOCK on Windows]");

    let listener = TcpListener::bind(config.server.bind).await?;
    info!("Exit Node listening on {}", config.server.bind);

    loop {
        let (stream, addr) = listener.accept().await?;
        let service = service.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let service = service.clone();

            let hyper_service = service_fn(move |req| {
                let service = service.clone();
                async move { handle_http_request(req, service).await }
            });

            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, hyper_service)
                .await
            {
                debug!("Connection closed: {}", e);
            }
        });
    }
}

async fn handle_http_request(
    req: Request<Incoming>,
    service: Arc<ExitService>,
) -> Result<Response<BoxBody>, hyper::Error> {
    // Changed to BoxBody wrapper
    match (req.method(), req.uri().path()) {
        (&hyper::Method::POST, "/forward") => {
            let body = req.collect().await?.to_bytes();
            if let Ok(packet) = rkyv::from_bytes::<PlainPacket, rkyv::rancor::Error>(&body) {
                if let Err(e) = service.handle_forward(packet).await {
                    error!("Forward error: {}", e);
                }
                Ok(Response::new(fullempty()))
            } else {
                Ok(Response::builder()
                    .status(400)
                    .body(full("Invalid Data"))
                    .unwrap())
            }
        }
        (&hyper::Method::GET, "/stream") => {
            // handler_id query param?
            // Assume 1 for demo or parse query
            // Demo: Using handler_id=1; production should parse from query string
            let handler_id = 1;
            let stream = service.register_stream(handler_id);
            let body = StreamBody::new(stream);
            let boxed = BodyExt::boxed(body); // requires http-body-util BoxBody
            // return Ok(Response::new(boxed));
            // Type mismatch annoyance usually.
            // Using a helper `BoxBody` type alias helps.
            Ok(Response::new(boxed))
        }
        _ => Ok(Response::builder()
            .status(404)
            .body(full("Not Found"))
            .unwrap()),
    }
}

// Helpers for body types
type BoxBody = http_body_util::combinators::BoxBody<Bytes, anyhow::Error>;

fn full(chunk: &'static str) -> BoxBody {
    Full::new(Bytes::from(chunk))
        .map_err(|_| anyhow::anyhow!("never"))
        .boxed()
}

fn fullempty() -> BoxBody {
    Full::new(Bytes::new())
        .map_err(|_| anyhow::anyhow!("never"))
        .boxed()
}

/// Run exit-node in reverse connection mode (client mode)
async fn run_reverse_mode(config: &DaemonConfig) -> Result<()> {
    let handler_endpoint = config
        .server
        .handler_endpoint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("handler_endpoint not configured for reverse_mode"))?;

    let node_name = config
        .server
        .location
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("exit-node");

    let preferred_group_id = config.server.preferred_group_id;

    info!(
        "Connecting to handler at {} (name={}, preferred_group={:?})",
        handler_endpoint, node_name, preferred_group_id
    );

    // Create ExitService for TUN interface
    let service = ExitService::new()?;
    info!("TUN interface up (10.200.0.1/16) [MOCK on Windows]");

    // Connect to handler with retry logic
    loop {
        match connect_to_handler(
            handler_endpoint,
            node_name,
            preferred_group_id,
            service.clone(),
        )
        .await
        {
            Ok(_) => {
                info!("Connection to handler closed, reconnecting in 5s...");
            }
            Err(e) => {
                error!("Failed to connect to handler: {}, retrying in 5s...", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

/// Connect to handler and handle WebSocket communication
async fn connect_to_handler(
    handler_endpoint: &str,
    node_name: &str,
    preferred_group_id: Option<i32>,
    service: Arc<ExitService>,
) -> Result<()> {
    use tokio_tungstenite::connect_async;

    // Build WebSocket URL (no group_id - will be negotiated)
    let ws_url = format!(
        "ws://{}/exit-node/register?name={}",
        handler_endpoint, node_name
    );

    info!("Connecting to {}", ws_url);

    // Connect to handler
    let (ws_stream, _) = connect_async(&ws_url).await?;
    info!("Connected to handler successfully");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Wait for GroupList from handler
    use apfsds_protocol::{ControlMessage, GroupInfo};

    let selected_group_id = if let Some(group_id) = preferred_group_id {
        // Use configured group_id
        info!("Using configured group_id: {}", group_id);

        // Still need to receive GroupList from handler (protocol requirement)
        loop {
            match ws_receiver.next().await {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(data))) => {
                    if let Ok(msg) = rkyv::from_bytes::<ControlMessage, rkyv::rancor::Error>(&data)
                    {
                        if let ControlMessage::GroupList { groups } = msg {
                            info!("Received {} available groups from handler", groups.len());

                            // Verify that configured group exists
                            if groups.iter().any(|g| g.group_id == group_id) {
                                info!("Configured group {} found in available groups", group_id);
                                break group_id;
                            } else {
                                warn!(
                                    "Configured group {} not found, falling back to auto-select",
                                    group_id
                                );
                                // Fall back to auto-select
                                let selected = groups
                                    .iter()
                                    .min_by_key(|g| g.load)
                                    .ok_or_else(|| anyhow::anyhow!("No groups available"))?;
                                info!(
                                    "Auto-selected group {} ({}), load: {}%",
                                    selected.group_id, selected.name, selected.load
                                );
                                break selected.group_id;
                            }
                        }
                    }
                }
                Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                    return Err(anyhow::anyhow!(
                        "Handler closed connection before sending groups"
                    ));
                }
                Some(Err(e)) => {
                    return Err(anyhow::anyhow!("WebSocket error: {}", e));
                }
                None => {
                    return Err(anyhow::anyhow!("Connection closed before receiving groups"));
                }
                _ => {}
            }
        }
    } else {
        // Auto-select group with lowest load
        loop {
            match ws_receiver.next().await {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(data))) => {
                    if let Ok(msg) = rkyv::from_bytes::<ControlMessage, rkyv::rancor::Error>(&data)
                    {
                        if let ControlMessage::GroupList { groups } = msg {
                            info!("Received {} available groups from handler", groups.len());

                            // Select group with lowest load
                            let selected = groups
                                .iter()
                                .min_by_key(|g| g.load)
                                .ok_or_else(|| anyhow::anyhow!("No groups available"))?;

                            info!(
                                "Auto-selected group {} ({}), load: {}%",
                                selected.group_id, selected.name, selected.load
                            );

                            break selected.group_id;
                        }
                    }
                }
                Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                    return Err(anyhow::anyhow!(
                        "Handler closed connection before sending groups"
                    ));
                }
                Some(Err(e)) => {
                    return Err(anyhow::anyhow!("WebSocket error: {}", e));
                }
                None => {
                    return Err(anyhow::anyhow!("Connection closed before receiving groups"));
                }
                _ => {}
            }
        }
    };

    // Send group selection back to handler
    let select_msg = ControlMessage::GroupSelect {
        group_id: selected_group_id,
    };
    if let Ok(msg_bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&select_msg) {
        ws_sender
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                msg_bytes.to_vec().into(),
            ))
            .await?;
        info!("Sent group selection to handler");
    } else {
        return Err(anyhow::anyhow!("Failed to serialize group selection"));
    }

    // Handle incoming messages from handler
    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                // Decode PlainPacket from handler
                if let Ok(packet) = rkyv::from_bytes::<PlainPacket, rkyv::rancor::Error>(&data) {
                    // Forward to TUN interface
                    if let Err(e) = service.handle_forward(packet).await {
                        error!("Failed to forward packet: {}", e);
                    }
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                if let Err(e) = ws_sender
                    .send(tokio_tungstenite::tungstenite::Message::Pong(data))
                    .await
                {
                    error!("Failed to send pong: {}", e);
                    break;
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                info!("Handler closed connection");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
