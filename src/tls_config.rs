// src/tls_config.rs

use std::{
    fs,
    io::{self},
    path::Path,
    sync::Arc,
};
use pem::Pem;
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};

/// Holds the Tokio-Rustls acceptor for your HTTPS gateway.
pub struct TlsConfig {
    pub acceptor: TlsAcceptor,
}

impl TlsConfig {
    /// Load certificate chain and private key from PEM files,
    /// and return a configured `TlsAcceptor`.
    ///
    /// This version uses the `pem` crate to collect all PEM blocks,
    /// finds the ones tagged CERTIFICATE and PRIVATE KEY, and
    /// bails out with an `io::Error` if anything is missing or malformed.
    pub fn load<P: AsRef<Path>>(cert_path: P, key_path: P) -> io::Result<Self> {
        // Read entire cert file
        let cert_bytes = fs::read(&cert_path)?;
        // Parse all PEM blocks
        let certs_pem = pem::parse_many(&cert_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("PEM parse error (certs): {}", e)))?;
        // Filter CERTIFICATE blocks
        let certs_der: Vec<Certificate> = certs_pem
            .into_iter()
            .filter(|block: &Pem| block.tag() == "CERTIFICATE")
            .map(|block| {
                let bytes = block.contents().to_vec();
                Certificate(bytes)
            })
            .collect();
        if certs_der.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No CERTIFICATE blocks found in {:?}", cert_path.as_ref()),
            ));
        }

        // Read entire key file
        let key_bytes = fs::read(&key_path)?;
        // Parse all PEM blocks
        let keys_pem = pem::parse_many(&key_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("PEM parse error (keys): {}", e)))?;
        // Filter any PRIVATE KEY (PKCS#8 or RSA)
        let mut keys_der: Vec<PrivateKey> = keys_pem
            .into_iter()
            .filter(|block: &Pem| block.tag().ends_with("PRIVATE KEY"))
            .map(|block| {
                // use the public getter `.contents()` and clone into Vec<u8>
                let bytes = block.contents().to_vec();
                PrivateKey(bytes)
            })
            .collect();
        if keys_der.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("No PRIVATE KEY blocks found in {:?}", key_path.as_ref()),
            ));
        }
        // Take the first key
        let priv_key = keys_der.remove(0);

        // Build the rustls ServerConfig
        let mut config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs_der, priv_key)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("TLS config error: {}", e)))?;

        // ALPN
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(TlsConfig { acceptor })
    }
}
