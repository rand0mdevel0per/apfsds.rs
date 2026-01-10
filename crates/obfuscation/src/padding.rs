//! Smart padding to disguise packet sizes

/// Target packet sizes mimicking real API traffic
const SIZE_DISTRIBUTION: &[(usize, f32)] = &[
    (512, 0.40),   // 40% small packets
    (1024, 0.20),  // 20%
    (2048, 0.15),  // 15%
    (4096, 0.15),  // 15%
    (8192, 0.07),  // 7%
    (16384, 0.03), // 3% large packets
];

/// Maximum jitter percentage (±10%)
const JITTER_PERCENT: usize = 10;

/// Padding strategy configuration
pub struct PaddingStrategy {
    /// Enable random jitter
    pub jitter: bool,
    /// Minimum output size
    pub min_size: usize,
}

impl Default for PaddingStrategy {
    fn default() -> Self {
        Self {
            jitter: true,
            min_size: 64,
        }
    }
}

impl PaddingStrategy {
    /// Create a new padding strategy
    pub fn new() -> Self {
        Self::default()
    }

    /// Without jitter (for testing)
    pub fn no_jitter() -> Self {
        Self {
            jitter: false,
            min_size: 64,
        }
    }

    /// Calculate target size for padding
    pub fn calculate_target_size(&self, payload_len: usize) -> usize {
        // Find the next target size
        let base_target = SIZE_DISTRIBUTION
            .iter()
            .find(|(size, _)| *size > payload_len)
            .map(|(size, _)| *size)
            .unwrap_or(16384);

        // Ensure minimum size
        let target = base_target.max(self.min_size);

        if self.jitter {
            // Add jitter (±10%)
            let jitter_range = target / JITTER_PERCENT;
            let jitter = fastrand::usize(0..=jitter_range * 2);
            target.saturating_sub(jitter_range) + jitter
        } else {
            target
        }
    }

    /// Calculate how much padding to add
    pub fn calculate_padding_len(&self, payload_len: usize) -> usize {
        let target = self.calculate_target_size(payload_len);
        target.saturating_sub(payload_len)
    }

    /// Add padding to data
    pub fn pad(&self, data: &[u8]) -> Vec<u8> {
        let padding_len = self.calculate_padding_len(data.len());
        let total_len = data.len() + padding_len + 4; // 4 bytes for original length

        let mut result = Vec::with_capacity(total_len);

        // Store original length as u32 LE
        result.extend_from_slice(&(data.len() as u32).to_le_bytes());

        // Original data
        result.extend_from_slice(data);

        // Random padding (not zeros to avoid patterns)
        for _ in 0..padding_len {
            result.push(fastrand::u8(..));
        }

        result
    }

    /// Remove padding from data
    pub fn unpad(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 4 {
            return None;
        }

        // Read original length
        let original_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

        if data.len() < 4 + original_len {
            return None;
        }

        Some(data[4..4 + original_len].to_vec())
    }
}

/// Select a target size based on distribution
pub fn select_distributed_size() -> usize {
    let r = fastrand::f32();
    let mut cumulative = 0.0;

    for &(size, prob) in SIZE_DISTRIBUTION {
        cumulative += prob;
        if r < cumulative {
            return size;
        }
    }

    8192 // fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_unpad_roundtrip() {
        let strategy = PaddingStrategy::no_jitter();
        let original = vec![1, 2, 3, 4, 5];

        let padded = strategy.pad(&original);
        assert!(padded.len() > original.len());

        let unpadded = PaddingStrategy::unpad(&padded).unwrap();
        assert_eq!(original, unpadded);
    }

    #[test]
    fn test_target_size_selection() {
        let strategy = PaddingStrategy::no_jitter();

        // Small data should be padded to 512
        assert_eq!(strategy.calculate_target_size(100), 512);

        // Medium data should be padded to 1024
        assert_eq!(strategy.calculate_target_size(600), 1024);

        // Large data should be padded to 16384
        assert_eq!(strategy.calculate_target_size(10000), 16384);
    }

    #[test]
    fn test_padding_is_random() {
        let strategy = PaddingStrategy::default();
        let data = vec![0u8; 10];

        let padded1 = strategy.pad(&data);
        let padded2 = strategy.pad(&data);

        // Padding should be different due to random bytes
        let padding1 = &padded1[14..];
        let padding2 = &padded2[14..];

        // Note: This could theoretically fail with very low probability
        assert_ne!(padding1, padding2);
    }

    #[test]
    fn test_distributed_size() {
        // Run multiple times to verify distribution
        let mut counts = [0usize; 6];
        let n = 10000;

        for _ in 0..n {
            let size = select_distributed_size();
            match size {
                512 => counts[0] += 1,
                1024 => counts[1] += 1,
                2048 => counts[2] += 1,
                4096 => counts[3] += 1,
                8192 => counts[4] += 1,
                16384 => counts[5] += 1,
                _ => {}
            }
        }

        // 512 should be most common (~40%)
        assert!(counts[0] > counts[1]);
        assert!(counts[0] > counts[5]);
    }

    #[test]
    fn test_empty_data() {
        let strategy = PaddingStrategy::no_jitter();
        let data: Vec<u8> = vec![];

        let padded = strategy.pad(&data);
        let unpadded = PaddingStrategy::unpad(&padded).unwrap();

        assert!(unpadded.is_empty());
    }
}
