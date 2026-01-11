//! APFSDS Transport - WebSocket and network layer
//!
//! This crate provides:
//! - WebSocket client with Chrome handshake emulation
//! - WebSocket server
//! - Connection pool (round-robin)
//! - Noise traffic generation
//! - Exit node communication

mod exit_client;
mod exit_pool;
mod frame_codec;
mod noise;
mod pool;
mod wss_client;
mod wss_server;
mod mtls;
mod quic;
mod ssh;

pub use exit_client::*;
pub use exit_pool::*;
pub use frame_codec::*;
pub use noise::*;
pub use pool::*;
pub use wss_client::*;
pub use wss_server::*;
pub use mtls::*;
pub use quic::*;
pub use ssh::*;

use apfsds_protocol::PlainPacket;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait PacketDispatcher: Send + Sync {
    async fn dispatch(&self, packet: PlainPacket);
}

pub type SharedPacketDispatcher = Arc<dyn PacketDispatcher>;
