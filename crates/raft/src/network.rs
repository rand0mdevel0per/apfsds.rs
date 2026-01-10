//! Raft network implementation

use crate::{NodeId, TypeConfig};
use openraft::error::{InstallSnapshotError, RPCError, RaftError, Unreachable};
use openraft::network::{RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, trace};

/// Network errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),
}

/// HTTP-based Raft network
pub struct RaftHttpNetwork {
    target: BasicNode,
    client: reqwest::Client,
}

impl RaftHttpNetwork {
    pub fn new(target: BasicNode) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self { target, client }
    }

    async fn send_rpc<Req, Resp>(&self, path: &str, req: &Req) -> Result<Resp, NetworkError>
    where
        Req: Serialize,
        Resp: for<'de> Deserialize<'de>,
    {
        let url = format!("http://{}{}", self.target.addr, path);
        
        let resp = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(NetworkError::RequestFailed(format!("HTTP {}", resp.status())));
        }

        resp.json()
            .await
            .map_err(|e| NetworkError::RequestFailed(e.to_string()))
    }
}

/// Network factory
pub struct NetworkFactory;

impl NetworkFactory {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftNetworkFactory<TypeConfig> for NetworkFactory {
    type Network = RaftHttpNetwork;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        debug!("Creating network client for node: {:?}", node);
        RaftHttpNetwork::new(node.clone())
    }
}

impl RaftNetwork<TypeConfig> for RaftHttpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: openraft::network::RPCOption,
    ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        trace!("Sending append_entries to {:?}", self.target);

        self.send_rpc("/raft/append", &rpc)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: openraft::network::RPCOption,
    ) -> Result<
        InstallSnapshotResponse<TypeConfig>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        trace!("Sending install_snapshot to {:?}", self.target);

        self.send_rpc("/raft/snapshot", &rpc)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<TypeConfig>,
        _option: openraft::network::RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        trace!("Sending vote request to {:?}", self.target);

        self.send_rpc("/raft/vote", &rpc)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_factory() {
        let _factory = NetworkFactory::new();
    }
}
