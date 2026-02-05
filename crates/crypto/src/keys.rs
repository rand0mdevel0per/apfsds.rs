//! Ed25519, X25519, and ML-DSA-65 key management

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use ml_dsa::{KeyGen, MlDsa65};
use rand::rngs::OsRng;
use signature::{Signer as SigSigner, Verifier as SigVerifier};
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

    #[error("Key deserialization failed: {0}")]
    KeyDeserializationFailed(String),

    #[error("Key serialization failed: {0}")]
    KeySerializationFailed(String),
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
    keypair: ml_dsa::KeyPair<MlDsa65>,
}

impl MlDsa65KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let keypair = MlDsa65::from_seed(&seed.into());
        Self { keypair }
    }

    /// Create from secret key bytes (32-byte seed)
    pub fn from_secret(secret_bytes: &[u8]) -> Result<Self, KeyError> {
        if secret_bytes.len() != 32 {
            return Err(KeyError::InvalidKeyLength {
                expected: 32,
                actual: secret_bytes.len(),
            });
        }
        let seed: [u8; 32] = secret_bytes.try_into().unwrap();
        let keypair = MlDsa65::from_seed(&seed.into());
        Ok(Self { keypair })
    }

    /// Get the public key bytes
    pub fn public_key(&self) -> Vec<u8> {
        self.keypair.verifying_key().encode().to_vec()
    }

    /// Get the secret key bytes (32-byte seed)
    pub fn secret_key(&self) -> Vec<u8> {
        self.keypair.to_seed().to_vec()
    }

    /// Sign a message (returns detached signature)
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let sig = self.keypair.signing_key().sign(message);
        let encoded = sig.encode();
        <[u8]>::to_vec(encoded.as_ref())
    }

    /// Verify a signature with public key
    pub fn verify_with_pk(
        pk_bytes: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), KeyError> {
        use ml_dsa::{Signature, VerifyingKey};

        // Convert pk_bytes to EncodedVerifyingKey
        let pk_array = pk_bytes
            .try_into()
            .map_err(|_| KeyError::InvalidKeyLength {
                expected: 1952, // ML-DSA-65 public key size
                actual: pk_bytes.len(),
            })?;
        let verifying_key = VerifyingKey::<MlDsa65>::decode(pk_array);

        // Convert signature bytes to EncodedSignature and decode
        let sig_array = signature
            .try_into()
            .map_err(|_| KeyError::InvalidKeyLength {
                expected: 3309, // ML-DSA-65 signature size
                actual: signature.len(),
            })?;
        let sig =
            Signature::<MlDsa65>::decode(sig_array).ok_or(KeyError::InvalidSignatureFormat)?;

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
