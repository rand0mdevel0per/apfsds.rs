//! QUIC/HTTP3 Transport Implementation
//!
//! Provides high-performance, low-latency transport using QUIC protocol.
//! Used for Handler <-> Exit Node communication as an alternative to HTTP/2.

use anyhow::{Result, anyhow};
use quinn::{Endpoint, ClientConfig, ServerConfig, Connection};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, debug, error};

/// QUIC Transport Configuration
#[derive(Debug, Clone)]
pub struct QuicConfig {
    /// Certificate for TLS
    pub cert_der: Vec<u8>,
    /// Private key for TLS
    pub key_der: Vec<u8>,
    /// Skip certificate verification (for testing)
    pub skip_verify: bool,
}

/// QUIC Client for outgoing connections
pub struct QuicClient {
    endpoint: Endpoint,
}

impl QuicClient {
    /// Create a new QUIC client
    pub fn new(bind: SocketAddr, config: &QuicConfig) -> Result<Self> {
        let client_crypto = if config.skip_verify {
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
                .with_no_client_auth()
        } else {
            // TODO: Load CA certs
            rustls::ClientConfig::builder()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth()
        };

        let client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)?
        ));

        let mut endpoint = Endpoint::client(bind)?;
        endpoint.set_default_client_config(client_config);

        Ok(Self { endpoint })
    }

    /// Connect to a QUIC server
    pub async fn connect(&self, server_addr: SocketAddr, server_name: &str) -> Result<QuicConnection> {
        let connection = self.endpoint.connect(server_addr, server_name)?.await?;
        info!("QUIC connected to {}", server_addr);
        Ok(QuicConnection { connection })
    }
}

/// QUIC Server for incoming connections
pub struct QuicServer {
    endpoint: Endpoint,
}

impl QuicServer {
    /// Create a new QUIC server
    pub fn new(bind: SocketAddr, config: &QuicConfig) -> Result<Self> {
        let cert = CertificateDer::from(config.cert_der.clone());
        // Assume PKCS8 format for private key
        let key = PrivateKeyDer::Pkcs8(config.key_der.clone().into());

        let server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)?;

        let server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?
        ));

        let endpoint = Endpoint::server(server_config, bind)?;
        info!("QUIC server listening on {}", bind);

        Ok(Self { endpoint })
    }

    /// Accept incoming connection
    pub async fn accept(&self) -> Option<QuicConnection> {
        match self.endpoint.accept().await {
            Some(incoming) => {
                match incoming.await {
                    Ok(connection) => Some(QuicConnection { connection }),
                    Err(e) => {
                        error!("QUIC accept error: {}", e);
                        None
                    }
                }
            }
            None => None,
        }
    }
}

/// QUIC Connection wrapper
pub struct QuicConnection {
    connection: Connection,
}

impl QuicConnection {
    /// Send data over QUIC
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let mut stream = self.connection.open_uni().await?;
        stream.write_all(data).await?;
        stream.finish()?;
        Ok(())
    }

    /// Receive data from QUIC (uni stream)
    pub async fn recv(&self) -> Result<Vec<u8>> {
        let mut recv = self.connection.accept_uni().await?;
        let data = recv.read_to_end(1024 * 1024).await?; // 1MB limit
        Ok(data)
    }

    /// Open bidirectional stream
    pub async fn open_bi(&self) -> Result<(quinn::SendStream, quinn::RecvStream)> {
        Ok(self.connection.open_bi().await?)
    }

    /// Close connection
    pub fn close(&self) {
        self.connection.close(0u32.into(), b"done");
    }
}

/// Skip server certificate verification (for testing only!)
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}
