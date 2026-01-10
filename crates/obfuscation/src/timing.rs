//! Timing jitter for traffic obfuscation

use std::time::Duration;

/// Default jitter range in milliseconds
pub const DEFAULT_JITTER_MS: u64 = 50;

/// Default inter-frame delay range in microseconds
pub const DEFAULT_INTER_FRAME_DELAY_US: (u64, u64) = (100, 5000);

/// Timing configuration
#[derive(Debug, Clone)]
pub struct TimingConfig {
    /// Maximum jitter to add before sending (ms)
    pub max_jitter_ms: u64,

    /// Inter-frame delay range (us)
    pub inter_frame_delay: (u64, u64),

    /// Reconnect interval range (seconds)
    pub reconnect_interval: (u64, u64),

    /// Noise traffic interval range (seconds)
    pub noise_interval: (u64, u64),
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            max_jitter_ms: DEFAULT_JITTER_MS,
            inter_frame_delay: DEFAULT_INTER_FRAME_DELAY_US,
            reconnect_interval: (60, 180),
            noise_interval: (10, 30),
        }
    }
}

impl TimingConfig {
    /// Generate a random jitter duration
    pub fn random_jitter(&self) -> Duration {
        Duration::from_millis(fastrand::u64(0..=self.max_jitter_ms))
    }

    /// Generate a random inter-frame delay
    pub fn random_inter_frame_delay(&self) -> Duration {
        let (min, max) = self.inter_frame_delay;
        Duration::from_micros(fastrand::u64(min..=max))
    }

    /// Generate a random reconnect interval
    pub fn random_reconnect_interval(&self) -> Duration {
        let (min, max) = self.reconnect_interval;
        Duration::from_secs(fastrand::u64(min..=max))
    }

    /// Generate a random noise interval
    pub fn random_noise_interval(&self) -> Duration {
        let (min, max) = self.noise_interval;
        Duration::from_secs(fastrand::u64(min..=max))
    }
}

/// Async sleep with jitter (requires tokio)
pub async fn sleep_with_jitter(base: Duration, max_jitter_ms: u64) {
    let jitter = Duration::from_millis(fastrand::u64(0..=max_jitter_ms));
    tokio::time::sleep(base + jitter).await;
}

/// Calculate delay based on packet timing to avoid detection
pub fn calculate_adaptive_delay(
    last_packet_time_ms: u64,
    current_time_ms: u64,
    target_rate_bps: u64,
) -> Duration {
    // Calculate expected interval based on target rate
    // (simplified - in practice would consider packet sizes)
    let expected_interval_ms = 1000 / (target_rate_bps / 8 / 1500).max(1);

    let elapsed = current_time_ms.saturating_sub(last_packet_time_ms);

    if elapsed >= expected_interval_ms {
        // We're on time or behind, minimal delay
        Duration::from_millis(fastrand::u64(0..10))
    } else {
        // We're ahead, add delay
        let delay = expected_interval_ms - elapsed;
        Duration::from_millis(delay + fastrand::u64(0..10))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TimingConfig::default();

        assert_eq!(config.max_jitter_ms, 50);
        assert_eq!(config.reconnect_interval, (60, 180));
    }

    #[test]
    fn test_random_jitter() {
        let config = TimingConfig::default();

        for _ in 0..100 {
            let jitter = config.random_jitter();
            assert!(jitter <= Duration::from_millis(50));
        }
    }

    #[test]
    fn test_random_reconnect() {
        let config = TimingConfig::default();

        for _ in 0..100 {
            let interval = config.random_reconnect_interval();
            assert!(interval >= Duration::from_secs(60));
            assert!(interval <= Duration::from_secs(180));
        }
    }

    #[test]
    fn test_adaptive_delay() {
        // We're behind schedule
        let delay = calculate_adaptive_delay(0, 1000, 1_000_000);
        assert!(delay < Duration::from_millis(100));

        // We're ahead of schedule
        let delay = calculate_adaptive_delay(995, 1000, 1_000_000);
        // Should have some delay since we sent recently
    }
}
