//! Local DNS server implementation
//!
//! Provides a local UDP DNS server that forwards queries over the secure WSS tunnel.

use crate::config::ClientConfig;
use anyhow::Result;
use apfsds_obfuscation::{PaddingStrategy, XorMask};
use apfsds_protocol::{ControlMessage, FrameFlags, ProxyFrame};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Run the local DNS server
pub async fn run(config: &ClientConfig) -> Result<()> {
    if !config.dns.enabled {
        return Ok(());
    }

    let udp_socket = Arc::new(UdpSocket::bind(config.dns.bind).await?);
    info!("Local DNS server listening on {}", config.dns.bind);

    // Connect to Daemon WSS
    info!("Connecting to upstream for DNS...");

    // Connect with retry logic
    loop {
        match crate::wss::WssSession::connect(config).await {
            Ok(session) => {
                info!("Connected to Daemon WSS for DNS");
                let conn_id = session.conn_id;
                let (wss_tx, mut wss_rx) = session.split();

                let udp_socket_rx = udp_socket.clone();
                let udp_socket_tx = udp_socket.clone(); // Needed if we implement reply mapping

                // UDP -> WSS
                let udp_task = tokio::spawn(async move {
                    let mut buf = [0u8; 4096];

                    loop {
                        match udp_socket_rx.recv_from(&mut buf).await {
                            Ok((len, _)) => {
                                let query = buf[..len].to_vec();
                                let msg = ControlMessage::DohQuery { query };

                                let payload = match rkyv::to_bytes::<rkyv::rancor::Error>(&msg) {
                                    Ok(b) => b.to_vec(),
                                    Err(_) => continue,
                                };

                                let mut frame = ProxyFrame::new_control(payload);
                                frame.conn_id = conn_id;

                                if let Err(e) = wss_tx.send_frame(&frame).await {
                                    error!("WS send error: {}", e);
                                    break;
                                }
                            }
                            Err(e) => error!("UDP recv error: {}", e),
                        }
                    }
                });

                // WSS -> UDP
                while let Ok(Some(frame)) = wss_rx.recv_frame().await {
                    if frame.flags.is_control {
                        if let Ok(ctrl) =
                            rkyv::from_bytes::<ControlMessage, rkyv::rancor::Error>(&frame.payload)
                        {
                            if let ControlMessage::DohResponse { response } = ctrl {
                                // Send back to UDP
                                debug!(
                                    "Received DNS Response ({} bytes), dropping (no src addr map)",
                                    response.len()
                                );
                            }
                        }
                    }
                }

                info!("WSS connection lost, reconnecting...");
            }
            Err(e) => {
                error!("Failed to connect to WSS: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
