//! Noise traffic generation
//!
//! Generates fake traffic to blend with normal web activity.

use apfsds_obfuscation::TimingConfig;
use bytes::Bytes;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use tracing::{debug, trace};

/// Noise generator configuration
#[derive(Debug, Clone)]
pub struct NoiseConfig {
    /// Enable noise generation
    pub enabled: bool,
    /// Ratio of noise to real traffic (0.0 - 1.0)
    pub noise_ratio: f32,
    /// Timing configuration
    pub timing: TimingConfig,
    /// Generate fake JSON responses
    pub fake_json_enabled: bool,
    /// Generate SSE keepalive events
    pub sse_keepalive: bool,
}

impl Default for NoiseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            noise_ratio: 0.15,
            timing: TimingConfig::default(),
            fake_json_enabled: true,
            sse_keepalive: true,
        }
    }
}

/// Noise generator
pub struct NoiseGenerator {
    config: NoiseConfig,
    running: Arc<AtomicBool>,
}

impl NoiseGenerator {
    /// Create a new noise generator
    pub fn new(config: NoiseConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start generating noise and send to the provided channel
    pub fn start(&self, tx: mpsc::UnboundedSender<Bytes>) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let running = self.running.clone();
        running.store(true, Ordering::Relaxed);

        tokio::spawn(async move {
            debug!("Noise generator started");

            while running.load(Ordering::Relaxed) {
                // Wait for noise interval
                let interval = config.timing.random_noise_interval();
                tokio::time::sleep(interval).await;

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                // Generate noise
                let noise = if config.fake_json_enabled && fastrand::f32() < 0.7 {
                    generate_fake_json()
                } else if config.sse_keepalive {
                    generate_sse_event()
                } else {
                    generate_random_data()
                };

                trace!("Sending noise ({} bytes)", noise.len());

                if tx.send(Bytes::from(noise)).is_err() {
                    debug!("Noise channel closed, stopping generator");
                    break;
                }
            }

            debug!("Noise generator stopped");
        })
    }

    /// Stop the noise generator
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if should inject noise based on ratio
    pub fn should_inject(&self) -> bool {
        self.config.enabled && fastrand::f32() < self.config.noise_ratio
    }
}

/// Generate fake JSON API response
fn generate_fake_json() -> Vec<u8> {
    let templates = [
        r#"{"status":"ok","timestamp":%TS%,"data":{"items":[]}}"#,
        r#"{"success":true,"message":"Operation completed","id":"%UUID%"}"#,
        r#"{"results":[],"page":1,"total":0,"cached":true}"#,
        r#"{"health":"healthy","uptime":%TS%,"version":"1.0.0"}"#,
        r#"{"ack":true,"seq":%SEQ%,"received":true}"#,
        r#"{"type":"ping","ts":%TS%}"#,
        r#"{"notifications":[],"unread":0}"#,
    ];

    let template = templates[fastrand::usize(..templates.len())];

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let uuid = uuid::Uuid::new_v4();
    let seq = fastrand::u32(1..100000);

    template
        .replace("%TS%", &now.to_string())
        .replace("%UUID%", &uuid.to_string())
        .replace("%SEQ%", &seq.to_string())
        .into_bytes()
}

/// Generate SSE (Server-Sent Events) keepalive
fn generate_sse_event() -> Vec<u8> {
    let events = [
        "event: heartbeat\ndata: {\"ts\":%TS%}\n\n",
        ": keepalive\n\n",
        "event: ping\ndata: ok\n\n",
        "event: status\ndata: {\"connected\":true}\n\n",
    ];

    let event = events[fastrand::usize(..events.len())];

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    event.replace("%TS%", &now.to_string()).into_bytes()
}

/// Generate random binary data
fn generate_random_data() -> Vec<u8> {
    let len = fastrand::usize(64..512);
    (0..len).map(|_| fastrand::u8(..)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fake_json() {
        let json = generate_fake_json();
        assert!(!json.is_empty());

        // Should be valid UTF-8
        let s = String::from_utf8(json).unwrap();
        assert!(s.starts_with('{') || s.starts_with('['));
    }

    #[test]
    fn test_sse_event() {
        let event = generate_sse_event();
        let s = String::from_utf8(event).unwrap();
        assert!(s.ends_with("\n\n") || s.ends_with("\n"));
    }

    #[test]
    fn test_should_inject() {
        let mut config = NoiseConfig::default();
        config.noise_ratio = 1.0; // Always inject

        let generator = NoiseGenerator::new(config);
        assert!(generator.should_inject());

        let mut config2 = NoiseConfig::default();
        config2.enabled = false;

        let gen2 = NoiseGenerator::new(config2);
        assert!(!gen2.should_inject());
    }
}
