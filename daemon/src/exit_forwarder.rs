//! Exit Node Forwarder
//!
//! Forwards ProxyFrame data to exit nodes via HTTP/2 and handles responses.

use apfsds_protocol::{PlainPacket, ProxyFrame};
use apfsds_transport::{ExitClientError, ExitPool};
use std::sync::Arc;
use tracing::{debug, error};

/// Exit forwarder handles packet routing to exit nodes
pub struct ExitForwarder {
    pool: Arc<ExitPool>,
    node_id: u64,
}

impl ExitForwarder {
    pub fn new(pool: Arc<ExitPool>, node_id: u64) -> Self {
        Self { pool, node_id }
    }

    /// Forward a frame to an exit node
    pub async fn forward(&self, frame: &ProxyFrame, group_id: i32) -> Result<(), ExitClientError> {
        // Only forward DATA frames (not control frames)
        if frame.flags.is_control {
            return Ok(());
        }

        // Convert ProxyFrame to PlainPacket
        // Note: In a real implementation, we would need mapping from conn_id to remote endpoint.
        // For Phase 2, we assume the conn_id is sufficient or encoded in metadata.

        let packet = PlainPacket::from_frame(frame, self.node_id);

        if let Err(e) = self.pool.forward(&packet, group_id).await {
            error!("Failed to forward packet for conn {}: {}", frame.conn_id, e);
            return Err(e);
        }

        debug!("Forwarded frame for conn {}", frame.conn_id);
        Ok(())
    }
}
