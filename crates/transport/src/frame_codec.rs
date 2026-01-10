//! Frame codec for encoding/decoding ProxyFrames over WebSocket

use apfsds_obfuscation::{
    compress, compress_if_needed, decompress, is_compressed, PaddingStrategy, XorMask,
};
use apfsds_protocol::ProxyFrame;
use thiserror::Error;
use tracing::trace;

#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("Invalid frame format")]
    InvalidFrameFormat,
}

/// Frame codec for encoding/decoding ProxyFrames
pub struct FrameCodec {
    xor_mask: XorMask,
    padding: PaddingStrategy,
    compression_enabled: bool,
}

impl FrameCodec {
    /// Create a new codec with the given session key
    pub fn new(session_key: u64) -> Self {
        Self {
            xor_mask: XorMask::new(session_key),
            padding: PaddingStrategy::default(),
            compression_enabled: true,
        }
    }

    /// Create without compression
    pub fn without_compression(session_key: u64) -> Self {
        Self {
            xor_mask: XorMask::new(session_key),
            padding: PaddingStrategy::default(),
            compression_enabled: false,
        }
    }

    /// Encode a ProxyFrame for transmission
    pub fn encode(&self, frame: &ProxyFrame) -> Result<Vec<u8>, CodecError> {
        // 1. Serialize with rkyv
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(frame)
            .map_err(|e| CodecError::SerializationFailed(e.to_string()))?
            .to_vec();

        trace!("Serialized frame: {} bytes", bytes.len());

        // 2. Compress if needed
        let (data, compressed) = if self.compression_enabled {
            compress_if_needed(&bytes)
                .map_err(|e| CodecError::CompressionFailed(e.to_string()))?
        } else {
            (bytes, false)
        };

        trace!(
            "After compression: {} bytes (compressed: {})",
            data.len(),
            compressed
        );

        // 3. XOR mask
        let masked = self.xor_mask.apply(&data);

        // 4. Add padding
        let mut padded = self.padding.pad(&masked);

        // 5. Prepend flags byte (bit 0 = compressed)
        let flags = if compressed { 0x01 } else { 0x00 };
        padded.insert(0, flags);

        trace!("Final encoded size: {} bytes", padded.len());

        Ok(padded)
    }

    /// Decode a ProxyFrame from received data
    pub fn decode(&self, data: &[u8]) -> Result<ProxyFrame, CodecError> {
        if data.is_empty() {
            return Err(CodecError::InvalidFrameFormat);
        }

        // 1. Extract flags byte
        let flags = data[0];
        let compressed = (flags & 0x01) != 0;
        let remaining = &data[1..];

        trace!("Decoding frame: {} bytes, compressed: {}", data.len(), compressed);

        // 2. Remove padding
        let unpadded = PaddingStrategy::unpad(remaining)
            .ok_or(CodecError::InvalidFrameFormat)?;

        // 3. XOR unmask
        let unmasked = self.xor_mask.apply(&unpadded);

        // 4. Decompress if needed
        let bytes = if compressed {
            decompress(&unmasked)
                .map_err(|e| CodecError::DecompressionFailed(e.to_string()))?
        } else {
            unmasked
        };

        // 5. Deserialize with rkyv
        let archived = rkyv::access::<apfsds_protocol::ArchivedProxyFrame, rkyv::rancor::Error>(&bytes)
            .map_err(|e| CodecError::DeserializationFailed(e.to_string()))?;

        let frame: ProxyFrame = rkyv::deserialize::<ProxyFrame, rkyv::rancor::Error>(archived)
            .map_err(|e| CodecError::DeserializationFailed(e.to_string()))?;

        Ok(frame)
    }

    /// Encode frame as binary WebSocket message
    pub fn encode_to_message(&self, frame: &ProxyFrame) -> Result<tokio_tungstenite::tungstenite::Message, CodecError> {
        let bytes = self.encode(frame)?;
        Ok(tokio_tungstenite::tungstenite::Message::Binary(bytes.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let codec = FrameCodec::new(12345);

        let frame = ProxyFrame::new_data(
            42,
            ProxyFrame::ipv4_to_mapped([192, 168, 1, 1]),
            8080,
            vec![1, 2, 3, 4, 5],
        );

        let encoded = codec.encode(&frame).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(frame.conn_id, decoded.conn_id);
        assert_eq!(frame.rport, decoded.rport);
        assert_eq!(frame.payload, decoded.payload);
    }

    #[test]
    fn test_large_payload_compression() {
        let codec = FrameCodec::new(12345);

        // Large payload that should be compressed
        let payload: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        let frame = ProxyFrame::new_data(1, [0; 16], 443, payload.clone());

        let encoded = codec.encode(&frame).unwrap();

        // Check that compression flag is set
        assert_eq!(encoded[0] & 0x01, 0x01);

        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(frame.payload, decoded.payload);
    }

    #[test]
    fn test_without_compression() {
        let codec = FrameCodec::without_compression(12345);

        let payload: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        let frame = ProxyFrame::new_data(1, [0; 16], 443, payload);

        let encoded = codec.encode(&frame).unwrap();

        // Check that compression flag is NOT set
        assert_eq!(encoded[0] & 0x01, 0x00);
    }
}
