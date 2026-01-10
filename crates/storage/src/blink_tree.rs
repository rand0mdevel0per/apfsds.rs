//! B-link tree index (simplified version for Phase 1)

use dashmap::DashMap;

use crate::SegmentPtr;

/// A simplified B-link tree index using DashMap
/// Full B-link tree implementation will be added in Phase 2
pub struct BLinkTree {
    /// Connection ID -> Segment pointer
    index: DashMap<u64, SegmentPtr>,
}

impl BLinkTree {
    /// Create a new index
    pub fn new() -> Self {
        Self {
            index: DashMap::new(),
        }
    }

    /// Insert or update an entry
    pub fn insert(&self, conn_id: u64, ptr: SegmentPtr) {
        self.index.insert(conn_id, ptr);
    }

    /// Search for a connection
    pub fn search(&self, conn_id: u64) -> Option<SegmentPtr> {
        self.index.get(&conn_id).map(|r| *r)
    }

    /// Remove a connection
    pub fn remove(&self, conn_id: u64) -> Option<SegmentPtr> {
        self.index.remove(&conn_id).map(|(_, v)| v)
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = (u64, SegmentPtr)> + '_ {
        self.index.iter().map(|r| (*r.key(), *r.value()))
    }
}

impl Default for BLinkTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_search() {
        let tree = BLinkTree::new();

        let ptr = SegmentPtr {
            segment_id: 1,
            offset: 100,
        };

        tree.insert(42, ptr);

        let found = tree.search(42).unwrap();
        assert_eq!(found.segment_id, 1);
        assert_eq!(found.offset, 100);
    }

    #[test]
    fn test_remove() {
        let tree = BLinkTree::new();

        let ptr = SegmentPtr {
            segment_id: 1,
            offset: 100,
        };

        tree.insert(42, ptr);
        assert!(tree.search(42).is_some());

        tree.remove(42);
        assert!(tree.search(42).is_none());
    }
}
