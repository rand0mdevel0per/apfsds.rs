//! APFSDS Protocol - Frame definitions and serialization
//!
//! This crate defines the core data structures for the APFSDS protocol:
//! - `ProxyFrame`: The fundamental unit of data transmission
//! - `AuthRequest`/`AuthResponse`: Authentication handshake
//! - `TokenPayload`: One-time connection tokens
//!
//! All structures use rkyv for zero-copy deserialization.

mod frame;
mod auth;
mod validation;

pub use frame::*;
pub use auth::*;
pub use validation::*;
