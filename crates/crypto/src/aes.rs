//! AES-256-GCM encryption

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AesError {
    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed")]
    DecryptionFailed,

    #[error("Invalid key length: expected 32, got {0}")]
    InvalidKeyLength(usize),

    #[error("Invalid nonce length: expected 12, got {0}")]
    InvalidNonceLength(usize),

    #[error("Ciphertext too short")]
    CiphertextTooShort,
}

/// AES-256-GCM cipher wrapper
pub struct Aes256GcmCipher {
    cipher: Aes256Gcm,
}

impl Aes256GcmCipher {
    /// Create a new cipher from a 32-byte key
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(key).expect("key length is 32");
        Self { cipher }
    }

    /// Encrypt data with a random nonce
    /// Returns: nonce (12 bytes) || ciphertext
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, AesError> {
        use rand::RngCore;
        
        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| AesError::EncryptionFailed)?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Encrypt with a specific nonce
    pub fn encrypt_with_nonce(&self, nonce: &[u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, AesError> {
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| AesError::EncryptionFailed)
    }

    /// Decrypt data (expects nonce || ciphertext)
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, AesError> {
        if data.len() < 12 {
            return Err(AesError::CiphertextTooShort);
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AesError::DecryptionFailed)
    }

    /// Decrypt with a specific nonce
    pub fn decrypt_with_nonce(&self, nonce: &[u8; 12], ciphertext: &[u8]) -> Result<Vec<u8>, AesError> {
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AesError::DecryptionFailed)
    }
}

/// Derive AES key from X25519 shared secret using SHA256
pub fn derive_aes_key(shared_secret: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"APFSDS-AES-KEY-DERIVE");
    hasher.update(shared_secret);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32];
        let cipher = Aes256GcmCipher::new(&key);

        let plaintext = b"Hello, APFSDS!";
        let encrypted = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = [0u8; 32];
        let key2 = [1u8; 32];

        let cipher1 = Aes256GcmCipher::new(&key1);
        let cipher2 = Aes256GcmCipher::new(&key2);

        let encrypted = cipher1.encrypt(b"secret").unwrap();
        let result = cipher2.decrypt(&encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_key_derivation() {
        let shared_secret = [42u8; 32];
        let key1 = derive_aes_key(&shared_secret);
        let key2 = derive_aes_key(&shared_secret);

        assert_eq!(key1, key2);

        // Different shared secret should produce different key
        let other_secret = [43u8; 32];
        let key3 = derive_aes_key(&other_secret);
        assert_ne!(key1, key3);
    }
}
