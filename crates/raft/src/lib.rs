//! APFSDS Raft - Distributed consensus for connection state
//!
//! This crate provides Raft consensus integration with openraft.
//! 
//! NOTE: Full openraft integration is work-in-progress.
//! This version provides a simplified in-memory state sync API.

mod types;

pub use types::*;

use apfsds_storage::StorageEngine;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::info;

/// Raft node errors
#[derive(Error, Debug)]
pub enum RaftNodeError {
    #[error("Not initialized")]
    NotInitialized,

    #[error("Not leader")]
    NotLeader,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Simplified Raft node (single-node mode for Phase 2)
/// 
/// Full distributed Raft will be implemented in Phase 3.
/// This provides the API surface for daemon integration.
pub struct RaftNode {
    node_id: u64,
    storage: Arc<StorageEngine>,
    state: Arc<RwLock<NodeState>>,
}

#[derive(Default)]
struct NodeState {
    is_leader: bool,
    term: u64,
    peers: HashMap<u64, String>,
}

impl RaftNode {
    /// Create a new Raft node (single-node mode)
    pub async fn new(
        node_id: u64,
        storage: Arc<StorageEngine>,
    ) -> Result<Self, RaftNodeError> {
        info!("Creating Raft node {} (single-node mode)", node_id);

        let state = Arc::new(RwLock::new(NodeState {
            is_leader: true, // Single node is always leader
            term: 1,
            peers: HashMap::new(),
        }));

        Ok(Self {
            node_id,
            storage,
            state,
        })
    }

    /// Initialize cluster (single node becomes leader)
    pub async fn initialize_cluster(&self) -> Result<(), RaftNodeError> {
        let mut state = self.state.write().await;
        state.is_leader = true;
        state.term = 1;
        info!("Cluster initialized with node {} as leader", self.node_id);
        Ok(())
    }

    /// Add a peer (stub for future distributed mode)
    pub async fn add_peer(&self, peer_id: u64, addr: String) -> Result<(), RaftNodeError> {
        let mut state = self.state.write().await;
        state.peers.insert(peer_id, addr);
        info!("Added peer {} to cluster", peer_id);
        Ok(())
    }

    /// Write to the state machine
    pub async fn write(&self, request: Request) -> Result<Response, RaftNodeError> {
        let state = self.state.read().await;
        if !state.is_leader {
            return Err(RaftNodeError::NotLeader);
        }

        // Apply directly to storage (single-node mode)
        match request {
            Request::Upsert {
                conn_id,
                client_addr,
                nat_entry,
                assigned_pod,
                ..
            } => {
                let meta = apfsds_protocol::ConnMeta {
                    client_addr,
                    nat_entry,
                    assigned_pod,
                    stream_states: vec![],
                };
                self.storage.upsert(conn_id, meta)
                    .map_err(|e| RaftNodeError::Internal(e.to_string()))?;
                Ok(Response::Ok { affected: 1 })
            }
            Request::Delete { conn_id } => {
                let affected = if self.storage.delete(conn_id).is_some() { 1 } else { 0 };
                Ok(Response::Ok { affected })
            }
            Request::Cleanup { .. } => Ok(Response::Ok { affected: 0 }),
            Request::Noop => Ok(Response::Ok { affected: 0 }),
        }
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.state.read().await.is_leader
    }

    /// Get the current leader ID
    pub async fn leader_id(&self) -> Option<u64> {
        let state = self.state.read().await;
        if state.is_leader {
            Some(self.node_id)
        } else {
            None
        }
    }

    /// Get node ID
    pub fn node_id(&self) -> u64 {
        self.node_id
    }

    /// Shutdown the Raft node
    pub async fn shutdown(&self) -> Result<(), RaftNodeError> {
        info!("Raft node {} shut down", self.node_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apfsds_storage::StorageConfig;

    #[tokio::test]
    async fn test_single_node_raft() {
        let storage = Arc::new(StorageEngine::new(StorageConfig::default()));
        let node = RaftNode::new(1, storage).await.unwrap();
        
        assert!(node.is_leader().await);
        assert_eq!(node.leader_id().await, Some(1));
        
        let resp = node.write(Request::Noop).await.unwrap();
        assert_eq!(resp, Response::Ok { affected: 0 });
    }
}
