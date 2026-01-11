use async_raft::{AppData, AppDataResponse};
use serde::{Deserialize, Serialize};

/// Application data request (log entry payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientRequest {
    /// Insert or update connection
    Upsert {
        conn_id: u64,
        client_addr: [u8; 16],
        nat_entry: (u16, u16),
        assigned_pod: u32,
    },

    /// Delete connection
    Delete { conn_id: u64 },

    /// Cleanup expired connections
    Cleanup { before_timestamp: u64 },

    /// No-op
    Noop,
}

impl AppData for ClientRequest {}

/// Application data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientResponse {
    /// Success with affected count
    Ok { affected: u64 },

    /// Error with message
    Error { message: String },
}

impl AppDataResponse for ClientResponse {}
