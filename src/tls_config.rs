// src/tls_config.rs
use std::{fs::File, io::BufReader, sync::Arc};
use tokio_rustls::TlsAcceptor;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use anyhow::{Context, Result};

pub struct TlsConfig {
    acceptor: TlsAcceptor,
}

impl TlsConfig {
    /// Load TLS config and return an acceptor ready for HTTPS server.
    pub fn load(cert_path: &str, key_path: &str) -> Result<Self> {
        let cert_file = &mut BufReader::new(File::open(cert_path)
            .with_context(|| format!("opening certificate file '{}'", cert_path))?);
        let key_file = &mut BufReader::new(File::open(key_path)
            .with_context(|| format!("opening private key file '{}'", key_path))?);

        let cert_chain: Vec<Certificate> = certs(cert_file)?
            .into_iter()
            .map(Certificate)
            .collect();
        let mut keys = pkcs8_private_keys(key_file)?;
        let key = PrivateKey(keys.remove(0));

        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .context("building ServerConfig")?;

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(Self { acceptor })
    }

    pub fn into_acceptor(self) -> TlsAcceptor {
        self.acceptor
    }
}
