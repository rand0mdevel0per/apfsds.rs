//! SOCKS5 proxy server

use crate::config::ClientConfig;
use anyhow::Result;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, trace, warn};

/// SOCKS5 version
const SOCKS5_VERSION: u8 = 0x05;

/// SOCKS5 authentication methods
const AUTH_NO_AUTH: u8 = 0x00;

/// SOCKS5 commands
const CMD_CONNECT: u8 = 0x01;

/// SOCKS5 address types
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

/// SOCKS5 reply codes
const REP_SUCCESS: u8 = 0x00;
const REP_GENERAL_FAILURE: u8 = 0x01;
const REP_CONNECTION_NOT_ALLOWED: u8 = 0x02;
const REP_NETWORK_UNREACHABLE: u8 = 0x03;
const REP_HOST_UNREACHABLE: u8 = 0x04;
const REP_CONNECTION_REFUSED: u8 = 0x05;

/// Run the SOCKS5 server
pub async fn run(config: &ClientConfig) -> Result<()> {
    let listener = TcpListener::bind(config.socks5.bind).await?;
    info!("SOCKS5 server listening on {}", config.socks5.bind);

    loop {
        let (stream, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        let config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, &config).await {
                error!("Connection error from {}: {}", addr, e);
            }
        });
    }
}

/// Handle a single SOCKS5 connection
async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    config: &ClientConfig,
) -> Result<()> {
    // Check emergency mode
    if crate::emergency::is_emergency_mode() {
        warn!("Rejecting connection due to emergency mode");
        return Ok(());
    }

    // 1. Handshake
    let version = stream.read_u8().await?;
    if version != SOCKS5_VERSION {
        return Err(anyhow::anyhow!("Invalid SOCKS version: {}", version));
    }

    let nmethods = stream.read_u8().await?;
    let mut methods = vec![0u8; nmethods as usize];
    stream.read_exact(&mut methods).await?;

    // We only support no-auth for now
    if !methods.contains(&AUTH_NO_AUTH) {
        stream.write_all(&[SOCKS5_VERSION, 0xFF]).await?;
        return Err(anyhow::anyhow!("No acceptable auth method"));
    }

    // Accept no-auth
    stream.write_all(&[SOCKS5_VERSION, AUTH_NO_AUTH]).await?;

    // 2. Request
    let version = stream.read_u8().await?;
    let cmd = stream.read_u8().await?;
    let _rsv = stream.read_u8().await?;
    let atyp = stream.read_u8().await?;

    if version != SOCKS5_VERSION {
        return Err(anyhow::anyhow!("Invalid version in request"));
    }

    if cmd != CMD_CONNECT {
        send_reply(&mut stream, REP_GENERAL_FAILURE).await?;
        return Err(anyhow::anyhow!("Unsupported command: {}", cmd));
    }

    // Parse target address
    let target = parse_target(&mut stream, atyp).await?;
    debug!("Connection from {} to {}", addr, target);

    // Resolve target to IP/Port (ProxyFrame requires IP)
    let target_sock_addr = match tokio::net::lookup_host(&target).await {
        Ok(mut iter) => iter
            .next()
            .ok_or(anyhow::anyhow!("No IP found for target"))?,
        Err(e) => {
            error!("DNS resolution failed for {}: {}", target, e);
            send_reply(&mut stream, REP_HOST_UNREACHABLE).await?;
            return Ok(());
        }
    };

    // Connect to Daemon via WSS Tunnel
    info!("Tunneling connection to {} via WSS", target);
    match crate::wss::WssSession::connect(config).await {
        Ok(session) => {
            send_reply(&mut stream, REP_SUCCESS).await?;

            let conn_id = session.conn_id; // Capture ID before split
            let (wss_sender, mut wss_receiver) = session.split();
            let (mut client_read, mut client_write) = stream.into_split();

            // Prepare Target Info for ProxyFrame
            let rip = match target_sock_addr.ip() {
                std::net::IpAddr::V4(ip) => ip.to_ipv6_mapped().octets(),
                std::net::IpAddr::V6(ip) => ip.octets(),
            };
            let rport = target_sock_addr.port();

            // Task: TCP -> WSS
            let sender_task = tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                loop {
                    match client_read.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            let frame = apfsds_protocol::ProxyFrame::new_data(
                                conn_id,
                                rip,
                                rport,
                                buf[..n].to_vec(),
                            );
                            if let Err(e) = wss_sender.send_frame(&frame).await {
                                error!("WSS send failed: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("TCP read failed: {}", e);
                            break;
                        }
                    }
                }
            });

            // Task: WSS -> TCP
            while let Ok(Some(frame)) = wss_receiver.recv_frame().await {
                if !frame.flags.is_control {
                    if let Err(e) = client_write.write_all(&frame.payload).await {
                        error!("TCP write failed: {}", e);
                        break;
                    }
                }
            }

            let _ = sender_task.await;
        }
        Err(e) => {
            error!("Failed to connect to WSS Upstream: {}", e);
            send_reply(&mut stream, REP_CONNECTION_REFUSED).await?;
        }
    }

    Ok(())
}

/// Parse target address from SOCKS5 request
async fn parse_target(stream: &mut TcpStream, atyp: u8) -> Result<String> {
    match atyp {
        ATYP_IPV4 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            let port = stream.read_u16().await?;
            Ok(format!(
                "{}.{}.{}.{}:{}",
                addr[0], addr[1], addr[2], addr[3], port
            ))
        }
        ATYP_DOMAIN => {
            let len = stream.read_u8().await? as usize;
            let mut domain = vec![0u8; len];
            stream.read_exact(&mut domain).await?;
            let port = stream.read_u16().await?;
            let domain_str = String::from_utf8(domain)?;
            Ok(format!("{}:{}", domain_str, port))
        }
        ATYP_IPV6 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            let port = stream.read_u16().await?;
            let ipv6 = std::net::Ipv6Addr::from(addr);
            Ok(format!("[{}]:{}", ipv6, port))
        }
        _ => Err(anyhow::anyhow!("Unknown address type: {}", atyp)),
    }
}

/// Send SOCKS5 reply
async fn send_reply(stream: &mut TcpStream, rep: u8) -> Result<()> {
    // Reply: VER REP RSV ATYP BND.ADDR BND.PORT
    // We send 0.0.0.0:0 as bound address
    let reply = [
        SOCKS5_VERSION,
        rep,
        0x00, // RSV
        ATYP_IPV4,
        0,
        0,
        0,
        0, // BND.ADDR
        0,
        0, // BND.PORT
    ];
    stream.write_all(&reply).await?;
    Ok(())
}
