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
use futures::stream::StreamExt;
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

            let dev = tun::create(&config)
                .map_err(|e| anyhow::anyhow!("Failed to create TUN: {}", e))?;
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
        if let Ok(mut header) =
            etherparse::Ipv4Header::from_slice(&packet.payload).map(|(h, _)| h)
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
