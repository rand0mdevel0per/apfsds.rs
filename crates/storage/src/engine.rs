//! Storage engine for connection state

use apfsds_protocol::{ConnMeta, ConnRecord};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

use crate::{BLinkTree, Segment, SegmentPtr};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Segment full")]
    SegmentFull,

    #[error("Record not found")]
    NotFound,

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Storage engine configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Segment size limit in bytes
    pub segment_size_limit: usize,

    /// Number of segments to keep before compaction
    pub compaction_threshold: usize,

    /// Cleanup interval in seconds
    pub cleanup_interval: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            segment_size_limit: 10 * 1024 * 1024, // 10MB
            compaction_threshold: 10,
            cleanup_interval: 300, // 5 minutes
        }
    }
}

/// MVCC storage engine
pub struct StorageEngine {
    /// Active segment (write target)
    active_segment: RwLock<Segment>,

    /// Sealed segments (read-only)
    sealed_segments: RwLock<Vec<Segment>>,

    /// B-link tree index
    index: Arc<BLinkTree>,

    /// Global transaction ID counter
    txid_counter: AtomicU64,

    /// Configuration
    config: StorageConfig,
}

impl StorageEngine {
    /// Create a new storage engine
    pub fn new(config: StorageConfig) -> Self {
        let segment = Segment::with_size_limit(config.segment_size_limit);

        Self {
            active_segment: RwLock::new(segment),
            sealed_segments: RwLock::new(Vec::new()),
            index: Arc::new(BLinkTree::new()),
            txid_counter: AtomicU64::new(1),
            config,
        }
    }

    /// Get the next transaction ID
    pub fn next_txid(&self) -> u64 {
        self.txid_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Insert or update a connection record
    pub fn upsert(&self, conn_id: u64, metadata: ConnMeta) -> Result<u64, StorageError> {
        let txid = self.next_txid();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let record = ConnRecord {
            conn_id,
            metadata,
            created_at: now,
            last_active: now,
            access_count: 1,
            txid,
        };

        // Try to append to active segment
        let mut segment = self.active_segment.write();
        let offset = segment.append(&record);

        match offset {
            Some(offset) => {
                let ptr = SegmentPtr {
                    segment_id: segment.id,
                    offset,
                };
                self.index.insert(conn_id, ptr);
                Ok(txid)
            }
            None => {
                // Segment full - seal and create new
                drop(segment);
                self.rotate_segment()?;

                // Retry
                let mut segment = self.active_segment.write();
                let offset = segment.append(&record).ok_or(StorageError::SegmentFull)?;

                let ptr = SegmentPtr {
                    segment_id: segment.id,
                    offset,
                };
                self.index.insert(conn_id, ptr);
                Ok(txid)
            }
        }
    }

    /// Get a connection record
    pub fn get(&self, conn_id: u64) -> Option<ConnRecord> {
        let ptr = self.index.search(conn_id)?;

        // Search in active segment
        let active = self.active_segment.read();
        if ptr.segment_id == active.id {
            return active.read_at(ptr.offset);
        }
        drop(active);

        // Search in sealed segments
        let sealed = self.sealed_segments.read();
        for segment in sealed.iter() {
            if ptr.segment_id == segment.id {
                return segment.read_at(ptr.offset);
            }
        }

        None
    }

    /// Delete a connection record
    pub fn delete(&self, conn_id: u64) -> Option<SegmentPtr> {
        self.index.remove(conn_id)
    }

    /// Rotate the active segment
    fn rotate_segment(&self) -> Result<(), StorageError> {
        let mut active = self.active_segment.write();
        let mut sealed = self.sealed_segments.write();

        // Seal current segment
        let mut old_segment = std::mem::replace(
            &mut *active,
            Segment::with_size_limit(self.config.segment_size_limit),
        );
        old_segment.seal();

        sealed.push(old_segment);

        // Check if we need to compact
        if sealed.len() > self.config.compaction_threshold {
            // Compaction: merge sealed segments and remove obsolete entries
            // For now, just log - production would spawn async compaction task
            tracing::info!("Compaction threshold reached: {} sealed segments", sealed.len());
        }

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> StorageStats {
        let active = self.active_segment.read();
        let sealed = self.sealed_segments.read();

        StorageStats {
            active_segment_size: active.size(),
            active_record_count: active.record_count(),
            sealed_segment_count: sealed.len(),
            total_indexed: self.index.len(),
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub active_segment_size: usize,
    pub active_record_count: usize,
    pub sealed_segment_count: usize,
    pub total_indexed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta() -> ConnMeta {
        ConnMeta {
            client_addr: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            nat_entry: (1234, 5678),
            assigned_pod: 1,
            stream_states: vec![],
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let engine = StorageEngine::new(StorageConfig::default());

        let meta = make_meta();
        engine.upsert(42, meta.clone()).unwrap();

        let record = engine.get(42).unwrap();
        assert_eq!(record.conn_id, 42);
    }

    #[test]
    fn test_delete() {
        let engine = StorageEngine::new(StorageConfig::default());

        let meta = make_meta();
        engine.upsert(42, meta).unwrap();
        assert!(engine.get(42).is_some());

        engine.delete(42);
        assert!(engine.get(42).is_none());
    }

    #[test]
    fn test_stats() {
        let engine = StorageEngine::new(StorageConfig::default());

        for i in 0..10 {
            engine.upsert(i, make_meta()).unwrap();
        }

        let stats = engine.stats();
        assert_eq!(stats.total_indexed, 10);
        assert_eq!(stats.active_record_count, 10);
    }
}
