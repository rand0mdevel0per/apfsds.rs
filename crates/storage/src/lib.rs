//! APFSDS Storage - MVCC storage engine
//!
//! This crate provides:
//! - MVCC segment-based storage
//! - B-link tree index (lock-free)
//! - Compaction
//! - tmpfs integration

mod segment;
mod blink_tree;
mod engine;

pub use segment::*;
pub use blink_tree::*;
pub use engine::*;

// TODO: Implement full storage engine
// For Phase 1, we use a simple in-memory store
