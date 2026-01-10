//! Raft log storage implementation

use crate::{NodeId, TypeConfig};
use openraft::storage::{LogFlushed, RaftLogReader, RaftLogStorage};
use openraft::{Entry, LogId, LogState, OptionalSend, StorageError, Vote};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::RangeBounds;
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// In-memory log storage
pub struct LogStorage {
    vote: RwLock<Option<Vote<NodeId>>>,
    log: RwLock<BTreeMap<u64, Entry<TypeConfig>>>,
    last_purged_log_id: RwLock<Option<LogId<NodeId>>>,
}

impl LogStorage {
    pub fn new() -> Self {
        Self {
            vote: RwLock::new(None),
            log: RwLock::new(BTreeMap::new()),
            last_purged_log_id: RwLock::new(None),
        }
    }
}

impl Default for LogStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftLogReader<TypeConfig> for LogStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + OptionalSend>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>> {
        let log = self.log.read().await;
        let entries: Vec<_> = log.range(range).map(|(_, v)| v.clone()).collect();
        Ok(entries)
    }
}

impl RaftLogStorage<TypeConfig> for LogStorage {
    type LogReader = Self;

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let log = self.log.read().await;
        let last_purged = *self.last_purged_log_id.read().await;
        let last = log.iter().next_back().map(|(_, ent)| ent.log_id);

        Ok(LogState {
            last_purged_log_id: last_purged,
            last_log_id: last,
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        Self {
            vote: RwLock::new(*self.vote.read().await),
            log: RwLock::new(self.log.read().await.clone()),
            last_purged_log_id: RwLock::new(*self.last_purged_log_id.read().await),
        }
    }

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        debug!("Saving vote: {:?}", vote);
        *self.vote.write().await = Some(*vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        Ok(*self.vote.read().await)
    }

    async fn append<I>(
        &mut self,
        entries: I,
        callback: LogFlushed<TypeConfig>,
    ) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
    {
        let mut log = self.log.write().await;
        for entry in entries {
            trace!("Appending log entry: {:?}", entry.log_id);
            log.insert(entry.log_id.index, entry);
        }
        callback.log_io_completed(Ok(()));
        Ok(())
    }

    async fn truncate(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        debug!("Truncating log at: {:?}", log_id);
        let mut log = self.log.write().await;
        let keys_to_remove: Vec<_> = log.range(log_id.index..).map(|(k, _)| *k).collect();
        for key in keys_to_remove {
            log.remove(&key);
        }
        Ok(())
    }

    async fn purge(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        debug!("Purging log up to: {:?}", log_id);
        *self.last_purged_log_id.write().await = Some(log_id);
        let mut log = self.log.write().await;
        let keys_to_remove: Vec<_> = log.range(..=log_id.index).map(|(k, _)| *k).collect();
        for key in keys_to_remove {
            log.remove(&key);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_storage_basic() {
        let mut storage = LogStorage::new();
        let state = storage.get_log_state().await.unwrap();
        assert!(state.last_log_id.is_none());
    }
}
