//! APFSDS Crypto - Encryption, signing, and key management
//!
//! This crate provides:
//! - Ed25519 key generation and signing (legacy)
//! - ML-DSA-65 (Dilithium3) post-quantum signatures
//! - X25519 ECDH key exchange (legacy)
//! - ML-KEM-768 (Kyber) post-quantum key exchange
//! - AES-256-GCM encryption/decryption
//! - HMAC-SHA256 with constant-time comparison
//! - Replay cache for nonce deduplication

mod aes;
mod hmac_auth;
mod keys;
mod replay;

pub use aes::*;
pub use hmac_auth::*;
pub use keys::*;
pub use replay::*;
