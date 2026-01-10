//! APFSDS Transport - WebSocket and network layer
//!
//! This crate provides:
//! - WebSocket client with Chrome handshake emulation
//! - WebSocket server
//! - Connection pool (round-robin)
//! - Noise traffic generation

mod wss_client;
mod wss_server;
mod pool;
mod noise;
mod frame_codec;

pub use wss_client::*;
pub use wss_server::*;
pub use pool::*;
pub use noise::*;
pub use frame_codec::*;
