//! Replay protection cache

use dashmap::DashMap;
use std::time::{Duration, Instant};

/// Thread-safe replay cache for nonce/UUID deduplication
pub struct ReplayCache {
    /// Map of nonce -> expiration time
    seen: DashMap<[u8; 32], Instant>,
    /// TTL for entries
    ttl: Duration,
}

impl ReplayCache {
    /// Create a new replay cache with the given TTL
    pub fn new(ttl: Duration) -> Self {
        Self {
            seen: DashMap::new(),
            ttl,
        }
    }

    /// Check if a nonce has been seen and insert it if not
    /// Returns true if the nonce is new (not a replay)
    pub fn check_and_insert(&self, nonce: &[u8; 32]) -> bool {
        let now = Instant::now();
        let expiry = now + self.ttl;

        // Check if already exists and not expired
        if let Some(existing) = self.seen.get(nonce) {
            if *existing > now {
                return false; // Replay detected
            }
        }

        // Insert or update
        self.seen.insert(*nonce, expiry);
        true
    }

    /// Check if a nonce has been seen (without inserting)
    pub fn contains(&self, nonce: &[u8; 32]) -> bool {
        if let Some(expiry) = self.seen.get(nonce) {
            *expiry > Instant::now()
        } else {
            false
        }
    }

    /// Remove expired entries
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.seen.retain(|_, expiry| *expiry > now);
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.seen.clear();
    }
}

/// UUID-based replay cache (16-byte keys)
pub struct UuidReplayCache {
    seen: DashMap<[u8; 16], Instant>,
    ttl: Duration,
}

impl UuidReplayCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            seen: DashMap::new(),
            ttl,
        }
    }

    pub fn check_and_insert(&self, uuid: &[u8; 16]) -> bool {
        let now = Instant::now();
        let expiry = now + self.ttl;

        if let Some(existing) = self.seen.get(uuid) {
            if *existing > now {
                return false;
            }
        }

        self.seen.insert(*uuid, expiry);
        true
    }

    pub fn cleanup(&self) {
        let now = Instant::now();
        self.seen.retain(|_, expiry| *expiry > now);
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_detection() {
        let cache = ReplayCache::new(Duration::from_secs(60));
        let nonce = [42u8; 32];

        // First time should succeed
        assert!(cache.check_and_insert(&nonce));

        // Second time should fail (replay)
        assert!(!cache.check_and_insert(&nonce));
    }

    #[test]
    fn test_different_nonces() {
        let cache = ReplayCache::new(Duration::from_secs(60));
        let nonce1 = [1u8; 32];
        let nonce2 = [2u8; 32];

        assert!(cache.check_and_insert(&nonce1));
        assert!(cache.check_and_insert(&nonce2));
    }

    #[test]
    fn test_cleanup() {
        let cache = ReplayCache::new(Duration::from_millis(10));
        let nonce = [42u8; 32];

        cache.check_and_insert(&nonce);
        assert_eq!(cache.len(), 1);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));

        cache.cleanup();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_uuid_cache() {
        let cache = UuidReplayCache::new(Duration::from_secs(60));
        let uuid = [42u8; 16];

        assert!(cache.check_and_insert(&uuid));
        assert!(!cache.check_and_insert(&uuid));
    }
}
