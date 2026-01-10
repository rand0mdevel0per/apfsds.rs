//! HMAC-SHA256 authentication

use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug)]
pub enum HmacError {
    #[error("Invalid key length")]
    InvalidKeyLength,

    #[error("HMAC verification failed")]
    VerificationFailed,
}

/// HMAC-SHA256 authenticator
pub struct HmacAuthenticator {
    secret: [u8; 32],
}

impl HmacAuthenticator {
    /// Create a new authenticator with the given secret
    pub fn new(secret: [u8; 32]) -> Self {
        Self { secret }
    }

    /// Compute HMAC for the given data
    pub fn compute(&self, data: &[u8]) -> [u8; 32] {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC can take key of any size");
        mac.update(data);
        mac.finalize().into_bytes().into()
    }

    /// Compute HMAC with timestamp
    pub fn compute_with_timestamp(&self, data: &[u8], timestamp: u64) -> [u8; 32] {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .expect("HMAC can take key of any size");
        mac.update(data);
        mac.update(&timestamp.to_le_bytes());
        mac.finalize().into_bytes().into()
    }

    /// Verify HMAC in constant time
    pub fn verify(&self, data: &[u8], expected: &[u8; 32]) -> Result<(), HmacError> {
        let computed = self.compute(data);
        if constant_time_compare(&computed, expected) {
            Ok(())
        } else {
            Err(HmacError::VerificationFailed)
        }
    }

    /// Verify HMAC with timestamp in constant time
    pub fn verify_with_timestamp(
        &self,
        data: &[u8],
        timestamp: u64,
        expected: &[u8; 32],
    ) -> Result<(), HmacError> {
        let computed = self.compute_with_timestamp(data, timestamp);
        if constant_time_compare(&computed, expected) {
            Ok(())
        } else {
            Err(HmacError::VerificationFailed)
        }
    }
}

/// Constant-time comparison to prevent timing attacks
#[inline]
fn constant_time_compare(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut result = 0u8;
    for i in 0..32 {
        result |= a[i] ^ b[i];
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_compute_verify() {
        let secret = [42u8; 32];
        let auth = HmacAuthenticator::new(secret);

        let data = b"Hello, APFSDS!";
        let mac = auth.compute(data);

        assert!(auth.verify(data, &mac).is_ok());
    }

    #[test]
    fn test_hmac_wrong_data() {
        let secret = [42u8; 32];
        let auth = HmacAuthenticator::new(secret);

        let data = b"Hello, APFSDS!";
        let wrong_data = b"Wrong data!";
        let mac = auth.compute(data);

        assert!(auth.verify(wrong_data, &mac).is_err());
    }

    #[test]
    fn test_hmac_with_timestamp() {
        let secret = [42u8; 32];
        let auth = HmacAuthenticator::new(secret);

        let data = b"Hello, APFSDS!";
        let timestamp = 1234567890u64;

        let mac = auth.compute_with_timestamp(data, timestamp);
        assert!(auth.verify_with_timestamp(data, timestamp, &mac).is_ok());

        // Wrong timestamp should fail
        assert!(auth.verify_with_timestamp(data, timestamp + 1, &mac).is_err());
    }

    #[test]
    fn test_constant_time_compare() {
        let a = [1u8; 32];
        let b = [1u8; 32];
        let c = [2u8; 32];

        assert!(constant_time_compare(&a, &b));
        assert!(!constant_time_compare(&a, &c));
    }
}
