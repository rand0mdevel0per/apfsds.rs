//! Exit Node Connection Pool
//!
//! Manages reverse connections from exit-nodes without public IP.
//! Exit-nodes connect to handler via WebSocket and register themselves.

use anyhow::Result;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// WebSocket sender type
type WsSender = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<hyper_util::rt::TokioIo<tokio::net::TcpStream>>,
    Message,
>;

/// Exit node connection info
#[derive(Debug)]
pub struct ExitNodeConnection {
    /// Node ID (generated)
    pub node_id: u64,
    /// Node name
    pub name: String,
    /// Group ID
    pub group_id: i32,
    /// WebSocket sender
    pub sender: mpsc::UnboundedSender<Message>,
}

/// Exit Node Pool
pub struct ExitNodePool {
    /// Map of node_id -> connection
    connections: Arc<DashMap<u64, ExitNodeConnection>>,
    /// Next node ID
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl ExitNodePool {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Register a new exit-node connection
    pub fn register(
        &self,
        name: String,
        group_id: i32,
        sender: mpsc::UnboundedSender<Message>,
    ) -> u64 {
        let node_id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let conn = ExitNodeConnection {
            node_id,
            name: name.clone(),
            group_id,
            sender,
        };

        self.connections.insert(node_id, conn);
        info!(
            "Exit-node registered: id={}, name={}, group={}",
            node_id, name, group_id
        );

        node_id
    }

    /// Unregister an exit-node
    pub fn unregister(&self, node_id: u64) {
        if let Some((_, conn)) = self.connections.remove(&node_id) {
            info!("Exit-node unregistered: id={}, name={}", node_id, conn.name);
        }
    }

    /// Get exit-node by ID
    pub fn get(
        &self,
        node_id: u64,
    ) -> Option<dashmap::mapref::one::Ref<'_, u64, ExitNodeConnection>> {
        self.connections.get(&node_id)
    }

    /// Select an exit-node by group_id (simple round-robin)
    pub fn select_by_group(&self, group_id: i32) -> Option<u64> {
        let nodes: Vec<u64> = self
            .connections
            .iter()
            .filter(|entry| entry.value().group_id == group_id)
            .map(|entry| *entry.key())
            .collect();

        if nodes.is_empty() {
            None
        } else {
            // Simple selection: first available
            // TODO: Implement proper load balancing
            Some(nodes[0])
        }
    }

    /// Get all node IDs in a group
    pub fn get_group_nodes(&self, group_id: i32) -> Vec<u64> {
        self.connections
            .iter()
            .filter(|entry| entry.value().group_id == group_id)
            .map(|entry| *entry.key())
            .collect()
    }

    /// Get connection count
    pub fn count(&self) -> usize {
        self.connections.len()
    }
}
