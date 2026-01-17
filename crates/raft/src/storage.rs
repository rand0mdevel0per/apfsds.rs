use crate::{ClientRequest, ClientResponse, NodeId};
use anyhow::Result;
use apfsds_storage::{ClickHouseBackup, ClickHouseConfig, Wal};
use async_raft::RaftStorage;
use async_raft::raft::{Entry, MembershipConfig};
use async_raft::storage::{CurrentSnapshotData, HardState, InitialState};
use async_trait::async_trait;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Persistent storage implementation for async-raft
pub struct PersistentStorage {
    node_id: NodeId,
    membership: RwLock<MembershipConfig>,
    log: RwLock<Vec<Entry<ClientRequest>>>, // In-memory log backed by WAL
    hard_state: RwLock<HardState>,
    snapshot: RwLock<Option<CurrentSnapshotData<Cursor<Vec<u8>>>>>,
    wal: Arc<Wal>,
    clickhouse: Arc<ClickHouseBackup>,
}

impl PersistentStorage {
    pub fn new(
        node_id: NodeId,
        data_dir: PathBuf,
        clickhouse_config: ClickHouseConfig,
    ) -> Result<Self> {
        let wal_path = data_dir.join(format!("raft-{}.wal", node_id));
        let wal = Arc::new(Wal::open(&wal_path)?);

        // Replay WAL to restore log entries
        let mut restored_log = Vec::new();
        if let Ok(entries) = wal.read_all() {
            for data in entries {
                if let Ok(entry) = serde_json::from_slice::<Entry<ClientRequest>>(&data) {
                    restored_log.push(entry);
                }
            }
            tracing::info!("Restored {} entries from WAL", restored_log.len());
        }

        let clickhouse = Arc::new(ClickHouseBackup::new(clickhouse_config)?);
        if clickhouse.is_enabled() {
            let ch = clickhouse.clone();
            tokio::spawn(async move {
                if let Err(e) = ch.ensure_tables().await {
                    eprintln!("Failed to ensure ClickHouse tables: {}", e);
                }
            });
            // Start flush task
            clickhouse.clone().start_flush_task();
        }

        let membership = MembershipConfig::new_initial(node_id);

        Ok(Self {
            node_id,
            membership: RwLock::new(membership),
            log: RwLock::new(restored_log),
            hard_state: RwLock::new(HardState {
                current_term: 0,
                voted_for: None,
            }),
            snapshot: RwLock::new(None),
            wal,
            clickhouse,
        })
    }
}

#[async_trait]
impl RaftStorage<ClientRequest, ClientResponse> for PersistentStorage {
    type Snapshot = Cursor<Vec<u8>>;
    type ShutdownError = std::io::Error;

    // ... (unchanged methods) ...

    async fn get_membership_config(&self) -> Result<MembershipConfig> {
        Ok(self.membership.read().await.clone())
    }

    async fn get_initial_state(&self) -> Result<InitialState> {
        let membership = self.membership.read().await.clone();
        let log = self.log.read().await;
        let hard_state = self.hard_state.read().await.clone();

        let (last_log_term, last_log_index) = match log.last() {
            Some(entry) => (entry.term, entry.index),
            None => (0, 0),
        };

        Ok(InitialState {
            last_log_term,
            last_log_index,
            last_applied_log: last_log_index,
            hard_state,
            membership,
        })
    }

    async fn save_hard_state(&self, hs: &HardState) -> Result<()> {
        *self.hard_state.write().await = hs.clone();
        // Persist HardState to WAL with special marker
        let marker = format!("__HARDSTATE__:{}", serde_json::to_string(hs).unwrap_or_default());
        let _ = self.wal.append(marker.as_bytes());
        let _ = self.wal.sync();
        Ok(())
    }

    async fn get_log_entries(&self, start: u64, stop: u64) -> Result<Vec<Entry<ClientRequest>>> {
        let log = self.log.read().await;
        Ok(log
            .iter()
            .filter(|e| e.index >= start && e.index < stop)
            .cloned()
            .collect())
    }

