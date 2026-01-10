//! Frame validation utilities

use crate::frame::{ArchivedProxyFrame, ProxyFrame};
use thiserror::Error;

/// Validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Duplicate frame UUID detected")]
    DuplicateUuid,

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("Timestamp out of range: {0}ms drift")]
    TimestampOutOfRange(i64),

    #[error("Invalid connection ID")]
    InvalidConnId,

    #[error("Payload too large: {size} bytes (max: {max})")]
    PayloadTooLarge { size: usize, max: usize },
}

/// Maximum allowed payload size (64KB)
pub const MAX_PAYLOAD_SIZE: usize = 65536;

/// Maximum allowed timestamp drift (30 seconds)
pub const MAX_TIMESTAMP_DRIFT_MS: i64 = 30_000;

/// Validate a ProxyFrame
pub fn validate_frame(frame: &ProxyFrame, current_time_ms: u64) -> Result<(), ValidationError> {
    // Check payload size
    if frame.payload.len() > MAX_PAYLOAD_SIZE {
        return Err(ValidationError::PayloadTooLarge {
            size: frame.payload.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    // Verify checksum
    let computed = crc32fast::hash(&frame.payload);
    if computed != frame.checksum {
        return Err(ValidationError::ChecksumMismatch {
            expected: frame.checksum,
            actual: computed,
        });
    }

    // Check timestamp
    let drift = current_time_ms as i64 - frame.timestamp as i64;
    if drift.abs() > MAX_TIMESTAMP_DRIFT_MS {
        return Err(ValidationError::TimestampOutOfRange(drift));
    }

    Ok(())
}

/// Validate an archived frame (zero-copy)
pub fn validate_archived_frame(
    frame: &ArchivedProxyFrame,
    current_time_ms: u64,
) -> Result<(), ValidationError> {
    // Check payload size
    if frame.payload.len() > MAX_PAYLOAD_SIZE {
        return Err(ValidationError::PayloadTooLarge {
            size: frame.payload.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    // Verify checksum - convert from rkyv's archived type
    let frame_checksum: u32 = frame.checksum.to_native();
    let computed = crc32fast::hash(&frame.payload);
    if computed != frame_checksum {
        return Err(ValidationError::ChecksumMismatch {
            expected: frame_checksum,
            actual: computed,
        });
    }

    // Check timestamp - convert from rkyv's archived type
    let frame_timestamp: u64 = frame.timestamp.to_native();
    let drift = current_time_ms as i64 - frame_timestamp as i64;
    if drift.abs() > MAX_TIMESTAMP_DRIFT_MS {
        return Err(ValidationError::TimestampOutOfRange(drift));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_frame() {
        let frame = ProxyFrame::new_data(1, [0; 16], 443, vec![1, 2, 3]);
        let result = validate_frame(&frame, frame.timestamp);
        assert!(result.is_ok());
    }

    #[test]
    fn test_checksum_mismatch() {
        let mut frame = ProxyFrame::new_data(1, [0; 16], 443, vec![1, 2, 3]);
        frame.checksum = 0xDEADBEEF; // Wrong checksum

        let result = validate_frame(&frame, frame.timestamp);
        assert!(matches!(result, Err(ValidationError::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_timestamp_drift() {
        let frame = ProxyFrame::new_data(1, [0; 16], 443, vec![1, 2, 3]);
        let future_time = frame.timestamp + 60_000; // 60 seconds later

        let result = validate_frame(&frame, future_time);
        assert!(matches!(result, Err(ValidationError::TimestampOutOfRange(_))));
    }

    #[test]
    fn test_payload_too_large() {
        let large_payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];
        let frame = ProxyFrame::new_data(1, [0; 16], 443, large_payload);

        let result = validate_frame(&frame, frame.timestamp);
        assert!(matches!(result, Err(ValidationError::PayloadTooLarge { .. })));
    }
}
