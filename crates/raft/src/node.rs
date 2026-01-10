//! Raft node wrapper

use crate::{
    default_raft_config, LogStorage, NetworkFactory, NodeId, Request, Response,
    StateMachine, TypeConfig,
};
use apfsds_storage::StorageEngine;
use openraft::{BasicNode, Config, Raft};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

/// Raft node errors
#[derive(Error, Debug)]
pub enum RaftNodeError {
    #[error("Raft error: {0}")]
    RaftError(String),

    #[error("Not initialized")]
    NotInitialized,

    #[error("Not leader")]
    NotLeader,
}

/// APFSDS Raft node wrapper
pub struct RaftNode {
    raft: Raft<TypeConfig>,
    pub node_id: NodeId,
    #[allow(dead_code)]
    storage: Arc<StorageEngine>,
}

impl RaftNode {
    /// Create a new Raft node
    pub async fn new(
        node_id: NodeId,
        storage: Arc<StorageEngine>,
        config: Option<Config>,
    ) -> Result<Self, RaftNodeError> {
        let config = Arc::new(config.unwrap_or_else(default_raft_config));

        let log_storage = LogStorage::new();
        let state_machine = StateMachine::new(storage.clone());
        let network = NetworkFactory::new();

        let raft = Raft::new(node_id, config, network, log_storage, state_machine)
            .await
            .map_err(|e| RaftNodeError::RaftError(e.to_string()))?;

        info!("Raft node {} initialized", node_id);

        Ok(Self {
            raft,
            node_id,
            storage,
        })
    }

    /// Initialize the cluster
    pub async fn initialize_cluster(&self) -> Result<(), RaftNodeError> {
        let mut members = BTreeMap::new();
        members.insert(
            self.node_id,
            BasicNode { addr: format!("127.0.0.1:{}", 25300 + self.node_id) },
        );

        self.raft
            .initialize(members)
            .await
            .map_err(|e| RaftNodeError::RaftError(e.to_string()))?;

        info!("Cluster initialized with node {}", self.node_id);
        Ok(())
    }

    /// Add a peer
    pub async fn add_peer(&self, peer_id: NodeId, addr: String) -> Result<(), RaftNodeError> {
        let node = BasicNode { addr };

        self.raft
            .add_learner(peer_id, node, true)
            .await
            .map_err(|e| RaftNodeError::RaftError(e.to_string()))?;

        info!("Added peer {} to cluster", peer_id);
        Ok(())
    }

    /// Propose a write request
    pub async fn write(&self, request: Request) -> Result<Response, RaftNodeError> {
        let response = self
            .raft
            .client_write(request)
            .await
            .map_err(|e| RaftNodeError::RaftError(e.to_string()))?;

        Ok(response.data)
    }

    /// Check if this node is the leader
    pub async fn is_leader(&self) -> bool {
        self.raft.ensure_linearizable().await.is_ok()
    }

    /// Get the current leader ID
    pub async fn leader_id(&self) -> Option<NodeId> {
        self.raft.current_leader().await
    }

    /// Get cluster metrics
    pub fn metrics(&self) -> openraft::RaftMetrics<TypeConfig> {
        self.raft.metrics().borrow().clone()
    }

    /// Shutdown the Raft node
    pub async fn shutdown(&self) -> Result<(), RaftNodeError> {
        self.raft
            .shutdown()
            .await
            .map_err(|e| RaftNodeError::RaftError(e.to_string()))?;
        info!("Raft node {} shut down", self.node_id);
        Ok(())
    }

    /// Get Raft instance
    pub fn raft(&self) -> &Raft<TypeConfig> {
        &self.raft
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apfsds_storage::StorageConfig;

    #[tokio::test]
    async fn test_raft_node_creation() {
        let storage = Arc::new(StorageEngine::new(StorageConfig::default()));
        let node = RaftNode::new(1, storage, None).await;
        assert!(node.is_ok());
    }
}
