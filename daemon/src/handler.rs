//! HTTP and WebSocket handler

use crate::config::DaemonConfig;
use anyhow::Result;
use hyper::{body::Incoming, Request, Response, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use http_body_util::{BodyExt, Full};
use bytes::Bytes;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, trace, warn};

/// Run as handler (main proxy server)
pub async fn run_handler(config: &DaemonConfig) -> Result<()> {
    let listener = TcpListener::bind(config.server.bind).await?;
    info!("Handler listening on {}", config.server.bind);

    let config = Arc::new(config.clone());

    loop {
        let (stream, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        let config = config.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);

            let service = service_fn(move |req| {
                let config = config.clone();
                async move { handle_request(req, addr, &config).await }
            });

            if let Err(e) = http1::Builder::new()
                .serve_connection(io, service)
                .with_upgrades()
                .await
            {
                error!("Connection error from {}: {}", addr, e);
            }
        });
    }
}

/// Handle HTTP request
async fn handle_request(
    req: Request<Incoming>,
    addr: SocketAddr,
    config: &DaemonConfig,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    trace!("Request from {}: {} {}", addr, req.method(), path);

    let response = match path {
        "/retrieve-token" => handle_retrieve_token(req, config).await,
        "/connect" => handle_connect(req, config).await,
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

/// Handle token retrieval request
async fn handle_retrieve_token(
    req: Request<Incoming>,
    _config: &DaemonConfig,
) -> Result<Response<Full<Bytes>>> {
    let start = std::time::Instant::now();

    // TODO: Implement full authentication flow
    // For now, return a placeholder response

    // Ensure constant-time response (200ms)
    let elapsed = start.elapsed();
    if elapsed < std::time::Duration::from_millis(200) {
        tokio::time::sleep(std::time::Duration::from_millis(200) - elapsed).await;
    }

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/octet-stream")
        .body(Full::new(Bytes::from("token-placeholder")))
        .unwrap())
}

/// Handle WebSocket connect request
async fn handle_connect(
    req: Request<Incoming>,
    _config: &DaemonConfig,
) -> Result<Response<Full<Bytes>>> {
    // Check for WebSocket upgrade
    let is_upgrade = req.headers()
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

    // TODO: Implement WebSocket upgrade and handling
    // For now, return a placeholder

    Ok(Response::builder()
        .status(501)
        .body(Full::new(Bytes::from("WebSocket not yet implemented")))
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
    // Simulate a normal website response
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

/// Run as exit node (simple forwarder)
pub async fn run_exit(config: &DaemonConfig) -> Result<()> {
    let listener = TcpListener::bind(config.server.bind).await?;
    info!("Exit node listening on {}", config.server.bind);

    loop {
        let (mut stream, addr) = listener.accept().await?;
        debug!("New connection from {}", addr);

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            // Read PlainPacket header
            let mut header = [0u8; 28];
            if stream.read_exact(&mut header).await.is_err() {
                return;
            }

            let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
            if magic != 0xDEADBEEF {
                warn!("Invalid magic from {}", addr);
                return;
            }

            // TODO: Parse target and forward
            // For now, just close the connection
            let _ = stream.shutdown().await;
        });
    }
}
