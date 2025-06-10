use serde::Deserialize;
use std::{fs::File, io::BufReader, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to open config file '{path}' : {source}")]
    Io { path: String, source: std::io::Error },

    #[error("failed to parse YAML config: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub http_port: u16,
    pub auth: Auth,
    pub tls: Tls,
    pub backends: Vec<Backend>,
    pub consul_url: String,
    pub tls_mode: String,
    pub tls_cert_path: String,
    pub tls_key_path: String,
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
        let file = File::open(p)
            .map_err(|e| ConfigError::Io { path: p.display().to_string(), source: e })?;
        let reader = BufReader::new(file);
        let cfg = serde_yaml::from_reader(reader)?; // uses serde_yaml::from_reader :contentReference[oaicite:0]{index=0}
        Ok(cfg)
    }
}
