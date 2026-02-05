//! WebSocket client with Chrome handshake emulation

use futures::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{Request, header},
        protocol::WebSocketConfig,
    },
};
use tracing::{debug, info, trace};

/// Chrome 120 User-Agent
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Browser profile for fingerprint emulation
#[derive(Debug, Clone)]
pub struct BrowserProfile {
    pub user_agent: String,
    pub accept_language: String,
    pub accept_encoding: String,
    pub sec_fetch_site: String,
    pub sec_fetch_mode: String,
    pub sec_fetch_dest: String,
    pub plugin_headers: Vec<(String, String)>,
}

impl Default for BrowserProfile {
    fn default() -> Self {
        Self::chrome_120_windows()
    }
}

impl BrowserProfile {
    /// Chrome 120 on Windows (default)
    pub fn chrome_120_windows() -> Self {
        Self {
            user_agent: CHROME_UA.to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            accept_encoding: "gzip, deflate, br".to_string(),
            sec_fetch_site: "cross-site".to_string(),
            sec_fetch_mode: "websocket".to_string(),
            sec_fetch_dest: "empty".to_string(),
            plugin_headers: Vec::new(),
        }
    }

    /// Chrome 120 with AdBlock Plus
    pub fn chrome_120_with_adblock() -> Self {
        let mut profile = Self::chrome_120_windows();
        profile.plugin_headers.push((
            "X-Client-Data".to_string(),
            "CIW2yQEIprbJAQjBtskBCKmdygEIlaHKAQiVocoBGI6jygE=".to_string(),
        ));
        profile
    }

    /// Chrome 120 on macOS
    pub fn chrome_120_macos() -> Self {
        Self {
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            accept_language: "en-US,en;q=0.9".to_string(),
            accept_encoding: "gzip, deflate, br".to_string(),
            sec_fetch_site: "cross-site".to_string(),
            sec_fetch_mode: "websocket".to_string(),
            sec_fetch_dest: "empty".to_string(),
            plugin_headers: Vec::new(),
        }
    }
}

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

    /// Browser profile for fingerprint emulation
    pub browser_profile: BrowserProfile,

    /// Cookies to send
    pub cookies: Vec<(String, String)>,

    /// Origin header (auto-generated from URL if None)
    pub origin: Option<String>,

    /// Referer header
    pub referer: Option<String>,

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
            browser_profile: BrowserProfile::default(),
            cookies: Vec::new(),
            origin: None,
            referer: None,
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
        ws_config.max_frame_size = Some(16 << 20); // 16MB

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

        let host = request.uri().host().unwrap_or("localhost").to_string();
        let uri = request.uri().clone();

        // Generate Origin if not provided
        let origin = config.origin.clone().unwrap_or_else(|| {
            let scheme = uri.scheme_str().unwrap_or("wss");
            let origin_scheme = if scheme == "wss" { "https" } else { "http" };
            format!("{}://{}", origin_scheme, host)
        });

        let headers = request.headers_mut();

        // Chrome-like headers in correct order (order matters for fingerprinting!)

        // 1. Host (already set by into_client_request, but ensure it's correct)
        headers.insert(header::HOST, host.parse().unwrap());

        // 2. Connection: Upgrade (set by WebSocket library)

        // 3. Pragma & Cache-Control
        headers.insert(header::PRAGMA, "no-cache".parse().unwrap());
        headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());

        // 4. User-Agent (from browser profile)
        headers.insert(
            header::USER_AGENT,
            config.browser_profile.user_agent.parse().unwrap(),
        );

        // 5. Upgrade: websocket (set by WebSocket library)

        // 6. Origin (critical for Chrome!)
        headers.insert(header::ORIGIN, origin.parse().unwrap());

        // 7. Sec-WebSocket-Version (set by WebSocket library)

        // 8. Accept-Encoding (from browser profile)
        headers.insert(
            header::ACCEPT_ENCODING,
            config.browser_profile.accept_encoding.parse().unwrap(),
        );

        // 9. Accept-Language (from browser profile)
        headers.insert(
            header::ACCEPT_LANGUAGE,
            config.browser_profile.accept_language.parse().unwrap(),
        );

        // 10. Cookie (if provided)
        if !config.cookies.is_empty() {
            let cookie_str = config
                .cookies
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("; ");
            headers.insert(header::COOKIE, cookie_str.parse().unwrap());
        }

        // 11. Sec-WebSocket-Key (set by WebSocket library)

        // 12. Sec-WebSocket-Extensions
        headers.insert(
            "Sec-WebSocket-Extensions",
            "permessage-deflate; client_max_window_bits"
                .parse()
                .unwrap(),
        );

        // 13. Sec-Fetch-* headers (Chrome 85+)
        headers.insert(
            "Sec-Fetch-Site",
            config.browser_profile.sec_fetch_site.parse().unwrap(),
        );
        headers.insert(
            "Sec-Fetch-Mode",
            config.browser_profile.sec_fetch_mode.parse().unwrap(),
        );
        headers.insert(
            "Sec-Fetch-Dest",
            config.browser_profile.sec_fetch_dest.parse().unwrap(),
        );

        // 14. Referer (if provided)
        if let Some(referer) = &config.referer {
            headers.insert(header::REFERER, referer.parse().unwrap());
        }

        // 15. Plugin headers (e.g., X-Client-Data for Chrome with sync enabled)
        for (key, value) in &config.browser_profile.plugin_headers {
            if let (Ok(name), Ok(val)) = (
                key.parse::<hyper::header::HeaderName>(),
                value.parse::<hyper::header::HeaderValue>(),
            ) {
                headers.insert(name, val);
            }
        }

        // 16. Authorization (if token is provided)
        if let Some(token) = &config.token {
            headers.insert(
                header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
        }

        // 17. Custom headers (last, to allow overrides)
        for (key, value) in &config.headers {
            if let (Ok(name), Ok(val)) = (
                key.parse::<hyper::header::HeaderName>(),
                value.parse::<hyper::header::HeaderValue>(),
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
    pub fn stream_mut(&mut self) -> &mut WebSocketStream<MaybeTlsStream<TcpStream>> {
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
