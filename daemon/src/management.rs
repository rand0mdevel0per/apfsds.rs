//! Management API for APFSDS Daemon
//!
//! Provides administration endpoints for managing:
//! - Users/Accounts
//! - Nodes (Exit Nodes)
//! - System Statistics

use crate::config::DaemonConfig;
use crate::connection_registry::ConnectionRegistry;
use apfsds_raft;
use anyhow::Result;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    response::Html,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

/// Management API Configuration
#[derive(Clone)]
struct AppState {
    config: Arc<DaemonConfig>,
    registry: Arc<ConnectionRegistry>,
    raft_node: Option<Arc<apfsds_raft::RaftNode>>,
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
    raft_node: Option<Arc<apfsds_raft::RaftNode>>,
) -> Result<()> {
    let state = AppState {
        config,
        registry,
        raft_node,
    };

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/admin/users", post(create_user))
        .route("/admin/users/:id", delete(delete_user))
        .route("/admin/nodes", post(register_node))
        .route("/admin/stats", get(get_stats))
        .route("/admin/cluster/membership", post(change_cluster_membership))
        .with_state(state);

    info!("Management API listening on {}", bind);
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Deserialize)]
struct MembershipRequest {
    members: Vec<u64>,
}

// Basic Dashboard Handler
async fn dashboard() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>APFSDS Dashboard</title>
    <style>body{font-family:sans-serif;background:#1a1b1e;color:#fff;padding:20px}.card{background:#25262b;padding:20px;margin-bottom:20px;border-radius:8px}</style>
</head>
<body>
    <div class="card">
        <h1>APFSDS Dashboard</h1>
        <p>System is running.</p>
        <p><a href="/metrics">Prometheus Metrics</a></p>
    </div>
</body>
</html>"#)
}

async fn change_cluster_membership(
    State(state): State<AppState>,
    Json(payload): Json<MembershipRequest>,
) -> Json<serde_json::Value> {
    if let Some(raft) = &state.raft_node {
        let members: std::collections::HashSet<u64> = payload.members.into_iter().collect();
        match raft.change_membership(members).await {
            Ok(_) => Json(serde_json::json!({ "status": "success", "message": "Membership change initiated" })),
            Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
        }
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Raft node not initialized" }))
    }
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
