//! SSH Transport Implementation
//!
//! Provides SSH tunneling capabilities for censorship resistance.
//! Uses `russh` (pure Rust SSH implementation).

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use russh::{
    client, server, MethodSet, ChannelId, Channel,
};
use russh_keys::key::KeyPair;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tracing::{info, error, debug};

// ==================== Client ====================

struct ClientHandler;

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // In this specific fallback mode, we might blindly trust the key 
        // OR verify it against a known pinned key.
        // For simplicity/fallback, we return true (trust on first use / pinned logic elsewhere)
        Ok(true)
    }
}

pub struct SshClient {
    session: client::Handle<ClientHandler>,
}

impl SshClient {
    pub async fn connect(addr: SocketAddr, user: &str, key: KeyPair) -> Result<Self> {
        let config = client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        };
        let config = Arc::new(config);
        
        let sh = ClientHandler;
        let mut session = client::connect(config, addr, sh).await?;
        
        // Authenticate
        let auth_res = session.authenticate_publickey(user, Arc::new(key)).await?;
        if !auth_res {
            return Err(anyhow!("SSH authentication failed"));
        }
        
        info!("SSH connected to {}", addr);
        Ok(Self { session })
    }

    pub async fn open_tunnel(&mut self) -> Result<Channel<client::Msg>> {
        let channel = self.session.channel_open_session().await?;
        Ok(channel)
    }
}

// ==================== Server ====================

#[derive(Clone)]
struct ServerHandler;

impl server::Server for ServerHandler {
    type Handler = Self;
    
    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self {
        Self
    }
}

#[async_trait]
impl server::Handler for ServerHandler {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &russh_keys::key::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        // Security Note: In production, validate public key against authorized keys registry
        // For development/testing, accept all keys
        tracing::warn!("SSH: Accepting all public keys (production should validate)");
        Ok(server::Auth::Accept)
    }
}

pub struct SshServer {
    config: Arc<server::Config>,
    listener: tokio::net::TcpListener,
}

impl SshServer {
    pub async fn new(bind: SocketAddr, key: KeyPair) -> Result<Self> {
        let mut config = server::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(30)),
            auth_rejection_time: std::time::Duration::from_secs(1),
            ..Default::default()
        };
        config.keys.push(key);
        let config = Arc::new(config);

        let listener = tokio::net::TcpListener::bind(bind).await?;
        info!("SSH server listening on {}", bind);

        Ok(Self { config, listener })
    }

    pub async fn accept(&self) -> Result<()> {
        let (stream, addr) = self.listener.accept().await?;
        info!("SSH incoming connection from {}", addr);
        
        let config = self.config.clone();
        tokio::spawn(async move {
            let handler = ServerHandler;
            if let Err(e) = russh::server::run_stream(config, stream, handler).await {
                error!("SSH session error: {}", e);
            }
        });
        Ok(())
    }
}
