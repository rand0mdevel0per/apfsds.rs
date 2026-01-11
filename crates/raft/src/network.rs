use crate::{ClientRequest, ClientResponse, NodeId};
use anyhow::{Result, anyhow};
use async_raft::RaftNetwork;
use async_raft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Network implementation for async-raft
pub struct Network {
    client: Client,
    peers: Arc<RwLock<HashMap<NodeId, String>>>,
}

impl Network {
    pub fn new(peers: Arc<RwLock<HashMap<NodeId, String>>>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
            peers,
        }
    }

    async fn get_peer_url(&self, target: NodeId) -> Result<String> {
        self.peers
            .read()
            .await
            .get(&target)
            .cloned()
            .ok_or_else(|| anyhow!("Peer {} not found", target))
    }

    async fn post<Req, Resp>(&self, target: NodeId, path: &str, req: Req) -> Result<Resp>
    where
        Req: serde::Serialize,
        Resp: serde::de::DeserializeOwned,
    {
        let url = format!("http://{}{}", self.get_peer_url(target).await?, path);

        let resp = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;

        if !resp.status().is_success() {
            return Err(anyhow!("Remote error: {}", resp.status()));
        }

        resp.json()
            .await
            .map_err(|e| anyhow!("Serialization error: {}", e))
    }
}

#[async_trait]
impl RaftNetwork<ClientRequest> for Network {
    async fn append_entries(
        &self,
        target: NodeId,
        rpc: AppendEntriesRequest<ClientRequest>,
    ) -> Result<AppendEntriesResponse> {
        self.post(target, "/raft/append", rpc).await
    }

    async fn install_snapshot(
        &self,
        target: NodeId,
        rpc: InstallSnapshotRequest,
    ) -> Result<InstallSnapshotResponse> {
        self.post(target, "/raft/snapshot", rpc).await
    }

    async fn vote(&self, target: NodeId, rpc: VoteRequest) -> Result<VoteResponse> {
        self.post(target, "/raft/vote", rpc).await
    }
}
