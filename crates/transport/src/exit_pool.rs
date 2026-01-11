//! Exit node pool with health checking and load balancing
//!
//! Manages multiple exit nodes and distributes traffic.

use crate::exit_client::{ExitClient, ExitClientConfig, ExitClientError, SharedExitClient};
use apfsds_protocol::PlainPacket;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::SharedPacketDispatcher;
use std::collections::HashMap;

/// Definition of an exit node
#[derive(Debug, Clone)]
pub struct ExitNodeDefinition {
    pub url: String,
    pub group_id: i32,
}

/// Configuration for exit pool
#[derive(Debug, Clone)]
pub struct ExitPoolConfig {
    /// List of exit nodes
    pub exit_nodes: Vec<ExitNodeDefinition>,

    /// Health check interval
    pub health_check_interval: Duration,

    /// Per-client timeout
    pub client_timeout: Duration,

    /// Use HTTP/2
    pub http2: bool,
}

impl Default for ExitPoolConfig {
    fn default() -> Self {
        Self {
            exit_nodes: vec![ExitNodeDefinition {
                url: "http://127.0.0.1:8081".into(),
                group_id: 0,
            }],
            health_check_interval: Duration::from_secs(10),
            client_timeout: Duration::from_secs(10),
            http2: true,
        }
    }
}

/// Pool of exit node clients for a specific group
pub struct GroupPool {
    clients: Vec<SharedExitClient>,
    next_index: AtomicUsize,
}

/// Pool of exit node clients with load balancing
pub struct ExitPool {
    groups: RwLock<HashMap<i32, GroupPool>>,
    config: ExitPoolConfig,
    dispatcher: SharedPacketDispatcher,
    handler_id: u64,
}

impl ExitPool {
    /// Create a new exit pool
    pub fn new(
        config: ExitPoolConfig,
        handler_id: u64,
        dispatcher: SharedPacketDispatcher,
    ) -> Result<Self, ExitClientError> {
        let mut groups_map: HashMap<i32, Vec<SharedExitClient>> = HashMap::new();

        for node_def in &config.exit_nodes {
            let client_config = ExitClientConfig {
                base_url: node_def.url.clone(),
                timeout: config.client_timeout,
                http2: config.http2,
            };

            let client = Arc::new(ExitClient::new(client_config)?);
            // Start return traffic subscription
            client.clone().subscribe(handler_id, dispatcher.clone());

            groups_map
                .entry(node_def.group_id)
                .or_default()
                .push(client);
        }

        let mut groups = HashMap::new();
        for (id, clients) in groups_map {
            groups.insert(
                id,
                GroupPool {
                    clients,
                    next_index: AtomicUsize::new(0),
                },
            );
        }

        info!("Created exit pool with {} groups", groups.len());

        Ok(Self {
            groups: RwLock::new(groups),
            config,
            dispatcher,
            handler_id,
        })
    }

    /// Forward a packet using round-robin selection within a group
    pub async fn forward(
        &self,
        packet: &PlainPacket,
        group_id: i32,
    ) -> Result<(), ExitClientError> {
        let groups = self.groups.read().await;

        // Fallback to default group 0 if requested group doesn't exist
        let group = groups.get(&group_id).or_else(|| groups.get(&0));

        let group = match group {
            Some(g) => g,
            None => {
                return Err(ExitClientError::ConnectionFailed(format!(
                    "Group {} not found and no default group",
                    group_id
                )));
            }
        };

        if group.clients.is_empty() {
            return Err(ExitClientError::ConnectionFailed(
                "No exit nodes available in group".to_string(),
            ));
        }

        // Round-robin with health awareness
        let start_index = group.next_index.fetch_add(1, Ordering::Relaxed) % group.clients.len();
        let mut attempts = 0;

        while attempts < group.clients.len() {
            let index = (start_index + attempts) % group.clients.len();
            let client = &group.clients[index];

            if client.is_healthy() {
                match client.forward(packet).await {
                    Ok(()) => {
                        debug!(
                            "Forwarded via exit node {} (Group {})",
                            client.base_url(),
                            group_id
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        warn!("Exit node {} failed: {}", client.base_url(), e);
                        attempts += 1;
                    }
                }
            } else {
                attempts += 1;
            }
        }

        Err(ExitClientError::ConnectionFailed(
            "All exit nodes failed".to_string(),
        ))
    }

    /// Run health check on all nodes
    pub async fn health_check_all(&self) {
        let groups = self.groups.read().await;
        let mut healthy_count = 0;
        let mut total_count = 0;

        for group in groups.values() {
            for client in &group.clients {
                if client.health_check().await {
                    healthy_count += 1;
                } else {
                    warn!("Exit node {} is unhealthy", client.base_url());
                }
                total_count += 1;
            }
        }

        debug!("{}/{} exit nodes healthy", healthy_count, total_count);
    }

    /// Start background health checker
    pub fn start_health_checker(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;
                self.health_check_all().await;
            }
        })
    }

    /// Get count of healthy nodes
    pub async fn healthy_count(&self) -> usize {
        let groups = self.groups.read().await;
        let mut count = 0;
        for group in groups.values() {
            count += group.clients.iter().filter(|c| c.is_healthy()).count();
        }
        count
    }

    /// Get total node count
    pub async fn total_count(&self) -> usize {
        let groups = self.groups.read().await;
        groups.values().map(|g| g.clients.len()).sum()
    }

    /// Add a new exit node dynamically
    pub async fn add_node(&self, url: String, group_id: i32) -> Result<(), ExitClientError> {
        let client_config = ExitClientConfig {
            base_url: url.clone(),
            timeout: self.config.client_timeout,
            http2: self.config.http2,
        };

        let client = Arc::new(ExitClient::new(client_config)?);
        // Start subscription
        client
            .clone()
            .subscribe(self.handler_id, self.dispatcher.clone());

        let mut groups = self.groups.write().await;

        let group = groups.entry(group_id).or_insert_with(|| GroupPool {
            clients: Vec::new(),
            next_index: AtomicUsize::new(0),
        });

        group.clients.push(client);

        info!("Added exit node: {} to Group {}", url, group_id);
        Ok(())
    }
}
