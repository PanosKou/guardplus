use serde::Deserialize;
use std::{fs::File, io::BufReader, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to open config file '{path}' : {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to parse YAML config: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub http_port: u16,
    /// Optional HTTPS port; if absent we’ll default to http_port + 1
    pub https_port: Option<u16>,
    /// Optional gRPC port; if absent we’ll default to 50051
    pub grpc_port: Option<u16>,
    /// Optional TCP proxy port; if absent we’ll default to 91000
    pub tcp_port: Option<u16>,
    /// Optional UDP proxy port; if absent we’ll default to 92000
    pub udp_port: Option<u16>,

    pub auth: Auth,
    pub tls: Tls,
    pub backends: Vec<Backend>,
    pub consul_url: String,
    pub tls_mode: String,
    // these two you already have under `tls: Tls`
    // pub tls_cert_path: String,
    // pub tls_key_path: String,
    pub tls_domain: String,
    pub tls_email: String,
    pub bearer_token: String,
    pub rate_limit_per_sec: u32,
    pub rate_limit_burst: u32,
}
#[derive(Debug, Deserialize)]
pub struct Auth {
    pub oidc_providers: Vec<OidcProvider>,
}

#[derive(Debug, Deserialize)]
pub struct OidcProvider {
    pub name: String,
    pub issuer_url: String,
    pub audience: String,
}

#[derive(Debug, Deserialize)]
pub struct Tls {
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize)]
pub struct Backend {
    pub name: String,
    pub protocol: String,
    pub address: String,
    pub routes: Vec<String>,
}

impl Config {
    /// Load and parse configuration from the given YAML file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let p = path.as_ref();
        let file = File::open(p).map_err(|e| ConfigError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        let reader = BufReader::new(file);
        let cfg = serde_yaml::from_reader(reader)?; // uses serde_yaml::from_reader :contentReference[oaicite:0]{index=0}
        Ok(cfg)
    }
}
