//! APFSDS Obfuscation - Traffic obfuscation utilities
//!
//! This crate provides:
//! - SIMD XOR mask (AVX2 + portable fallback)
//! - Smart padding (matching target distribution)
//! - Compression utilities
//! - Timing jitter

mod xor_mask;
mod padding;
mod compression;
mod timing;

pub use xor_mask::*;
pub use padding::*;
pub use compression::*;
pub use timing::*;
