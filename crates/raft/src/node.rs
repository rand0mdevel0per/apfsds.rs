use crate::network::Network;
use crate::storage::PersistentStorage;
use crate::{ClientRequest, ClientResponse, NodeId};
use async_raft::Config;
use std::sync::Arc;
use tokio::sync::RwLock;

/// APFSDS Raft Type
pub type ApfsdsRaft = async_raft::Raft<ClientRequest, ClientResponse, Network, PersistentStorage>;

use apfsds_storage::ClickHouseConfig;

/// Raft Node Wrapper
#[derive(Clone)]
pub struct RaftNode {
    pub node_id: NodeId,
    pub raft: Arc<ApfsdsRaft>,
    pub storage: Arc<PersistentStorage>,
    pub network: Arc<Network>,
    pub peers: Arc<RwLock<std::collections::HashMap<NodeId, String>>>,
}

impl RaftNode {
    /// Create a new Raft node
    pub fn new(node_id: NodeId, config: Arc<Config>) -> Self {
        let peers = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let network = Arc::new(Network::new(peers.clone()));
        // For Phase 3, we default to a data directory in current working dir
        let data_dir = std::env::current_dir().unwrap().join("data");
        // TODO: Pass actual config from daemon
        let ch_config = ClickHouseConfig::default();
        let storage = Arc::new(
            PersistentStorage::new(node_id, data_dir, ch_config).expect("Failed to create storage"),
        );

        let raft = Arc::new(ApfsdsRaft::new(
            node_id,
            config,
            network.clone(),
            storage.clone(),
        ));

        Self {
            node_id,
            raft,
            storage,
            network,
            peers,
        }
    }

    pub async fn add_peer(&self, id: NodeId, addr: String) {
        self.peers.write().await.insert(id, addr);
    }

    /// Change cluster membership
    pub async fn change_membership(&self, members: std::collections::HashSet<NodeId>) -> anyhow::Result<()> {
        self.raft.change_membership(members).await.map_err(|e| anyhow::anyhow!("Raft membership error: {:?}", e))
    }

    /// Get Raft metrics
    pub async fn get_metrics(&self) -> async_raft::RaftMetrics {
        self.raft.metrics().borrow().clone()
    }
}
