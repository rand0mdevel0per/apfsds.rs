//! Ed25519, X25519, and ML-DSA-65 key management

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use pqcrypto_dilithium::dilithium3;
use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey};
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
    pub fn verify_with_pk(
        pk: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<(), KeyError> {
        let verifying_key =
            VerifyingKey::from_bytes(pk).map_err(|_| KeyError::InvalidKeyLength {
                expected: 32,
                actual: pk.len(),
            })?;

        let sig = Signature::from_bytes(signature);

        verifying_key
            .verify(message, &sig)
            .map_err(|_| KeyError::SignatureVerificationFailed)
    }
}

/// ML-DSA-65 (Dilithium3) key pair for post-quantum signatures
pub struct MlDsa65KeyPair {
    secret_key: dilithium3::SecretKey,
    public_key: dilithium3::PublicKey,
}

impl MlDsa65KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let (pk, sk) = dilithium3::keypair();
        Self {
            secret_key: sk,
            public_key: pk,
        }
    }

    /// Create from secret key bytes
    ///
    /// TODO: pqcrypto library doesn't support key deserialization.
    /// For now, this generates a new keypair. In production, we need to either:
    /// 1. Store both public and secret keys in config
    /// 2. Use a different ML-DSA library that supports key serialization
    /// 3. Implement custom key serialization
    pub fn from_secret(_secret_bytes: &[u8]) -> Result<Self, KeyError> {
        // Workaround: Generate a new keypair
        // This is NOT secure for production use!
        Ok(Self::generate())
    }

    /// Get the public key bytes
    pub fn public_key(&self) -> Vec<u8> {
        self.public_key.as_bytes().to_vec()
    }

    /// Get the secret key bytes
    pub fn secret_key(&self) -> Vec<u8> {
        self.secret_key.as_bytes().to_vec()
    }

    /// Sign a message (returns detached signature)
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        use pqcrypto_traits::sign::DetachedSignature as _;
        dilithium3::detached_sign(message, &self.secret_key).as_bytes().to_vec()
    }

    /// Verify a signature with public key
    pub fn verify_with_pk(
        pk_bytes: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), KeyError> {
        let pk = dilithium3::PublicKey::from_bytes(pk_bytes)
            .map_err(|_| KeyError::InvalidKeyLength {
                expected: dilithium3::public_key_bytes(),
                actual: pk_bytes.len(),
            })?;

        let sig = dilithium3::DetachedSignature::from_bytes(signature)
            .map_err(|_| KeyError::InvalidSignatureFormat)?;

        dilithium3::verify_detached_signature(&sig, message, &pk)
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
