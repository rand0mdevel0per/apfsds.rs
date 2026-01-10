//! Authentication module

use apfsds_crypto::{Ed25519KeyPair, HmacAuthenticator, ReplayCache, UuidReplayCache};
use apfsds_protocol::{AuthRequest, AuthResponse, TokenPayload};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid timestamp: drift {0}ms")]
    InvalidTimestamp(i64),

    #[error("Nonce reused (replay attack)")]
    NonceReused,

    #[error("Invalid HMAC signature")]
    InvalidHmac,

    #[error("Token expired")]
    TokenExpired,

    #[error("Token already used")]
    TokenAlreadyUsed,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Crypto error: {0}")]
    CryptoError(String),
}

/// Authenticator for handling client authentication
pub struct Authenticator {
    /// Server key pair
    keypair: Ed25519KeyPair,

    /// HMAC authenticator
    hmac: HmacAuthenticator,

    /// Nonce replay cache
    nonce_cache: ReplayCache,

    /// Token replay cache
    token_cache: UuidReplayCache,

    /// Maximum timestamp drift (ms)
    max_drift_ms: i64,

    /// Token TTL (ms)
    token_ttl_ms: u64,
}

impl Authenticator {
    /// Create a new authenticator
    pub fn new(server_sk: [u8; 32], hmac_secret: [u8; 32], token_ttl_secs: u64) -> Self {
        Self {
            keypair: Ed25519KeyPair::from_secret(&server_sk),
            hmac: HmacAuthenticator::new(hmac_secret),
            nonce_cache: ReplayCache::new(Duration::from_secs(120)),
            token_cache: UuidReplayCache::new(Duration::from_secs(token_ttl_secs + 60)),
            max_drift_ms: 30_000, // 30 seconds
            token_ttl_ms: token_ttl_secs * 1000,
        }
    }

    /// Get the server public key
    pub fn public_key(&self) -> [u8; 32] {
        self.keypair.public_key()
    }

    /// Verify an authentication request
    pub fn verify(&self, auth: &AuthRequest) -> Result<u64, AuthError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Check timestamp
        let drift = now as i64 - auth.timestamp as i64;
        if drift.abs() > self.max_drift_ms {
            return Err(AuthError::InvalidTimestamp(drift));
        }

        // Check nonce
        if !self.nonce_cache.check_and_insert(&auth.nonce) {
            return Err(AuthError::NonceReused);
        }

        // Verify HMAC
        self.hmac
            .verify_with_timestamp(&auth.hmac_base, auth.timestamp, &auth.hmac_signature)
            .map_err(|_| AuthError::InvalidHmac)?;

        // Extract user_id from hmac_base (format: "user_id:timestamp:random")
        let user_id = extract_user_id(&auth.hmac_base)?;

        debug!("Authenticated user {}", user_id);

        Ok(user_id)
    }

    /// Generate a one-time token
    pub fn generate_token(&self, user_id: u64, nonce: &[u8; 32]) -> Vec<u8> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let payload = TokenPayload {
            user_id,
            nonce: *nonce,
            issued_at: now,
            valid_until: now + self.token_ttl_ms,
        };

        // Serialize
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&payload)
            .expect("serialization should not fail")
            .to_vec();

        // Sign
        let signature = self.keypair.sign(&bytes);

        // Combine
        let mut token = bytes;
        token.extend_from_slice(&signature);

        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &token)
            .into_bytes()
    }

    /// Verify and consume a one-time token
    pub fn verify_and_consume_token(&self, token: &[u8]) -> Result<u64, AuthError> {
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, token)
            .map_err(|_| AuthError::InvalidSignature)?;

        if decoded.len() < 64 {
            return Err(AuthError::InvalidSignature);
        }

        let (payload_bytes, signature) = decoded.split_at(decoded.len() - 64);
        let signature: [u8; 64] = signature.try_into().map_err(|_| AuthError::InvalidSignature)?;

        // Verify signature
        Ed25519KeyPair::verify_with_pk(&self.keypair.public_key(), payload_bytes, &signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        // Deserialize
        let archived = rkyv::access::<apfsds_protocol::ArchivedTokenPayload, rkyv::rancor::Error>(payload_bytes)
            .map_err(|e| AuthError::CryptoError(e.to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Check expiration - convert from rkyv's archived type
        let valid_until: u64 = archived.valid_until.to_native();
        if now > valid_until {
            return Err(AuthError::TokenExpired);
        }

        // Check if already used
        let mut nonce = [0u8; 16];
        nonce.copy_from_slice(&archived.nonce[..16]);
        if !self.token_cache.check_and_insert(&nonce) {
            return Err(AuthError::TokenAlreadyUsed);
        }

        Ok(archived.user_id.to_native())
    }

    /// Run cleanup tasks
    pub fn cleanup(&self) {
        self.nonce_cache.cleanup();
        self.token_cache.cleanup();
    }
}

/// Extract user_id from HMAC base string
fn extract_user_id(hmac_base: &[u8]) -> Result<u64, AuthError> {
    let s = std::str::from_utf8(hmac_base).map_err(|_| AuthError::InvalidHmac)?;

    // Format: "user_id:timestamp:random"
    let parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() {
        return Err(AuthError::InvalidHmac);
    }

    parts[0]
        .parse()
        .map_err(|_| AuthError::InvalidHmac)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_auth() -> Authenticator {
        let server_sk = [42u8; 32];
        let hmac_secret = [43u8; 32];
        Authenticator::new(server_sk, hmac_secret, 60)
    }

    #[test]
    fn test_token_roundtrip() {
        let auth = create_auth();
        let nonce = [1u8; 32];

        let token = auth.generate_token(12345, &nonce);
        let user_id = auth.verify_and_consume_token(&token).unwrap();

        assert_eq!(user_id, 12345);
    }

    #[test]
    fn test_token_reuse() {
        let auth = create_auth();
        let nonce = [1u8; 32];

        let token = auth.generate_token(12345, &nonce);

        // First use should succeed
        assert!(auth.verify_and_consume_token(&token).is_ok());

        // Second use should fail
        assert!(auth.verify_and_consume_token(&token).is_err());
    }
}
