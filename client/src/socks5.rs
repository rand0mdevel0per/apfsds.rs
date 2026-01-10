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

    // 3. TODO: Forward through WebSocket to daemon
    // For now, just do direct connection (for testing)
    match TcpStream::connect(&target).await {
        Ok(target_stream) => {
            send_reply(&mut stream, REP_SUCCESS).await?;
            
            // Bidirectional copy
            let (mut client_read, mut client_write) = stream.into_split();
            let (mut target_read, mut target_write) = target_stream.into_split();

            let c2t = tokio::io::copy(&mut client_read, &mut target_write);
            let t2c = tokio::io::copy(&mut target_read, &mut client_write);

            tokio::select! {
                r = c2t => { trace!("client->target finished: {:?}", r); }
                r = t2c => { trace!("target->client finished: {:?}", r); }
            }
        }
        Err(e) => {
            error!("Failed to connect to {}: {}", target, e);
            let reply = match e.kind() {
                std::io::ErrorKind::ConnectionRefused => REP_CONNECTION_REFUSED,
                std::io::ErrorKind::PermissionDenied => REP_CONNECTION_NOT_ALLOWED,
                _ => REP_HOST_UNREACHABLE,
            };
            send_reply(&mut stream, reply).await?;
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
        0, 0, 0, 0, // BND.ADDR
        0, 0, // BND.PORT
    ];
    stream.write_all(&reply).await?;
    Ok(())
}
