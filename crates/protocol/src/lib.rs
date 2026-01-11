//! APFSDS Protocol - Frame definitions and serialization
//!
//! This crate defines the core data structures for the APFSDS protocol:
//! - `ProxyFrame`: The fundamental unit of data transmission
//! - `AuthRequest`/`AuthResponse`: Authentication handshake
//! - `TokenPayload`: One-time connection tokens
//! - `ControlMessage`: Out-of-band control messages
//!
//! All structures use rkyv for zero-copy deserialization.

mod auth;
mod frame;
mod validation;

pub use auth::*;
pub use frame::*;
pub use validation::*;
