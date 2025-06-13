// src/tls_config.rs

use std::{
    fs::File,
    io::{self, BufReader},
    sync::Arc,
};
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};
use rustls_pemfile::{certs, pkcs8_private_keys};

/// Holds the Tokio‐Rustls acceptor for your HTTPS gateway.
pub struct TlsConfig {
    pub acceptor: TlsAcceptor,
}

impl TlsConfig {
    /// Load certificate chain and private key from PEM files,
    /// and return a configured `TlsAcceptor`.
    pub fn load(cert_path: &str, key_path: &str) -> io::Result<Self> {
        // 1) Read and parse the certificate chain
        let cert_file = File::open(cert_path)?;
        let mut cert_reader = BufReader::new(cert_file);

        // rustls-pemfile::certs returns io::Result<Vec<Vec<u8>>>
        let raw_certs = certs(&mut cert_reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("TLS cert parse error: {}", e)))?;
        if raw_certs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No certificates found in cert_path",
            ));
        }
        // Wrap into rustls::Certificate
        let certs = raw_certs.into_iter().map(Certificate).collect::<Vec<_>>();

        // 2) Read and parse the private key(s)
        let key_file = File::open(key_path)?;
        let mut key_reader = BufReader::new(key_file);

        // rustls-pemfile::pkcs8_private_keys returns io::Result<Vec<Vec<u8>>>
        let raw_keys = pkcs8_private_keys(&mut key_reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("TLS key parse error: {}", e)))?;
        if raw_keys.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No private keys found in key_path",
            ));
        }
        // Wrap into rustls::PrivateKey
        let mut keys = raw_keys.into_iter().map(PrivateKey).collect::<Vec<_>>();
        let key = keys.remove(0);

        // 3) Build rustls ServerConfig
        let mut config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("TLS config error: {}", e)))?;

        // Optional: enable ALPN for HTTP/2 and HTTP/1.1
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        // 4) Create Tokio‐Rustls acceptor
        let acceptor = TlsAcceptor::from(Arc::new(config));

        Ok(TlsConfig { acceptor })
    }
}
