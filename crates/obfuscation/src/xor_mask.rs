//! SIMD XOR mask for data obfuscation

use std::time::{SystemTime, UNIX_EPOCH};

/// XOR mask configuration
pub struct XorMask {
    /// Session key for mask generation
    session_key: u64,
    /// Time step for mask rotation (seconds)
    time_step: u64,
}

impl XorMask {
    /// Create a new XOR mask with the given session key
    pub fn new(session_key: u64) -> Self {
        Self {
            session_key,
            time_step: 60, // Rotate mask every minute
        }
    }

    /// Create with custom time step
    pub fn with_time_step(session_key: u64, time_step: u64) -> Self {
        Self {
            session_key,
            time_step,
        }
    }

    /// Generate mask bytes based on current time and session key
    fn generate_mask(&self, len: usize) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let seed = (now / self.time_step) ^ self.session_key;

        // Simple PRNG (xorshift64)
        let mut state = seed;
        let mut mask = Vec::with_capacity(len);

        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            mask.push((state & 0xFF) as u8);
        }

        mask
    }

    /// Apply XOR mask to data (in-place)
    pub fn apply_inplace(&self, data: &mut [u8]) {
        let mask = self.generate_mask(data.len());
        self.xor_with_mask(data, &mask);
    }

    /// Apply XOR mask and return new buffer
    pub fn apply(&self, data: &[u8]) -> Vec<u8> {
        let mask = self.generate_mask(data.len());
        let mut result = data.to_vec();
        self.xor_with_mask(&mut result, &mask);
        result
    }

    /// XOR data with mask (chooses SIMD or scalar based on platform)
    #[inline]
    fn xor_with_mask(&self, data: &mut [u8], mask: &[u8]) {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            self.xor_avx2(data, mask);
            return;
        }

        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            self.xor_neon(data, mask);
            return;
        }

        // Portable fallback
        self.xor_scalar(data, mask);
    }

    /// Scalar XOR (portable fallback)
    #[inline]
    fn xor_scalar(&self, data: &mut [u8], mask: &[u8]) {
        for (d, m) in data.iter_mut().zip(mask.iter()) {
            *d ^= *m;
        }
    }

    /// AVX2 SIMD XOR (x86_64)
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    #[inline]
    fn xor_avx2(&self, data: &mut [u8], mask: &[u8]) {
        use std::arch::x86_64::*;

        let len = data.len();
        let mut i = 0;

        unsafe {
            // Process 32 bytes at a time
            while i + 32 <= len {
                let data_vec = _mm256_loadu_si256(data[i..].as_ptr() as *const __m256i);
                let mask_vec = _mm256_loadu_si256(mask[i..].as_ptr() as *const __m256i);
                let xor_vec = _mm256_xor_si256(data_vec, mask_vec);
                _mm256_storeu_si256(data[i..].as_mut_ptr() as *mut __m256i, xor_vec);
                i += 32;
            }
        }

        // Process remaining bytes
        for j in i..len {
            data[j] ^= mask[j];
        }
    }

    /// NEON SIMD XOR (aarch64)
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[inline]
    fn xor_neon(&self, data: &mut [u8], mask: &[u8]) {
        use std::arch::aarch64::*;

        let len = data.len();
        let mut i = 0;

        unsafe {
            // Process 16 bytes at a time
            while i + 16 <= len {
                let data_vec = vld1q_u8(data[i..].as_ptr());
                let mask_vec = vld1q_u8(mask[i..].as_ptr());
                let xor_vec = veorq_u8(data_vec, mask_vec);
                vst1q_u8(data[i..].as_mut_ptr(), xor_vec);
                i += 16;
            }
        }

        // Process remaining bytes
        for j in i..len {
            data[j] ^= mask[j];
        }
    }
}

impl Default for XorMask {
    fn default() -> Self {
        Self::new(0xDEADBEEF_CAFEBABE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_roundtrip() {
        let mask = XorMask::new(12345);
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let masked = mask.apply(&original);
        assert_ne!(original, masked);

        let unmasked = mask.apply(&masked);
        assert_eq!(original, unmasked);
    }

    #[test]
    fn test_xor_inplace() {
        let mask = XorMask::new(12345);
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut data = original.clone();

        mask.apply_inplace(&mut data);
        assert_ne!(original, data);

        mask.apply_inplace(&mut data);
        assert_eq!(original, data);
    }

    #[test]
    fn test_large_data() {
        let mask = XorMask::new(12345);
        let original: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();

        let masked = mask.apply(&original);
        let unmasked = mask.apply(&masked);

        assert_eq!(original, unmasked);
    }

    #[test]
    fn test_empty_data() {
        let mask = XorMask::new(12345);
        let data: Vec<u8> = vec![];

        let result = mask.apply(&data);
        assert!(result.is_empty());
    }
}
