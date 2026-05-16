use serde::Deserialize;
use std::{collections::HashSet, fs::File, io::BufReader, path::Path};
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
    pub https_port: Option<u16>,
    pub grpc_port: Option<u16>,
    pub tcp_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub http_bind_addr: Option<String>,
    pub https_bind_addr: Option<String>,
    pub grpc_bind_addr: Option<String>,

    pub auth: Auth,
    pub tls: Tls,
    pub backends: Vec<Backend>,
    pub consul_url: String,
    pub tls_mode: String,
    #[allow(dead_code)]
    pub tls_domain: String,
    #[allow(dead_code)]
    pub tls_email: String,
    pub bearer_token: Option<String>,
    pub rate_limit_per_sec: u32,
    pub rate_limit_burst: u32,
    #[serde(default)]
    pub proxy: ProxyConfig,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Auth {
    #[serde(default)]
    pub oidc_providers: Vec<OidcProvider>,
    pub cloudflare_jwt_secret: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OidcProvider {
    pub name: String,
    pub issuer_url: String,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub routes: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProxyConfig {
    #[serde(default = "default_upstream")]
    pub upstream: String,
    #[serde(default)]
    pub endpoint_allowlist: Vec<String>,
    #[serde(default = "default_endpoint_denylist")]
    pub endpoint_denylist: Vec<String>,
    #[serde(default)]
    pub model_allowlist: HashSet<String>,
    #[serde(default = "default_max_body")]
    pub max_body_bytes: usize,
    #[serde(default = "default_max_prompt")]
    pub max_prompt_chars: usize,
    #[serde(default = "default_max_num_ctx")]
    pub max_num_ctx: u64,
    #[serde(default = "default_max_num_predict")]
    pub max_num_predict: i64,
}

fn default_upstream() -> String {
    std::env::var("GAMB_UPSTREAM").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
}
fn default_endpoint_denylist() -> Vec<String> {
    vec![
        "/api/pull".to_string(),
        "/api/create".to_string(),
        "/api/copy".to_string(),
        "/api/push".to_string(),
        "DELETE /api/delete".to_string(),
    ]
}
fn default_max_body() -> usize { 1_048_576 }
fn default_max_prompt() -> usize { 32_000 }
fn default_max_num_ctx() -> u64 { 32768 }
fn default_max_num_predict() -> i64 { 4096 }

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let p = path.as_ref();
        let file = File::open(p).map_err(|e| ConfigError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        let reader = BufReader::new(file);
        let cfg = serde_yaml::from_reader(reader)?;
        Ok(cfg)
    }
}


impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            upstream: default_upstream(),
            endpoint_allowlist: vec![],
            endpoint_denylist: default_endpoint_denylist(),
            model_allowlist: std::collections::HashSet::new(),
            max_body_bytes: default_max_body(),
            max_prompt_chars: default_max_prompt(),
            max_num_ctx: default_max_num_ctx(),
            max_num_predict: default_max_num_predict(),
        }
    }
}
