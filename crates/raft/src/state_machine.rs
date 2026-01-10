//! Raft state machine implementation

use crate::{Request, Response, TypeConfig, NodeId};
use apfsds_storage::StorageEngine;
use openraft::storage::RaftStateMachine;
use openraft::{BasicNode, EntryPayload, LogId, RaftSnapshotBuilder, Snapshot, SnapshotMeta, StoredMembership};
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// State machine for APFSDS Raft
pub struct StateMachine {
    /// Last applied log index
    pub last_applied_log: Option<LogId<NodeId>>,

    /// Last membership configuration
    pub last_membership: StoredMembership<NodeId, BasicNode>,

    /// Storage engine for connection state
    storage: Arc<StorageEngine>,

    /// In-memory connection count
    connection_count: Arc<RwLock<u64>>,
}

impl StateMachine {
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self {
            last_applied_log: None,
            last_membership: StoredMembership::default(),
            storage,
            connection_count: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn connection_count(&self) -> u64 {
        *self.connection_count.read().await
    }

    async fn apply_request(&self, request: &Request) -> Response {
        match request {
            Request::Upsert {
                conn_id,
                txid: _,
                client_addr,
                nat_entry,
                assigned_pod,
            } => {
                let metadata = apfsds_protocol::ConnMeta {
                    client_addr: *client_addr,
                    nat_entry: *nat_entry,
                    assigned_pod: *assigned_pod,
                    stream_states: vec![],
                };

                match self.storage.upsert(*conn_id, metadata) {
                    Ok(_) => {
                        let mut count = self.connection_count.write().await;
                        *count += 1;
                        Response::Ok { affected: 1 }
                    }
                    Err(e) => Response::Error {
                        message: e.to_string(),
                    },
                }
            }

            Request::Delete { conn_id } => {
                if self.storage.delete(*conn_id).is_some() {
                    let mut count = self.connection_count.write().await;
                    *count = count.saturating_sub(1);
                    Response::Ok { affected: 1 }
                } else {
                    Response::Ok { affected: 0 }
                }
            }

            Request::Cleanup { .. } => Response::Ok { affected: 0 },

            Request::Noop => Response::Ok { affected: 0 },
        }
    }
}

impl RaftStateMachine<TypeConfig> for StateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, BasicNode>), openraft::StorageError<NodeId>>
    {
        Ok((self.last_applied_log, self.last_membership.clone()))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<Response>, openraft::StorageError<NodeId>>
    where
        I: IntoIterator<Item = openraft::Entry<TypeConfig>> + Send,
    {
        let mut responses = Vec::new();

        for entry in entries {
            debug!("Applying entry: {:?}", entry.log_id);
            self.last_applied_log = Some(entry.log_id);

            match entry.payload {
                EntryPayload::Blank => responses.push(Response::Ok { affected: 0 }),
                EntryPayload::Normal(request) => {
                    responses.push(self.apply_request(&request).await);
                }
                EntryPayload::Membership(membership) => {
                    self.last_membership = StoredMembership::new(Some(entry.log_id), membership);
                    responses.push(Response::Ok { affected: 0 });
                }
            }
        }

        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        Self {
            last_applied_log: self.last_applied_log,
            last_membership: self.last_membership.clone(),
            storage: self.storage.clone(),
            connection_count: self.connection_count.clone(),
        }
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, openraft::StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), openraft::StorageError<NodeId>> {
        info!("Installing snapshot: {:?}", meta);
        let _data = snapshot.into_inner();
        self.last_applied_log = meta.last_log_id;
        self.last_membership = meta.last_membership.clone();
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, openraft::StorageError<NodeId>> {
        Ok(None)
    }
}

impl RaftSnapshotBuilder<TypeConfig> for StateMachine {
    async fn build_snapshot(
        &mut self,
    ) -> Result<Snapshot<TypeConfig>, openraft::StorageError<NodeId>> {
        info!("Building snapshot at {:?}", self.last_applied_log);

        let snapshot_id = format!(
            "{}-{}-{}",
            self.last_applied_log.map(|l| l.index).unwrap_or(0),
            self.last_applied_log.map(|l| l.leader_id.term).unwrap_or(0),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        let meta = SnapshotMeta {
            last_log_id: self.last_applied_log,
            last_membership: self.last_membership.clone(),
            snapshot_id,
        };

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(Vec::new())),
        })
    }
}
