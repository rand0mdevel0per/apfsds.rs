//! ClickHouse backup client for connection state persistence
//!
//! This module provides optional ClickHouse integration for backing up
//! connection state. The client is enabled via configuration, not feature flags.

use apfsds_protocol::ConnMeta;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// ClickHouse client errors
#[derive(Error, Debug)]
pub enum ClickHouseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Client not enabled")]
    NotEnabled,
}

/// ClickHouse client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    /// Enable ClickHouse backup
    pub enabled: bool,

    /// ClickHouse server URL
    pub url: String,

    /// Database name
    pub database: String,

    /// Table name for connection records
    pub table: String,

    /// Username (optional)
    pub username: Option<String>,

    /// Password (optional)
    pub password: Option<String>,

    /// Batch size for bulk inserts
    pub batch_size: usize,

    /// Flush interval
    pub flush_interval: Duration,
}

impl Default for ClickHouseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:8123".to_string(),
            database: "apfsds".to_string(),
            table: "connections".to_string(),
            username: None,
            password: None,
            batch_size: 1000,
            flush_interval: Duration::from_secs(5),
        }
    }
}

/// Connection record for ClickHouse storage
#[derive(Debug, Clone, Serialize, clickhouse::Row)]
pub struct ConnectionRecord {
    pub conn_id: u64,
    pub client_addr: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub assigned_pod: u32,
    pub created_at: u64,
}

impl ConnectionRecord {
    pub fn from_conn_meta(conn_id: u64, meta: &ConnMeta, timestamp: u64) -> Self {
        let client_addr = format!(
            "{}.{}.{}.{}",
            meta.client_addr[12], meta.client_addr[13], meta.client_addr[14], meta.client_addr[15]
        );

        Self {
            conn_id,
            client_addr,
            local_port: meta.nat_entry.0,
            remote_port: meta.nat_entry.1,
            assigned_pod: meta.assigned_pod,
            created_at: timestamp,
        }
    }
}

/// ClickHouse backup client
pub struct ClickHouseBackup {
    client: Option<clickhouse::Client>,
    config: ClickHouseConfig,
    buffer: RwLock<Vec<ConnectionRecord>>,
    raft_buffer: RwLock<Vec<RaftLogRecord>>,
}

impl ClickHouseBackup {
    /// Create a new ClickHouse backup client
    pub fn new(config: ClickHouseConfig) -> Result<Self, ClickHouseError> {
        let client = if config.enabled {
            let mut builder = clickhouse::Client::default().with_url(&config.url);

            if let Some(ref user) = config.username {
                builder = builder.with_user(user);
            }
            if let Some(ref pass) = config.password {
                builder = builder.with_password(pass);
            }

            builder = builder.with_database(&config.database);

            info!("ClickHouse backup enabled: {}", config.url);
            Some(builder)
        } else {
            info!("ClickHouse backup disabled");
            None
        };

        Ok(Self {
            client,
            config,
            buffer: RwLock::new(Vec::new()),
            raft_buffer: RwLock::new(Vec::new()),
        })
    }

    /// Check if backup is enabled
    pub fn is_enabled(&self) -> bool {
        self.client.is_some()
    }

    /// Record a new connection
    pub async fn record_connection(
        &self,
        conn_id: u64,
        meta: &ConnMeta,
    ) -> Result<(), ClickHouseError> {
        if !self.is_enabled() {
            return Ok(()); // Silently skip if not enabled
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let record = ConnectionRecord::from_conn_meta(conn_id, meta, timestamp);

        let mut buffer = self.buffer.write().await;
        buffer.push(record);

        // Check if we should flush
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // Release lock before flush
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush buffered records to ClickHouse
    pub async fn flush(&self) -> Result<usize, ClickHouseError> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(0),
        };

        let mut buffer = self.buffer.write().await;
        if buffer.is_empty() {
            return Ok(0);
        }

        let records: Vec<_> = buffer.drain(..).collect();
        let count = records.len();
        drop(buffer); // Release lock before insert

        debug!("Flushing {} records to ClickHouse", count);

        let mut insert = client
            .insert(&self.config.table)
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        for record in records {
            insert
                .write(&record)
                .await
                .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;
        }

        insert
            .end()
            .await
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        info!("Flushed {} records to ClickHouse", count);
        Ok(count)
    }

