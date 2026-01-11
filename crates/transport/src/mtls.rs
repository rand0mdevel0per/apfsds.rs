//! mTLS (Mutual TLS) for secure inter-node communication
//!
//! Provides TLS encryption and mutual authentication between
//! Handler and Exit nodes.

use anyhow::{Result, anyhow};
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio_rustls::{TlsAcceptor, TlsConnector, rustls};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tracing::{debug, info};

/// mTLS configuration
#[derive(Debug, Clone)]
pub struct MtlsConfig {
    /// Path to CA certificate (PEM)
    pub ca_path: String,
    /// Path to server/client certificate (PEM)
    pub cert_path: String,
    /// Path to private key (PEM)
    pub key_path: String,
}

/// Load certificates from PEM file
fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

/// Load private key from PEM file
fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // Try PKCS8 first, then RSA
    let keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    
    if let Some(key) = keys.into_iter().next() {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    
    // Reset reader and try RSA format
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let keys: Vec<_> = rustls_pemfile::rsa_private_keys(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    
    keys.into_iter()
        .next()
        .map(PrivateKeyDer::Pkcs1)
        .ok_or_else(|| anyhow!("No private key found"))
}

/// Create a TLS acceptor for the server side
pub fn create_acceptor(config: &MtlsConfig) -> Result<TlsAcceptor> {
    let certs = load_certs(Path::new(&config.cert_path))?;
    let key = load_key(Path::new(&config.key_path))?;
    
    // Load CA for client verification
    let ca_certs = load_certs(Path::new(&config.ca_path))?;
    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert)?;
    }
    
    // Build server config with client authentication
    let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()?;
    
    let server_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)?;
    
    info!("mTLS acceptor created");
    Ok(TlsAcceptor::from(Arc::new(server_config)))
}

/// Create a TLS connector for the client side
pub fn create_connector(config: &MtlsConfig) -> Result<TlsConnector> {
    let certs = load_certs(Path::new(&config.cert_path))?;
    let key = load_key(Path::new(&config.key_path))?;
    
    // Load CA for server verification
    let ca_certs = load_certs(Path::new(&config.ca_path))?;
    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert)?;
    }
    
    // Build client config with client certificate
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(certs, key)?;
    
    info!("mTLS connector created");
    Ok(TlsConnector::from(Arc::new(client_config)))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: Full tests require certificate files
    // These are placeholder tests
    
    #[test]
    fn test_mtls_config() {
        let config = MtlsConfig {
            ca_path: "/path/to/ca.pem".to_string(),
            cert_path: "/path/to/cert.pem".to_string(),
            key_path: "/path/to/key.pem".to_string(),
        };
        
        assert!(!config.ca_path.is_empty());
    }
}
