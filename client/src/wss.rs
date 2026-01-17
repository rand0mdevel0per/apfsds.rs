//! WSS Client Module
//!
//! Handles strictly typed ProxyFrame communication over WebSocket Secure.
//! Enforces traffic obfuscation (Padding -> Masking) and session key management.

use crate::config::ClientConfig;
use anyhow::{Result, anyhow};
use apfsds_obfuscation::{PaddingStrategy, XorMask};
use apfsds_protocol::ProxyFrame;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{debug, error, info};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsTx = SplitSink<WsStream, Message>;
type WsRx = SplitStream<WsStream>;

/// Encapsulated WSS Session
pub struct WssSession {
    tx: Arc<Mutex<WsTx>>,
    rx: WsRx,
    pub session_key: u64,
    pub conn_id: u64,
}

impl WssSession {
    /// Connect to the configured upstream endpoint
    pub async fn connect(config: &ClientConfig) -> Result<Self> {
        let endpoint = config
            .connection
            .endpoints
            .first()
            .ok_or_else(|| anyhow!("No endpoints configured"))?;

        // Determine scheme based on endpoint prefix or use default ws://
        let url = if endpoint.starts_with("wss://") || endpoint.starts_with("ws://") {
            format!("{}/ws", endpoint)
        } else {
            format!("ws://{}/ws", endpoint)
        };

        info!("Connecting to WSS upstream: {}", url);
        let (ws_stream, _) = connect_async(&url).await?;

        let (mut tx, mut rx) = ws_stream.split();

        // Handshake: Expect 8-byte conn_id from server
        let handshake_msg = rx
            .next()
            .await
            .ok_or_else(|| anyhow!("Connection closed before handshake"))??;

        let conn_id = match handshake_msg {
            Message::Binary(data) => {
                if data.len() != 8 {
                    return Err(anyhow!("Invalid handshake length: {}", data.len()));
                }
                u64::from_le_bytes(data[..8].try_into()?)
            }
            _ => return Err(anyhow!("Invalid handshake message type")),
        };

        debug!("Handshake successful. ConnID: {}", conn_id);

        Ok(Self {
            tx: Arc::new(Mutex::new(tx)),
            rx,
            session_key: conn_id, // Simple derivation as per Phase 3
            conn_id,
        })
    }

    /// Send a ProxyFrame with obfuscation
    pub async fn send_frame(&self, frame: &ProxyFrame) -> Result<()> {
        // Serialize
        let frame_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(frame)?.to_vec();

        // Pad
        let padding = PaddingStrategy::default(); // Uses jitter by default
        let padded = padding.pad(&frame_bytes);

        // Mask
        let xor_mask = XorMask::new(self.session_key);
        let masked = xor_mask.apply(&padded);

        // Send
        let mut tx = self.tx.lock().await;
        tx.send(Message::Binary(masked.into())).await?;

        Ok(())
    }

    /// Receive a ProxyFrame (handling obfuscation)
    /// Returns None if connection closed
    pub async fn recv_frame(&mut self) -> Result<Option<ProxyFrame>> {
        loop {
            let msg = match self.rx.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(None),
            };

            match msg {
                Message::Binary(data) => {
                    // Unmask
                    let xor_mask = XorMask::new(self.session_key);
                    let unmasked = xor_mask.apply(&data);

                    // Unpad
                    let unpadded = match PaddingStrategy::unpad(&unmasked) {
                        Some(d) => d,
                        None => {
                            debug!("Invalid padding, dropping frame");
                            continue;
                        }
                    };

                    // Deserialize
                    let frame = rkyv::from_bytes::<ProxyFrame, rkyv::rancor::Error>(&unpadded)?;
                    return Ok(Some(frame));
                }
                Message::Close(_) => return Ok(None),
                // Handle Pings/Pongs/Text automatically (ignore or respond)
                // Tungstenite handles Ping/Pong control frames internally usually?
                // If it exposes them, we ignore.
                _ => continue,
            }
        }
    }

    /// Split the session to allow independent Rx access (consumes Self)
    pub fn split(self) -> (WssSender, WssReceiver) {
        let tx = WssSender {
            tx: self.tx,
            session_key: self.session_key,
        };
        let rx = WssReceiver {
            rx: self.rx,
            session_key: self.session_key,
        };
        (tx, rx)
    }
}

pub struct WssSender {
    tx: Arc<Mutex<WsTx>>,
    session_key: u64,
}

impl WssSender {
    pub async fn send_frame(&self, frame: &ProxyFrame) -> Result<()> {
        // Same logic as WssSession::send_frame
        let frame_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(frame)?.to_vec();
        let padding = PaddingStrategy::default();
        let padded = padding.pad(&frame_bytes);
        let xor_mask = XorMask::new(self.session_key);
        let masked = xor_mask.apply(&padded);

        let mut tx = self.tx.lock().await;
        tx.send(Message::Binary(masked.into())).await?;
        Ok(())
    }
}

pub struct WssReceiver {
    rx: WsRx,
    session_key: u64,
}

impl WssReceiver {
    pub async fn recv_frame(&mut self) -> Result<Option<ProxyFrame>> {
        // Same logic as WssSession::recv_frame
        loop {
            let msg = match self.rx.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(None),
            };

            match msg {
                Message::Binary(data) => {
                    let xor_mask = XorMask::new(self.session_key);
                    let unmasked = xor_mask.apply(&data);
                    let unpadded = match PaddingStrategy::unpad(&unmasked) {
                        Some(d) => d,
                        None => {
                            debug!("Invalid padding");
                            continue;
                        }
                    };
                    let frame = rkyv::from_bytes::<ProxyFrame, rkyv::rancor::Error>(&unpadded)?;
                    return Ok(Some(frame));
                }
                Message::Close(_) => return Ok(None),
                _ => continue,
            }
        }
    }
}
