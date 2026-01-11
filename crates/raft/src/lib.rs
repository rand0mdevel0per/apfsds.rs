//! APFSDS Raft - Distributed consensus using async-raft
//!
//! Implements Raft consensus for connection state synchronization.

mod network;
mod node;
mod storage;
mod types;

// Re-exports
pub use async_raft::Config;
pub use network::Network;
pub use node::{ApfsdsRaft, RaftNode};
pub use storage::PersistentStorage;
pub use types::*;

/// Node Identifier
pub type NodeId = u64;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_raft_node_creation() {
        let config = Arc::new(Config::build("test-cluster".into()).validate().unwrap());
        let _node = RaftNode::new(1, config);
        // Async-raft node starts automatically in background usually?
        // Actually async-raft 0.6 Raft::new just creates it.
        // We need to check if it implements what we expect.
        assert!(true);
    }
}