    async fn delete_logs_from(&self, start: u64, stop: Option<u64>) -> Result<()> {
        let mut log = self.log.write().await;
        if let Some(stop_idx) = stop {
            log.retain(|e| e.index < start || e.index >= stop_idx);
        } else {
            log.retain(|e| e.index < start);
        }
        // WAL compaction happens during snapshot; for now just log
        tracing::debug!("Log truncated from index {}", start);
        Ok(())
    }

    async fn append_entry_to_log(&self, entry: &Entry<ClientRequest>) -> Result<()> {
        let mut log = self.log.write().await;
        log.push(entry.clone());

        // Persist to WAL
        let data = serde_json::to_vec(entry).unwrap(); // Use efficient serialization later
        self.wal.append(&data)?;
        self.wal.sync()?; // Fsync for durability

        Ok(())
    }

    async fn replicate_to_log(&self, entries: &[Entry<ClientRequest>]) -> Result<()> {
        let mut log = self.log.write().await;
        log.extend_from_slice(entries);

        // Persist batch to WAL
        for entry in entries {
            let data = serde_json::to_vec(entry).unwrap();
            self.wal.append(&data)?;
        }
        self.wal.sync()?;

        Ok(())
    }

    async fn apply_entry_to_state_machine(
        &self,
        index: &u64,
        data: &ClientRequest,
    ) -> Result<ClientResponse> {
        // Push commit to ClickHouse
        if self.clickhouse.is_enabled() {
            let payload = serde_json::to_string(data).unwrap_or_default();
            let op = match data {
                ClientRequest::Upsert { .. } => "Upsert",
                ClientRequest::Delete { .. } => "Delete",
                ClientRequest::Cleanup { .. } => "Cleanup",
                ClientRequest::Noop => "Noop",
            };

            // We don't have the term here easily unless passing it in replicate_to_state_machine
            // But async-raft apply sends generic data.
            // We can use 0 for term if not critical, or find a way to get it from log.
            // For now, let's just log it.
            // Ideally we get the entry from the log using index, but that's expensive.
            // Let's assume for archive, term 0 is acceptable placeholder or we fix the trait.
            let _ = self
                .clickhouse
                .archive_raft_log(*index, 0, op, &payload)
                .await;
        }

        match data {
            ClientRequest::Upsert { .. } => Ok(ClientResponse::Ok { affected: 1 }),
            ClientRequest::Delete { .. } => Ok(ClientResponse::Ok { affected: 1 }),
            _ => Ok(ClientResponse::Ok { affected: 0 }),
        }
    }

    async fn replicate_to_state_machine(&self, entries: &[(&u64, &ClientRequest)]) -> Result<()> {
        for (index, data) in entries {
            let _ = self.apply_entry_to_state_machine(index, data).await?;
        }
        Ok(())
    }

    async fn do_log_compaction(&self) -> Result<CurrentSnapshotData<Self::Snapshot>> {
        let snapshot = CurrentSnapshotData {
            term: 0,
            index: 0,
            membership: self.membership.read().await.clone(),
            snapshot: Box::new(Cursor::new(Vec::new())),
        };
        Ok(snapshot)
    }

    async fn create_snapshot(&self) -> Result<(String, Box<Self::Snapshot>)> {
        Ok(("snapshot".into(), Box::new(Cursor::new(Vec::new()))))
    }

    async fn finalize_snapshot_installation(
        &self,
        _index: u64,
        _term: u64,
        _delete_through: Option<u64>,
        _id: String,
        _snapshot: Box<Self::Snapshot>,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_current_snapshot(&self) -> Result<Option<CurrentSnapshotData<Self::Snapshot>>> {
        match &*self.snapshot.read().await {
            Some(s) => {
                let snapshot_data = CurrentSnapshotData {
                    term: s.term,
                    index: s.index,
                    membership: s.membership.clone(),
                    snapshot: Box::new(Cursor::new(s.snapshot.get_ref().clone())),
                };
                Ok(Some(snapshot_data))
            }
            None => Ok(None),
        }
    }
}
