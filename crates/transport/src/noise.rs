//! Noise traffic generation for obfuscation

use serde_json::json;
use std::time::Duration;
use tracing::trace;

/// Generate a fake SSE keepalive message
pub fn generate_sse_keepalive() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    format!(
        "data: {{\"type\":\"ping\",\"timestamp\":{}}}\n\n",
        timestamp
    )
}

/// Generate a fake JSON API response (mimics chat completion)
pub fn generate_fake_chat_response() -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": timestamp,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": null
        }]
    })
    .to_string()
}

/// Generate random binary noise
pub fn generate_binary_noise(size: usize) -> Vec<u8> {
    (0..size).map(|_| fastrand::u8(..)).collect()
}

/// Noise traffic configuration
#[derive(Debug, Clone)]
pub struct NoiseConfig {
    /// Enable SSE keepalive
    pub sse_enabled: bool,

    /// SSE interval in seconds
    pub sse_interval: (u64, u64),

    /// Enable fake JSON responses
    pub fake_json_enabled: bool,

    /// Fake JSON interval in seconds
    pub fake_json_interval: (u64, u64),

    /// Ratio of noise to real traffic (0.0 - 1.0)
    pub noise_ratio: f32,
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            sse_enabled: true,
            sse_interval: (10, 30),
            fake_json_enabled: true,
            fake_json_interval: (30, 120),
            noise_ratio: 0.15,
        }
    }
}

impl NoiseConfig {
    /// Generate random SSE interval
    pub fn random_sse_interval(&self) -> Duration {
        let (min, max) = self.sse_interval;
        Duration::from_secs(fastrand::u64(min..=max))
    }

    /// Generate random fake JSON interval
    pub fn random_fake_json_interval(&self) -> Duration {
        let (min, max) = self.fake_json_interval;
        Duration::from_secs(fastrand::u64(min..=max))
    }

    /// Should we send noise for this packet?
    pub fn should_send_noise(&self) -> bool {
        fastrand::f32() < self.noise_ratio
    }
}

/// Noise generator that produces periodic noise traffic
pub struct NoiseGenerator {
    config: NoiseConfig,
}

impl NoiseGenerator {
    pub fn new(config: NoiseConfig) -> Self {
        Self { config }
    }

    /// Get the next noise message (if any)
    pub fn next_noise(&self) -> Option<NoiseMessage> {
        if !self.config.should_send_noise() {
            return None;
        }

        // Randomly choose noise type
        let noise_type = fastrand::u8(0..3);
        match noise_type {
            0 if self.config.sse_enabled => Some(NoiseMessage::Text(generate_sse_keepalive())),
            1 if self.config.fake_json_enabled => {
                Some(NoiseMessage::Text(generate_fake_chat_response()))
            }
            _ => {
                let size = fastrand::usize(100..500);
                Some(NoiseMessage::Binary(generate_binary_noise(size)))
            }
        }
    }
}

/// Type of noise message
pub enum NoiseMessage {
    Text(String),
    Binary(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_keepalive() {
        let msg = generate_sse_keepalive();
        assert!(msg.starts_with("data: "));
        assert!(msg.ends_with("\n\n"));
        assert!(msg.contains("\"type\":\"ping\""));
    }

    #[test]
    fn test_fake_chat_response() {
        let msg = generate_fake_chat_response();
        assert!(msg.contains("chat.completion.chunk"));
        assert!(msg.contains("gpt-4"));
    }

    #[test]
    fn test_binary_noise() {
        let noise = generate_binary_noise(100);
        assert_eq!(noise.len(), 100);
    }

    #[test]
    fn test_noise_config() {
        let config = NoiseConfig::default();
        assert!(config.sse_enabled);
        assert_eq!(config.noise_ratio, 0.15);
    }
}
