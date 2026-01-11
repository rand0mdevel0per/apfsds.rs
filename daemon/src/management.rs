//! Management API for APFSDS Daemon
//!
//! Provides administration endpoints for managing:
//! - Users/Accounts
//! - Nodes (Exit Nodes)
//! - System Statistics

use crate::config::DaemonConfig;
use crate::connection_registry::ConnectionRegistry;
use crate::metrics::Metrics;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};

/// Management API Configuration
#[derive(Clone)]
struct AppState {
    config: Arc<DaemonConfig>,
    registry: Arc<ConnectionRegistry>,
    // pg_client: PgClient, // TODO: Require PgClient for user management
}

/// Create User Request
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub quota_bytes: Option<u64>,
}

/// Register Node Request
#[derive(Debug, Deserialize)]
pub struct RegisterNodeRequest {
    pub name: String,
    pub endpoint: String,
    pub weight: f64,
}

/// System Statistics
#[derive(Debug, Serialize)]
pub struct SystemStats {
    pub active_connections: usize,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
}

/// Start the Management API server
pub async fn start_server(
    bind: SocketAddr,
    config: Arc<DaemonConfig>,
    registry: Arc<ConnectionRegistry>,
) -> Result<()> {
    let state = AppState {
        config,
        registry,
    };

    let app = Router::new()
        .route("/admin/users", post(create_user))
        .route("/admin/users/:id", delete(delete_user))
        .route("/admin/nodes", post(register_node))
        .route("/admin/stats", get(get_stats))
        .with_state(state);

    info!("Management API listening on {}", bind);
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn create_user(
    State(_state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse {
    info!("Create user request: {:?}", payload);
    // TODO: Insert into database via PgClient
    (StatusCode::CREATED, Json("User created"))
}

async fn delete_user(
    State(_state): State<AppState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    info!("Delete user request: {}", id);
    // TODO: Delete from database
    (StatusCode::NO_CONTENT, Json("User deleted"))
}

async fn register_node(
    State(_state): State<AppState>,
    Json(payload): Json<RegisterNodeRequest>,
) -> impl IntoResponse {
    info!("Register node request: {:?}", payload);
    // TODO: Add to dynamic config / Raft
    (StatusCode::CREATED, Json("Node registered"))
}

async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    // Basic stats from registry
    let stats = SystemStats {
        active_connections: state.registry.count(),
        total_rx_bytes: 0, // Placeholder
        total_tx_bytes: 0, // Placeholder
    };
    (StatusCode::OK, Json(stats))
}
