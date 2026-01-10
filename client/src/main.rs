//! APFSDS Client
//!
//! A high-performance proxy client with TUN support.

mod config;
mod emergency;
mod socks5;
// mod tun_device; // TODO: Implement TUN support

use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use config::ClientConfig;

/// APFSDS Client - Privacy-preserving network proxy
#[derive(Parser, Debug)]
#[command(name = "apfsds")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Run in verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// Run in SOCKS5 mode (default)
    #[arg(long)]
    socks5: bool,

    /// Run in TUN mode
    #[arg(long)]
    tun: bool,
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

    info!("APFSDS Client v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = ClientConfig::load(&args.config).await?;
    info!("Loaded configuration from {}", args.config);

    // Start emergency mode checker
    let emergency_handle = emergency::start_checker(config.emergency.clone());

    // Run appropriate mode
    if args.tun {
        info!("Starting in TUN mode");
        // TODO: Implement TUN mode
        anyhow::bail!("TUN mode not yet implemented");
    } else {
        info!("Starting in SOCKS5 mode on {}", config.socks5.bind);
        socks5::run(&config).await?;
    }

    // Cleanup
    emergency_handle.abort();

    Ok(())
}
