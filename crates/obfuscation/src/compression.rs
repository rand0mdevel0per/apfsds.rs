//! Compression utilities

use thiserror::Error;

/// Compression threshold in bytes
pub const COMPRESSION_THRESHOLD: usize = 1024;

/// Default compression level
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("Data is not compressed")]
    NotCompressed,
}

/// Compress data using zstd if above threshold
pub fn compress_if_needed(data: &[u8]) -> Result<(Vec<u8>, bool), CompressionError> {
    if data.len() < COMPRESSION_THRESHOLD {
        return Ok((data.to_vec(), false));
    }

    compress(data).map(|compressed| (compressed, true))
}

/// Compress data using zstd
pub fn compress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    compress_with_level(data, DEFAULT_COMPRESSION_LEVEL)
}

/// Compress data with specific level (1-22)
pub fn compress_with_level(data: &[u8], level: i32) -> Result<Vec<u8>, CompressionError> {
    zstd::encode_all(data, level).map_err(|e| CompressionError::CompressionFailed(e.to_string()))
}

/// Decompress zstd data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    zstd::decode_all(data).map_err(|e| CompressionError::DecompressionFailed(e.to_string()))
}

/// Decompress with maximum size limit (for safety)
pub fn decompress_with_limit(data: &[u8], max_size: usize) -> Result<Vec<u8>, CompressionError> {
    let mut decoder = zstd::Decoder::new(data)
        .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;

    let mut result = Vec::new();
    let mut buf = [0u8; 8192];

    loop {
        use std::io::Read;
        let n = decoder
            .read(&mut buf)
            .map_err(|e| CompressionError::DecompressionFailed(e.to_string()))?;

        if n == 0 {
            break;
        }

        if result.len() + n > max_size {
            return Err(CompressionError::DecompressionFailed(format!(
                "Decompressed size exceeds limit of {} bytes",
                max_size
            )));
        }

        result.extend_from_slice(&buf[..n]);
    }

    Ok(result)
}

/// Check if data might be zstd compressed (magic number: 0x28 0xB5 0x2F 0xFD)
pub fn is_compressed(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0x28 && data[1] == 0xB5 && data[2] == 0x2F && data[3] == 0xFD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress() {
        let data = b"Hello, APFSDS! This is a test message that should be compressed.";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed).unwrap();

        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compress_if_needed_small() {
        let small_data = vec![1, 2, 3, 4, 5];
        let (result, compressed) = compress_if_needed(&small_data).unwrap();

        assert!(!compressed);
        assert_eq!(small_data, result);
    }

    #[test]
    fn test_compress_if_needed_large() {
        let large_data: Vec<u8> = (0..2000).map(|i| (i % 256) as u8).collect();
        let (result, compressed) = compress_if_needed(&large_data).unwrap();

        assert!(compressed);
        assert_ne!(large_data, result);

        // Verify we can decompress
        let decompressed = decompress(&result).unwrap();
        assert_eq!(large_data, decompressed);
    }

    #[test]
    fn test_is_compressed() {
        let data = b"uncompressed data";
        assert!(!is_compressed(data));

        let compressed = compress(data).unwrap();
        assert!(is_compressed(&compressed));
    }

    #[test]
    fn test_decompress_with_limit() {
        let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let compressed = compress(&data).unwrap();

        // Should fail with small limit
        let result = decompress_with_limit(&compressed, 1000);
        assert!(result.is_err());

        // Should succeed with large limit
        let result = decompress_with_limit(&compressed, 100000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data);
    }
}
