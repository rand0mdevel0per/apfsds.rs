//! APFSDS Client
//!
//! A high-performance proxy client with TUN support.

use anyhow::Result;
use clap::Parser;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use apfsds_client::config::ClientConfig;
use apfsds_client::{emergency, socks5};

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
        apfsds_client::run_tun(&config).await?;
    } else {
        // Start Local DNS service in background
        let config_dns = config.clone();
        tokio::spawn(async move {
            if let Err(e) = apfsds_client::local_dns::run(&config_dns).await {
                tracing::error!("Local DNS service failed: {}", e);
            }
        });

        info!("Starting in SOCKS5 mode on {}", config.socks5.bind);
        socks5::run(&config).await?;
    }

    // Cleanup
    emergency_handle.abort();

    Ok(())
}
