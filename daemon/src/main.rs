//! APFSDS Daemon
//!
//! The server-side component that handles client connections,
//! authentication, and traffic forwarding.

mod auth;
mod billing;
mod config;
mod connection_registry;
mod emergency;
mod exit_forwarder;
mod exit_node;
mod handler;
mod key_rotation;
mod metrics;
mod noise;
mod geoip;
mod management;
mod plugin;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use apfsds_raft::{Config as AsyncRaftConfig, RaftNode};
use apfsds_storage::postgres::PgClient;
use apfsds_transport::{ExitNodeDefinition, ExitPool, ExitPoolConfig};
use billing::BillingAggregator;
use config::DaemonConfig;
use exit_forwarder::ExitForwarder;

/// APFSDS Daemon - Server-side proxy handler
#[derive(Parser, Debug)]
#[command(name = "apfsdsd")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    /// Path to configuration file
    #[arg(short = 'f', long, default_value = "/etc/apfsds.d/cfg/master.toml")]
    config: String,

    /// Run in verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// Run as handler (default)
    #[arg(long)]
    handler: bool,

    /// Run as exit node
    #[arg(long)]
    exit: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("APFSDS Daemon v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = DaemonConfig::load(&args.config).await?;
    info!("Loaded configuration from {}", args.config);

    // Start metrics server
    let metrics_handle = metrics::start_server(&config.monitoring);

    // Initialize Database Client
    let pg_client = PgClient::new(&config.database.url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    // Auto-migrate
    info!("Migrating database...");
    pg_client
        .migrate()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to migrate database: {}", e))?;
    info!("Database migrated");

    // Initialize Billing Aggregator
    let billing = Arc::new(BillingAggregator::new(pg_client.clone()));
    let billing_handle = billing.clone().start();

    // Initialize Connection Registry
    let registry = connection_registry::ConnectionRegistry::new();

    // Initialize Raft Node (if Handler)
    let raft_node = if !args.exit {
        let raft_config = Arc::new(
            AsyncRaftConfig::build(format!("node-{}", config.raft.node_id))
                .validate()
                .map_err(|e| anyhow::anyhow!("Invalid raft config: {}", e))?,
        );
        let node = Arc::new(RaftNode::new(config.raft.node_id, raft_config));
        info!("Raft node initialized with ID: {}", config.raft.node_id);
        Some(node)
    } else {
        None
    };

    // Start Management API (Port 25348)
    let mgmt_bind = "0.0.0.0:25348".parse().unwrap();
    let mgmt_config = Arc::new(config.clone());
    let mgmt_registry = registry.clone();
    let mgmt_raft = raft_node.clone();
    
    tokio::spawn(async move {
        if let Err(e) = management::start_server(mgmt_bind, mgmt_config, mgmt_registry, mgmt_raft).await {
            tracing::error!("Management API error: {}", e);
        }
    });

    // Start Plugin System
    let plugin_socket = if cfg!(windows) {
        r"\\.\pipe\apfsds-plugin"
    } else {
        "/tmp/apfsds.sock"
    };
    let plugin_mgr = plugin::PluginManager::new(plugin_socket);
    tokio::spawn(async move {
        if let Err(e) = plugin_mgr.start().await {
            tracing::error!("Plugin Manager error: {}", e);
        }
    });

    // Run appropriate mode
    if args.exit {
        info!("Starting as exit node");
        exit_node::run(&config).await?;
    } else {
        // Initialize Exit Pool
        let exit_pool_config = ExitPoolConfig {
            exit_nodes: config
                .exit_nodes
                .iter()
                .map(|n| ExitNodeDefinition {
                    url: n.endpoint.clone(),
                    group_id: n.group_id,
                })
                .collect(),
            ..Default::default()
        };
        // Pass handler_id (node_id) and registry
        let exit_pool = Arc::new(ExitPool::new(
            exit_pool_config,
            config.raft.node_id,
            registry.clone(),
        )?);

        // Start background health checker
        let health_handle = exit_pool.clone().start_health_checker();

        // Initialize Exit Forwarder
        let exit_forwarder = Arc::new(ExitForwarder::new(exit_pool, config.raft.node_id));

        // Add peers from config
        if let Some(raft) = &raft_node {
             for peer in &config.raft.peers {
                info!("Configuring Raft peer: {}", peer);
                // In real impl, we might add them to the raft node here
             }
        }

        info!("Starting as handler on {}", config.server.bind);
        handler::run_handler(
            &config,
            exit_forwarder,
            raft_node.expect("Raft node missing in handler mode"),
            pg_client,
            billing,
            registry,
        )
        .await?;

        health_handle.abort();
    }

    // Cleanup
    metrics_handle.abort();
    billing_handle.abort();

    Ok(())
}
