//! APFSDS Control CLI
//!
//! Command-line interface for managing the APFSDS daemon.

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:25348")]
    api: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage users/accounts
    User {
        #[command(subcommand)]
        cmd: UserCommands,
    },
    /// Manage exit nodes
    Node {
        #[command(subcommand)]
        cmd: NodeCommands,
    },
    /// View system statistics
    Stats,
}

#[derive(Subcommand, Debug)]
enum UserCommands {
    /// Create a new user
    Create {
        /// Username
        username: String,
        /// Quota in bytes
        #[arg(long)]
        quota: Option<u64>,
    },
    /// Delete a user
    Delete {
        /// User ID
        id: u64,
    },
}

#[derive(Subcommand, Debug)]
enum NodeCommands {
    /// Register a new exit node
    Register {
        /// Name
        name: String,
        /// Endpoint (e.g., 1.2.3.4:25347)
        endpoint: String,
        /// Weight
        #[arg(long, default_value = "1.0")]
        weight: f64,
    },
}

#[derive(Debug, Serialize)]
struct CreateUserRequest {
    username: String,
    quota_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct RegisterNodeRequest {
    name: String,
    endpoint: String,
    weight: f64,
}

#[derive(Debug, Deserialize, Tabled)]
struct SystemStats {
    active_connections: usize,
    total_rx_bytes: u64,
    total_tx_bytes: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();

    match args.command {
        Commands::Stats => {
            let resp = client
                .get(format!("{}/admin/stats", args.api))
                .send()
                .await?
                .error_for_status()?;

            let stats: SystemStats = resp.json().await?;
            let table = tabled::Table::new(vec![stats]).to_string();
            println!("{}", table);
        }
        Commands::User { cmd } => match cmd {
            UserCommands::Create { username, quota } => {
                let req = CreateUserRequest {
                    username,
                    quota_bytes: quota,
                };
                let resp = client
                    .post(format!("{}/admin/users", args.api))
                    .json(&req)
                    .send()
                    .await?;

                if resp.status().is_success() {
                    println!("User created successfully");
                } else {
                    eprintln!("Error: {}", resp.status());
                }
            }
            UserCommands::Delete { id } => {
                let resp = client
                    .delete(format!("{}/admin/users/{}", args.api, id))
                    .send()
                    .await?;

                if resp.status().is_success() {
                    println!("User deleted successfully");
                } else {
                    eprintln!("Error: {}", resp.status());
                }
            }
        },
        Commands::Node { cmd } => match cmd {
            NodeCommands::Register {
                name,
                endpoint,
                weight,
            } => {
                let req = RegisterNodeRequest {
                    name,
                    endpoint,
                    weight,
                };
                let resp = client
                    .post(format!("{}/admin/nodes", args.api))
                    .json(&req)
                    .send()
                    .await?;

                if resp.status().is_success() {
                    println!("Node registered successfully");
                } else {
                    eprintln!("Error: {}", resp.status());
                }
            }
        },
    }

    Ok(())
}
