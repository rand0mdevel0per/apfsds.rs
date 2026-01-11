//! APFSDS Obfuscation - Traffic obfuscation utilities
//!
//! This crate provides:
//! - SIMD XOR mask (AVX2 + portable fallback)
//! - Smart padding (matching target distribution)
//! - Compression utilities
//! - Timing jitter

mod compression;
mod padding;
mod timing;
mod xor_mask;

pub use compression::*;
pub use padding::*;
pub use timing::*;
pub use xor_mask::*;
