use apfsds_protocol::{PlainPacket, ProxyFrame};
use apfsds_transport::PacketDispatcher;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{trace, warn};

/// Registry of active WebSocket connections
pub struct ConnectionRegistry {
    /// Map ConnID -> Sender (ProxyFrame)
    connections: DashMap<u64, UnboundedSender<ProxyFrame>>,
}

impl ConnectionRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connections: DashMap::new(),
        })
    }

    pub fn register(&self, conn_id: u64, sender: UnboundedSender<ProxyFrame>) {
        self.connections.insert(conn_id, sender);
    }

    pub fn unregister(&self, conn_id: u64) {
        self.connections.remove(&conn_id);
    }

    /// Get number of active connections
    pub fn count(&self) -> usize {
        self.connections.len()
    }
}

#[async_trait]
impl PacketDispatcher for ConnectionRegistry {
    async fn dispatch(&self, packet: PlainPacket) {
        if let Some(sender) = self.connections.get(&packet.conn_id) {
            let conn_id = packet.conn_id;
            // Convert PlainPacket -> ProxyFrame (Data)
            let frame =
                ProxyFrame::new_data(packet.conn_id, packet.rip, packet.rport, packet.payload);

            if let Err(e) = sender.send(frame) {
                warn!("Failed to dispatch packet to conn {}: {}", conn_id, e);
            } else {
                trace!("Dispatched return packet to conn {}", conn_id);
            }
        } else {
            // Drop unknown packet or log trace
            // trace!("Packet for unknown conn {}", packet.conn_id);
        }
    }
}