    /// Start background flush task
    pub fn start_flush_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.config.flush_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;
                if let Err(e) = self.flush().await {
                    warn!("ClickHouse flush error: {}", e);
                }
            }
        })
    }

    /// Create table if not exists
    pub async fn ensure_table(&self) -> Result<(), ClickHouseError> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(()),
        };

        let ddl = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.{} (
                conn_id UInt64,
                client_addr String,
                local_port UInt16,
                remote_port UInt16,
                assigned_pod UInt32,
                created_at DateTime64(3)
            ) ENGINE = MergeTree()
            ORDER BY (created_at, conn_id)
            TTL toDateTime(created_at) + INTERVAL 7 DAY
            "#,
            self.config.database, self.config.table
        );

        client
            .query(&ddl)
            .execute()
            .await
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        info!(
            "ClickHouse table ensured: {}.{}",
            self.config.database, self.config.table
        );
        Ok(())
    }

    /// Get buffered record count
    pub async fn buffered_count(&self) -> usize {
        self.buffer.read().await.len()
    }

    /// Record a raft log entry
    pub async fn archive_raft_log(
        &self,
        index: u64,
        term: u64,
        operation: &str,
        payload: &str,
    ) -> Result<(), ClickHouseError> {
        if !self.is_enabled() {
            return Ok(());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let record = RaftLogRecord {
            index,
            term,
            operation: operation.to_string(),
            payload: payload.to_string(),
            created_at: timestamp,
        };

        let mut buffer = self.raft_buffer.write().await;
        buffer.push(record);

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            self.flush_raft_logs().await?;
        }

        Ok(())
    }

    /// Flush buffered raft logs
    pub async fn flush_raft_logs(&self) -> Result<usize, ClickHouseError> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(0),
        };

        let mut buffer = self.raft_buffer.write().await;
        if buffer.is_empty() {
            return Ok(0);
        }

        let records: Vec<_> = buffer.drain(..).collect();
        let count = records.len();
        drop(buffer);

        let table_name = format!("{}_logs", self.config.table);

        let mut insert = client
            .insert(&table_name)
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        for record in records {
            insert
                .write(&record)
                .await
                .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;
        }

        insert
            .end()
            .await
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        Ok(count)
    }

    /// Create tables if not exists
    pub async fn ensure_tables(&self) -> Result<(), ClickHouseError> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(()),
        };

        // Ensure connections table
        self.ensure_table().await?;

        // Ensure raft logs table
        let table_name = format!("{}_logs", self.config.table);
        let ddl = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {}.{} (
                index UInt64,
                term UInt64,
                operation String,
                payload String,
                created_at DateTime64(3)
            ) ENGINE = MergeTree()
            ORDER BY (created_at, index)
            TTL toDateTime(created_at) + INTERVAL 30 DAY
            "#,
            self.config.database, table_name
        );

        client
            .query(&ddl)
            .execute()
            .await
            .map_err(|e| ClickHouseError::QueryFailed(e.to_string()))?;

        info!(
            "ClickHouse table ensured: {}.{}",
            self.config.database, table_name
        );
        Ok(())
    }
}

/// Raft log record for ClickHouse storage
#[derive(Debug, Clone, Serialize, clickhouse::Row)]
pub struct RaftLogRecord {
    pub index: u64,
    pub term: u64,
    pub operation: String,
    pub payload: String,
    pub created_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_disabled() {
        let config = ClickHouseConfig::default();
        assert!(!config.enabled);
    }

    #[tokio::test]
    async fn test_disabled_client() {
        let config = ClickHouseConfig::default();
        let backup = ClickHouseBackup::new(config).unwrap();
        assert!(!backup.is_enabled());
    }
}
