//! Type configuration for openraft 0.9+
//!
//! Uses declare_raft_types! macro with minimal configuration.
//! Default values (per openraft docs):
//! - NodeId = u64
//! - Node = BasicNode  
//! - Entry = Entry<Self>
//! - SnapshotData = Cursor<Vec<u8>>
//! - Responder = OneshotResponder<Self>
//! - AsyncRuntime = TokioRuntime

use openraft::declare_raft_types;
use serde::{Deserialize, Serialize};

/// Node ID type alias (matches openraft default)
pub type NodeId = u64;

/// Request type for Raft log entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    /// Insert or update connection
    Upsert {
        conn_id: u64,
        txid: u64,
        client_addr: [u8; 16],
        nat_entry: (u16, u16),
        assigned_pod: u32,
    },

    /// Delete connection
    Delete { conn_id: u64 },

    /// Cleanup expired connections
    Cleanup { before_timestamp: u64 },

    /// No-op (for leader election)
    Noop,
}

/// Response type for applied entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Response {
    Ok { affected: u64 },
    Error { message: String },
}

// Minimal declare_raft_types! - just specify D and R, use all defaults
declare_raft_types!(
    pub TypeConfig:
        D = Request,
        R = Response
);

/// Create default Raft configuration
pub fn default_raft_config() -> openraft::Config {
    openraft::Config {
        cluster_name: "apfsds".to_string(),
        election_timeout_min: 150,
        election_timeout_max: 300,
        heartbeat_interval: 50,
        install_snapshot_timeout: 200,
        max_payload_entries: 300,
        replication_lag_threshold: 1000,
        snapshot_policy: openraft::SnapshotPolicy::LogsSinceLast(1000),
        snapshot_max_chunk_size: 3 * 1024 * 1024,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = default_raft_config();
        assert_eq!(config.cluster_name, "apfsds");
    }
}
