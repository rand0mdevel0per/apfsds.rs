//! APFSDS Daemon
//!
//! The server-side component that handles client connections,
//! authentication, and traffic forwarding.

mod config;
mod auth;
mod handler;
mod metrics;

use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use config::DaemonConfig;

/// APFSDS Daemon - Server-side proxy handler
#[derive(Parser, Debug)]
#[command(name = "apfsdsd")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "daemon.toml")]
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

    // Run appropriate mode
    if args.exit {
        info!("Starting as exit node");
        handler::run_exit(&config).await?;
    } else {
        info!("Starting as handler on {}", config.server.bind);
        handler::run_handler(&config).await?;
    }

    // Cleanup
    metrics_handle.abort();

    Ok(())
}
