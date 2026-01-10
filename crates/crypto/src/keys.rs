//! Ed25519 and X25519 key management

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

#[derive(Error, Debug)]
pub enum KeyError {
    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Invalid signature format")]
    InvalidSignatureFormat,
}

/// Ed25519 key pair for signing
pub struct Ed25519KeyPair {
    signing_key: SigningKey,
}

impl Ed25519KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Create from secret key bytes
    pub fn from_secret(secret: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(secret);
        Self { signing_key }
    }

    /// Get the public key
    pub fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Get the secret key
    pub fn secret_key(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }

    /// Verify a signature (requires only the public key)
    pub fn verify_with_pk(pk: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<(), KeyError> {
        let verifying_key = VerifyingKey::from_bytes(pk)
            .map_err(|_| KeyError::InvalidKeyLength { expected: 32, actual: pk.len() })?;

        let sig = Signature::from_bytes(signature);

        verifying_key
            .verify(message, &sig)
            .map_err(|_| KeyError::SignatureVerificationFailed)
    }
}

/// X25519 key pair for ECDH key exchange
pub struct X25519KeyPair {
    secret: StaticSecret,
    public: X25519PublicKey,
}

impl X25519KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = X25519PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Create from secret key bytes
    pub fn from_secret(secret_bytes: &[u8; 32]) -> Self {
        let secret = StaticSecret::from(*secret_bytes);
        let public = X25519PublicKey::from(&secret);
        Self { secret, public }
    }

    /// Get the public key
    pub fn public_key(&self) -> [u8; 32] {
        self.public.to_bytes()
    }

    /// Perform ECDH to derive a shared secret
    pub fn diffie_hellman(&self, their_public: &[u8; 32]) -> [u8; 32] {
        let their_pk = X25519PublicKey::from(*their_public);
        self.secret.diffie_hellman(&their_pk).to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ed25519_sign_verify() {
        let keypair = Ed25519KeyPair::generate();
        let message = b"Hello, APFSDS!";

        let signature = keypair.sign(message);
        let pk = keypair.public_key();

        assert!(Ed25519KeyPair::verify_with_pk(&pk, message, &signature).is_ok());
    }

    #[test]
    fn test_ed25519_invalid_signature() {
        let keypair = Ed25519KeyPair::generate();
        let message = b"Hello, APFSDS!";
        let wrong_message = b"Wrong message";

        let signature = keypair.sign(message);
        let pk = keypair.public_key();

        assert!(Ed25519KeyPair::verify_with_pk(&pk, wrong_message, &signature).is_err());
    }

    #[test]
    fn test_x25519_key_exchange() {
        let alice = X25519KeyPair::generate();
        let bob = X25519KeyPair::generate();

        let alice_shared = alice.diffie_hellman(&bob.public_key());
        let bob_shared = bob.diffie_hellman(&alice.public_key());

        assert_eq!(alice_shared, bob_shared);
    }

    #[test]
    fn test_key_serialization() {
        let keypair = Ed25519KeyPair::generate();
        let secret = keypair.secret_key();
        let restored = Ed25519KeyPair::from_secret(&secret);

        assert_eq!(keypair.public_key(), restored.public_key());
    }
}
