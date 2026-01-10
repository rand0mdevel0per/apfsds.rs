//! WebSocket client with Chrome handshake emulation

use futures::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{
        client::IntoClientRequest,
        http::{header, Request},
        Message, protocol::WebSocketConfig,
    },
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info, trace, warn};

/// Chrome 120 User-Agent
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Error, Debug)]
pub enum WssClientError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WssClientConfig {
    /// Target WebSocket URL
    pub url: String,

    /// Authorization token
    pub token: Option<String>,

    /// Custom headers
    pub headers: Vec<(String, String)>,

    /// Enable compression
    pub compression: bool,

    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for WssClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            token: None,
            headers: Vec::new(),
            compression: true,
            timeout_secs: 30,
        }
    }
}

/// WebSocket client wrapper
pub struct WssClient {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WssClient {
    /// Connect to a WebSocket server with Chrome-like handshake
    pub async fn connect(config: WssClientConfig) -> Result<Self, WssClientError> {
        let request = Self::build_request(&config)?;

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = Some(64 << 20); // 64MB
        ws_config.max_frame_size = Some(16 << 20);   // 16MB

        debug!("Connecting to {}", config.url);

        let (stream, response) = connect_async_with_config(request, Some(ws_config), false)
            .await
            .map_err(|e| WssClientError::ConnectionFailed(e.to_string()))?;

        info!(
            "Connected to WebSocket server, status: {}",
            response.status()
        );

        Ok(Self { stream })
    }

    /// Build a Chrome-like HTTP request for WebSocket upgrade
    fn build_request(config: &WssClientConfig) -> Result<Request<()>, WssClientError> {
        let mut request = config
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| WssClientError::InvalidUrl(e.to_string()))?;

        let host = request
            .uri()
            .host()
            .unwrap_or("localhost")
            .to_string();

        let headers = request.headers_mut();

        // Chrome-like headers (order matters for fingerprinting!)
        headers.insert(header::HOST, host.parse().unwrap());
        headers.insert(header::USER_AGENT, CHROME_UA.parse().unwrap());
        headers.insert(
            header::ACCEPT_LANGUAGE,
            "en-US,en;q=0.9".parse().unwrap(),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            "gzip, deflate, br".parse().unwrap(),
        );
        headers.insert(
            "Sec-WebSocket-Extensions",
            "permessage-deflate; client_max_window_bits".parse().unwrap(),
        );

        // Add authorization if token is provided
        if let Some(token) = &config.token {
            headers.insert(
                header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
        }

        // Add custom headers
        for (key, value) in &config.headers {
            if let (Ok(name), Ok(val)) = (
                key.parse::<hyper::header::HeaderName>(),
                value.parse::<hyper::header::HeaderValue>()
            ) {
                headers.insert(name, val);
            }
        }

        Ok(request)
    }

    /// Send initial fake frames to emulate negotiation
    pub async fn send_initial_frames(&mut self) -> Result<(), WssClientError> {
        // Frame 1: Small text frame (like handshake ack)
        self.send_text("ping").await?;

        tokio::time::sleep(std::time::Duration::from_millis(fastrand::u64(10..50))).await;

        // Frame 2-3: Empty padding frames
        for _ in 0..2 {
            let padding: Vec<u8> = (0..fastrand::usize(100..500))
                .map(|_| fastrand::u8(..))
                .collect();
            self.send_binary(&padding).await?;

            tokio::time::sleep(std::time::Duration::from_millis(fastrand::u64(5..20))).await;
        }

        Ok(())
    }

    /// Send a binary message
    pub async fn send_binary(&mut self, data: &[u8]) -> Result<(), WssClientError> {
        trace!("Sending binary frame: {} bytes", data.len());
        self.stream
            .send(Message::Binary(data.to_vec().into()))
            .await
            .map_err(|e| WssClientError::SendFailed(e.to_string()))
    }

    /// Send a text message
    pub async fn send_text(&mut self, text: &str) -> Result<(), WssClientError> {
        trace!("Sending text frame: {}", text);
        self.stream
            .send(Message::Text(text.to_string().into()))
            .await
            .map_err(|e| WssClientError::SendFailed(e.to_string()))
    }

    /// Receive the next message
    pub async fn receive(&mut self) -> Result<Message, WssClientError> {
        match self.stream.next().await {
            Some(Ok(msg)) => {
                trace!("Received message: {:?}", msg);
                Ok(msg)
            }
            Some(Err(e)) => Err(WssClientError::ReceiveFailed(e.to_string())),
            None => Err(WssClientError::ConnectionClosed),
        }
    }

    /// Receive binary data only
    pub async fn receive_binary(&mut self) -> Result<Vec<u8>, WssClientError> {
        loop {
            match self.receive().await? {
                Message::Binary(data) => return Ok(data.to_vec()),
                Message::Ping(data) => {
                    self.stream
                        .send(Message::Pong(data))
                        .await
                        .map_err(|e| WssClientError::SendFailed(e.to_string()))?;
                }
                Message::Close(_) => return Err(WssClientError::ConnectionClosed),
                _ => continue, // Ignore text and other frames
            }
        }
    }

    /// Send ping
    pub async fn ping(&mut self, data: &[u8]) -> Result<(), WssClientError> {
        self.stream
            .send(Message::Ping(data.to_vec().into()))
            .await
            .map_err(|e| WssClientError::SendFailed(e.to_string()))
    }

    /// Close the connection
    pub async fn close(&mut self) -> Result<(), WssClientError> {
        debug!("Closing WebSocket connection");
        self.stream
            .close(None)
            .await
            .map_err(|e| WssClientError::SendFailed(e.to_string()))
    }

    /// Get mutable reference to the underlying stream
    pub fn stream_mut(
        &mut self,
    ) -> &mut WebSocketStream<MaybeTlsStream<TcpStream>> {
        &mut self.stream
    }

    /// Split into read and write halves
    pub fn split(
        self,
    ) -> (
        futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) {
        self.stream.split()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_ua() {
        assert!(CHROME_UA.contains("Chrome/120"));
        assert!(CHROME_UA.contains("Windows NT 10.0"));
    }

    #[test]
    fn test_config_default() {
        let config = WssClientConfig::default();
        assert!(config.compression);
        assert_eq!(config.timeout_secs, 30);
    }
}
