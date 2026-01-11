//! DNS over HTTPS (DoH) resolver via WebSocket
//!
//! Encapsulates DNS queries as control frames and sends them through WSS.
//! Alternative: User can configure system DNS to point to local DNS server.

use apfsds_protocol::ControlMessage;
use std::net::{IpAddr, Ipv4Addr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DohError {
    #[error("DNS query failed: {0}")]
    QueryFailed(String),

    #[error("Invalid response")]
    InvalidResponse,

    #[error("Timeout")]
    Timeout,

    #[error("No results")]
    NoResults,
}

/// DoH query builder (simplified - real implementation would use proper DNS wire format)
pub struct DohQuery {
    domain: String,
    query_type: QueryType,
}

#[derive(Clone, Copy, Debug)]
pub enum QueryType {
    A,
    AAAA,
}

impl DohQuery {
    /// Create a new A record query
    pub fn a(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            query_type: QueryType::A,
        }
    }

    /// Create a new AAAA record query
    pub fn aaaa(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            query_type: QueryType::AAAA,
        }
    }

    /// Build the query bytes (simplified format for internal use)
    pub fn to_bytes(&self) -> Vec<u8> {
        // Format: query_type (1 byte) + domain
        let mut bytes = Vec::with_capacity(1 + self.domain.len());
        bytes.push(match self.query_type {
            QueryType::A => 0x01,
            QueryType::AAAA => 0x1C,
        });
        bytes.extend(self.domain.as_bytes());
        bytes
    }

    /// Create ControlMessage for this query
    pub fn to_control_message(&self) -> ControlMessage {
        ControlMessage::DohQuery {
            query: self.to_bytes(),
        }
    }
}

/// Parse DoH response (simplified)
pub fn parse_doh_response(response: &ControlMessage) -> Result<Vec<IpAddr>, DohError> {
    match response {
        ControlMessage::DohResponse { response } => {
            if response.is_empty() {
                return Err(DohError::NoResults);
            }

            // Parse response format: count (1 byte) + [type (1 byte) + octets]...
            let count = response[0] as usize;
            let mut results = Vec::with_capacity(count);
            let mut offset = 1;

            for _ in 0..count {
                if offset >= response.len() {
                    break;
                }

                let record_type = response[offset];
                offset += 1;

                match record_type {
                    0x01 => {
                        // A record (4 bytes)
                        if offset + 4 <= response.len() {
                            let ip = Ipv4Addr::new(
                                response[offset],
                                response[offset + 1],
                                response[offset + 2],
                                response[offset + 3],
                            );
                            results.push(IpAddr::V4(ip));
                            offset += 4;
                        }
                    }
                    0x1C => {
                        // AAAA record (16 bytes)
                        if offset + 16 <= response.len() {
                            let mut octets = [0u8; 16];
                            octets.copy_from_slice(&response[offset..offset + 16]);
                            results.push(IpAddr::V6(octets.into()));
                            offset += 16;
                        }
                    }
                    _ => {
                        // Unknown record type, skip
                        break;
                    }
                }
            }

            if results.is_empty() {
                Err(DohError::NoResults)
            } else {
                Ok(results)
            }
        }
        _ => Err(DohError::InvalidResponse),
    }
}

/// Build DoH response bytes from resolved addresses
pub fn build_doh_response(addresses: &[IpAddr]) -> Vec<u8> {
    let mut response = Vec::new();
    response.push(addresses.len() as u8);

    for addr in addresses {
        match addr {
            IpAddr::V4(ip) => {
                response.push(0x01);
                response.extend(&ip.octets());
            }
            IpAddr::V6(ip) => {
                response.push(0x1C);
                response.extend(&ip.octets());
            }
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_to_bytes() {
        let query = DohQuery::a("example.com");
        let bytes = query.to_bytes();

        assert_eq!(bytes[0], 0x01);
        assert_eq!(&bytes[1..], b"example.com");
    }

    #[test]
    fn test_response_roundtrip() {
        let addresses = vec![
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
        ];

        let response_bytes = build_doh_response(&addresses);
        let response = ControlMessage::DohResponse {
            response: response_bytes,
        };

        let parsed = parse_doh_response(&response).unwrap();
        assert_eq!(parsed, addresses);
    }
}
