//! Type definitions for Raft commands and responses

use serde::{Deserialize, Serialize};

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
    /// Success with affected count
    Ok { affected: u64 },
    
    /// Error with message
    Error { message: String },
}
