//! APFSDS Crypto - Encryption, signing, and key management
//!
//! This crate provides:
//! - Ed25519 key generation and signing
//! - X25519 ECDH key exchange
//! - AES-256-GCM encryption/decryption
//! - HMAC-SHA256 with constant-time comparison
//! - Replay cache for nonce deduplication

mod keys;
mod aes;
mod hmac_auth;
mod replay;

pub use keys::*;
pub use aes::*;
pub use hmac_auth::*;
pub use replay::*;
