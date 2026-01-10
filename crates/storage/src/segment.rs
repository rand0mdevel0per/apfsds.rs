//! Storage segment for MVCC

use apfsds_protocol::ConnRecord;
use std::sync::atomic::{AtomicU64, Ordering};

/// Segment ID counter
static SEGMENT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A storage segment containing connection records
pub struct Segment {
    /// Segment ID
    pub id: u64,

    /// Data buffer
    data: Vec<u8>,

    /// Record offsets
    offsets: Vec<usize>,

    /// Is this segment sealed (immutable)?
    pub is_sealed: bool,

    /// Size limit
    size_limit: usize,
}

impl Segment {
    /// Create a new segment
    pub fn new() -> Self {
        Self::with_size_limit(10 * 1024 * 1024) // 10MB default
    }

    /// Create with custom size limit
    pub fn with_size_limit(size_limit: usize) -> Self {
        Self {
            id: SEGMENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            data: Vec::with_capacity(size_limit / 10),
            offsets: Vec::new(),
            is_sealed: false,
            size_limit,
        }
    }

    /// Append a record to the segment
    pub fn append(&mut self, record: &ConnRecord) -> Option<usize> {
        if self.is_sealed {
            return None;
        }

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(record).ok()?;

        if self.data.len() + bytes.len() > self.size_limit {
            return None;
        }

        let offset = self.data.len();
        self.data.extend_from_slice(&bytes);
        self.offsets.push(offset);

        Some(offset)
    }

    /// Read a record at offset
    pub fn read_at(&self, offset: usize) -> Option<ConnRecord> {
        if offset >= self.data.len() {
            return None;
        }

        // Find end of record (next offset or end of data)
        let end = self
            .offsets
            .iter()
            .find(|&&o| o > offset)
            .copied()
            .unwrap_or(self.data.len());

        let bytes = &self.data[offset..end];

        let archived = rkyv::access::<apfsds_protocol::ArchivedConnRecord, rkyv::rancor::Error>(bytes).ok()?;
        rkyv::deserialize::<ConnRecord, rkyv::rancor::Error>(archived).ok()
    }

    /// Get the current size
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the number of records
    pub fn record_count(&self) -> usize {
        self.offsets.len()
    }

    /// Seal the segment (make immutable)
    pub fn seal(&mut self) {
        self.is_sealed = true;
    }
}

impl Default for Segment {
    fn default() -> Self {
        Self::new()
    }
}

/// Pointer to a record in a segment
#[derive(Debug, Clone, Copy)]
pub struct SegmentPtr {
    pub segment_id: u64,
    pub offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use apfsds_protocol::{ConnMeta, StreamState};

    fn make_record(conn_id: u64) -> ConnRecord {
        ConnRecord {
            conn_id,
            metadata: ConnMeta {
                client_addr: [0; 16],
                nat_entry: (1234, 5678),
                assigned_pod: 1,
                stream_states: vec![],
            },
            created_at: 0,
            last_active: 0,
            access_count: 0,
            txid: 0,
        }
    }

    #[test]
    fn test_append_and_read() {
        let mut segment = Segment::new();
        let record = make_record(42);

        let offset = segment.append(&record).unwrap();
        let read_back = segment.read_at(offset).unwrap();

        assert_eq!(read_back.conn_id, 42);
    }

    #[test]
    fn test_multiple_records() {
        let mut segment = Segment::new();

        for i in 0..10 {
            let record = make_record(i);
            segment.append(&record).unwrap();
        }

        assert_eq!(segment.record_count(), 10);
    }

    #[test]
    fn test_sealed_segment() {
        let mut segment = Segment::new();
        segment.seal();

        let record = make_record(1);
        assert!(segment.append(&record).is_none());
    }
}
