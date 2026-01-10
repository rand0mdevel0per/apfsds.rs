//! ProxyFrame - The core data transmission unit

use bytes::Bytes;
use rkyv::{Archive, Deserialize, Serialize};

/// Proxy frame - the fundamental unit of all data transmission
///
/// Each frame represents either:
/// - A data packet (payload contains actual traffic)
/// - A control message (DoH query, keepalive, etc.)
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct ProxyFrame {
    /// Connection ID - unique per logical connection
    pub conn_id: u64,

    /// Remote IP address (16 bytes for IPv6, IPv4 mapped to ::ffff:x.x.x.x)
    pub rip: [u8; 16],

    /// Remote port
    pub rport: u16,

    /// Payload data
    pub payload: Vec<u8>,

    /// Frame UUID - unique per frame (replay protection)
    pub uuid: [u8; 16],

    /// Timestamp in milliseconds since Unix epoch
    pub timestamp: u64,

    /// CRC32 checksum of payload
    pub checksum: u32,

    /// Frame flags
    pub flags: FrameFlags,
}

/// Frame flags for control flow
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct FrameFlags {
    /// This is a control frame (DoH, keepalive, etc.)
    pub is_control: bool,

    /// Payload is zstd compressed
    pub is_compressed: bool,

    /// This is the final frame for this connection
    pub is_final: bool,

    /// Request acknowledgment
    pub needs_ack: bool,

    /// This frame is an acknowledgment
    pub is_ack: bool,
}

impl ProxyFrame {
    /// Create a new data frame
    pub fn new_data(conn_id: u64, rip: [u8; 16], rport: u16, payload: Vec<u8>) -> Self {
        let checksum = crc32fast::hash(&payload);
        let uuid = uuid::Uuid::new_v4().into_bytes();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            conn_id,
            rip,
            rport,
            payload,
            uuid,
            timestamp,
            checksum,
            flags: FrameFlags::default(),
        }
    }

    /// Create a control frame (e.g., DoH query)
    pub fn new_control(payload: Vec<u8>) -> Self {
        let mut frame = Self::new_data(0, [0; 16], 0, payload);
        frame.flags.is_control = true;
        frame
    }

    /// Create a connection close frame
    pub fn new_close(conn_id: u64) -> Self {
        let mut frame = Self::new_data(conn_id, [0; 16], 0, vec![]);
        frame.flags.is_final = true;
        frame
    }

    /// Verify the checksum
    pub fn verify_checksum(&self) -> bool {
        crc32fast::hash(&self.payload) == self.checksum
    }

    /// Convert IPv4 address to mapped IPv6 format
    pub fn ipv4_to_mapped(ipv4: [u8; 4]) -> [u8; 16] {
        let mut mapped = [0u8; 16];
        mapped[10] = 0xff;
        mapped[11] = 0xff;
        mapped[12..16].copy_from_slice(&ipv4);
        mapped
    }

    /// Extract IPv4 from mapped IPv6 format (if applicable)
    pub fn mapped_to_ipv4(mapped: &[u8; 16]) -> Option<[u8; 4]> {
        if mapped[..10] == [0; 10] && mapped[10] == 0xff && mapped[11] == 0xff {
            let mut ipv4 = [0u8; 4];
            ipv4.copy_from_slice(&mapped[12..16]);
            Some(ipv4)
        } else {
            None
        }
    }
}

/// Control frame types
#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum ControlMessage {
    /// DNS over HTTPS query
    DohQuery { query: Vec<u8> },

    /// DNS over HTTPS response
    DohResponse { response: Vec<u8> },

    /// Keepalive ping
    Ping { nonce: u64 },

    /// Keepalive pong
    Pong { nonce: u64 },

    /// Key rotation notification
    KeyRotation {
        new_pk: [u8; 32],
        valid_from: u64,
        valid_until: u64,
    },

    /// Emergency mode warning
    Emergency {
        level: EmergencyLevel,
        trigger_after: u64,
    },
}

/// Emergency level
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub enum EmergencyLevel {
    /// Warning only - client should prepare
    Warning,
    /// Stop all new connections
    Stop,
    /// Immediate shutdown
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let payload = vec![1, 2, 3, 4, 5];
        let frame = ProxyFrame::new_data(
            42,
            ProxyFrame::ipv4_to_mapped([192, 168, 1, 1]),
            8080,
            payload.clone(),
        );

        assert_eq!(frame.conn_id, 42);
        assert_eq!(frame.rport, 8080);
        assert_eq!(frame.payload, payload);
        assert!(frame.verify_checksum());
    }

    #[test]
    fn test_ipv4_mapping() {
        let ipv4 = [192, 168, 1, 1];
        let mapped = ProxyFrame::ipv4_to_mapped(ipv4);
        let extracted = ProxyFrame::mapped_to_ipv4(&mapped);

        assert_eq!(extracted, Some(ipv4));
    }

    #[test]
    fn test_serialization() {
        let frame = ProxyFrame::new_data(
            1,
            [0; 16],
            443,
            vec![0xDE, 0xAD, 0xBE, 0xEF],
        );

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&frame).unwrap();
        let archived = rkyv::access::<ArchivedProxyFrame, rkyv::rancor::Error>(&bytes).unwrap();

        assert_eq!(archived.conn_id, 1);
        assert_eq!(archived.rport, 443);
    }
}
