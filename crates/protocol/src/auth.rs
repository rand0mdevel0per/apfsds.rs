//! Authentication structures

use rkyv::{Archive, Deserialize, Serialize};

/// Authentication request from client
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct AuthRequest {
    /// HMAC base: "user_id:timestamp:random"
    pub hmac_base: Vec<u8>,

    /// HMAC signature
    pub hmac_signature: [u8; 32],

    /// Client's Ed25519 public key (for response encryption)
    pub client_pk: [u8; 32],

    /// Random nonce (replay protection)
    pub nonce: [u8; 32],

    /// Request timestamp (milliseconds)
    pub timestamp: u64,
}

/// Authentication response from server
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct AuthResponse {
    /// One-time connection token
    pub token: Vec<u8>,

    /// Token expiration timestamp
    pub valid_until: u64,

    /// Optional emergency warning
    pub warning: Option<EmergencyWarning>,
}

/// Emergency warning in auth response
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct EmergencyWarning {
    /// Warning level
    pub level: String,

    /// Recommended action
    pub action: String,

    /// When to trigger the action
    pub trigger_after: u64,
}

/// Token payload (signed by server)
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct TokenPayload {
    /// User ID
    pub user_id: u64,

    /// Nonce from auth request
    pub nonce: [u8; 32],

    /// Issue timestamp
    pub issued_at: u64,

    /// Expiration timestamp
    pub valid_until: u64,
}

/// Connection record for MVCC storage
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ConnRecord {
    /// Connection ID
    pub conn_id: u64,

    /// Connection metadata
    pub metadata: ConnMeta,

    /// Creation timestamp
    pub created_at: u64,

    /// Last activity timestamp
    pub last_active: u64,

    /// Access counter
    pub access_count: u32,

    /// MVCC transaction ID
    pub txid: u64,
}

/// Connection metadata
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct ConnMeta {
    /// Client address (IPv6)
    pub client_addr: [u8; 16],

    /// NAT entry (local_port, remote_port)
    pub nat_entry: (u16, u16),

    /// Assigned pod ID
    pub assigned_pod: u32,

    /// Stream states for multiplexing
    pub stream_states: Vec<StreamState>,
}

/// Stream state for multiplexed connections
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(derive(Debug))]
pub struct StreamState {
    /// Stream ID
    pub stream_id: u32,

    /// Bytes sent
    pub bytes_sent: u64,

    /// Bytes received
    pub bytes_received: u64,

    /// Is stream closed
    pub is_closed: bool,
}
