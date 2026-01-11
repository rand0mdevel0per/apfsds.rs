//! Key rotation management
//!
//! Handles scheduled and forced key rotation with grace periods.

use apfsds_crypto::Ed25519KeyPair;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::info;

/// Key rotation configuration
#[derive(Debug, Clone)]
pub struct KeyRotationConfig {
    /// Rotation interval (default: 7 days)
    pub rotation_interval: Duration,
    /// Grace period for old keys (default: 10 minutes)
    pub grace_period: Duration,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            rotation_interval: Duration::from_secs(604800), // 7 days
            grace_period: Duration::from_secs(600),         // 10 minutes
        }
    }
}

/// Key pair with metadata
struct KeyEntry {
    keypair: Ed25519KeyPair,
    created_at: Instant,
    expires_at: Option<Instant>,
}

/// Key manager for handling rotation
pub struct KeyManager {
    /// Current active key
    current: RwLock<KeyEntry>,
    /// Previous key (during grace period)
    previous: RwLock<Option<KeyEntry>>,
    /// Configuration
    config: KeyRotationConfig,
    /// Force rotation flag
    force_rotation: AtomicBool,
}

impl KeyManager {
    /// Create a new key manager with generated key
    pub fn new(config: KeyRotationConfig) -> Self {
        let keypair = Ed25519KeyPair::generate();
        Self {
            current: RwLock::new(KeyEntry {
                keypair,
                created_at: Instant::now(),
                expires_at: None,
            }),
            previous: RwLock::new(None),
            config,
            force_rotation: AtomicBool::new(false),
        }
    }

    /// Create with existing secret key
    pub fn with_secret(secret: [u8; 32], config: KeyRotationConfig) -> Self {
        let keypair = Ed25519KeyPair::from_secret(&secret);
        Self {
            current: RwLock::new(KeyEntry {
                keypair,
                created_at: Instant::now(),
                expires_at: None,
            }),
            previous: RwLock::new(None),
            config,
            force_rotation: AtomicBool::new(false),
        }
    }

    /// Get the current public key
    pub fn public_key(&self) -> [u8; 32] {
        self.current.read().unwrap().keypair.public_key()
    }

    /// Sign a message with the current key
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.current.read().unwrap().keypair.sign(message)
    }

    /// Verify a signature (checks current and previous keys)
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        // Try current key
        let current = self.current.read().unwrap();
        if Ed25519KeyPair::verify_with_pk(&current.keypair.public_key(), message, signature).is_ok()
        {
            return true;
        }

        // Try previous key if in grace period
        if let Some(prev) = self.previous.read().unwrap().as_ref() {
            if prev.expires_at.map(|e| Instant::now() < e).unwrap_or(false) {
                if Ed25519KeyPair::verify_with_pk(&prev.keypair.public_key(), message, signature)
                    .is_ok()
                {
                    return true;
                }
            }
        }

        false
    }

    /// Check if rotation is needed
    pub fn should_rotate(&self) -> bool {
        if self.force_rotation.load(Ordering::Relaxed) {
            return true;
        }

        let current = self.current.read().unwrap();
        current.created_at.elapsed() >= self.config.rotation_interval
    }

    /// Trigger forced rotation
    pub fn force_rotate(&self) {
        self.force_rotation.store(true, Ordering::Relaxed);
    }

    /// Perform key rotation
    ///
    /// Returns the new public key
    pub fn rotate(&self) -> [u8; 32] {
        info!("Performing key rotation");

        // Move current to previous with grace period
        let mut current = self.current.write().unwrap();
        let mut previous = self.previous.write().unwrap();

        let old_entry = KeyEntry {
            keypair: Ed25519KeyPair::from_secret(&current.keypair.secret_key()),
            created_at: current.created_at,
            expires_at: Some(Instant::now() + self.config.grace_period),
        };

        // Generate new key
        let new_keypair = Ed25519KeyPair::generate();
        let new_pk = new_keypair.public_key();

        *current = KeyEntry {
            keypair: new_keypair,
            created_at: Instant::now(),
            expires_at: None,
        };

        *previous = Some(old_entry);

        self.force_rotation.store(false, Ordering::Relaxed);

        info!("Key rotation complete, new PK: {:?}", &new_pk[..8]);
        new_pk
    }

    /// Cleanup expired previous key
    pub fn cleanup(&self) {
        let mut previous = self.previous.write().unwrap();
        if let Some(prev) = previous.as_ref() {
            if prev.expires_at.map(|e| Instant::now() >= e).unwrap_or(true) {
                info!("Cleaning up expired previous key");
                *previous = None;
            }
        }
    }

    /// Get rotation status
    pub fn status(&self) -> KeyRotationStatus {
        let current = self.current.read().unwrap();
        let previous = self.previous.read().unwrap();

        KeyRotationStatus {
            current_pk: current.keypair.public_key(),
            current_age_secs: current.created_at.elapsed().as_secs(),
            next_rotation_secs: self
                .config
                .rotation_interval
                .saturating_sub(current.created_at.elapsed())
                .as_secs(),
            in_grace_period: previous.is_some(),
            grace_remaining_secs: previous
                .as_ref()
                .and_then(|p| p.expires_at)
                .map(|e| e.saturating_duration_since(Instant::now()).as_secs()),
        }
    }
}

/// Key rotation status
#[derive(Debug, Clone)]
pub struct KeyRotationStatus {
    pub current_pk: [u8; 32],
    pub current_age_secs: u64,
    pub next_rotation_secs: u64,
    pub in_grace_period: bool,
    pub grace_remaining_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_rotation() {
        let config = KeyRotationConfig {
            rotation_interval: Duration::from_millis(100),
            grace_period: Duration::from_millis(50),
        };

        let manager = KeyManager::new(config);
        let pk1 = manager.public_key();

        // Sign with current key
        let msg = b"test message";
        let sig = manager.sign(msg);
        assert!(manager.verify(msg, &sig));

        // Rotate
        let pk2 = manager.rotate();
        assert_ne!(pk1, pk2);

        // Old signature should still verify during grace period
        assert!(manager.verify(msg, &sig));

        // New signature
        let sig2 = manager.sign(msg);
        assert!(manager.verify(msg, &sig2));
    }

    #[test]
    fn test_force_rotation() {
        let config = KeyRotationConfig::default();
        let manager = KeyManager::new(config);

        assert!(!manager.should_rotate());

        manager.force_rotate();
        assert!(manager.should_rotate());

        manager.rotate();
        assert!(!manager.should_rotate());
    }
}
