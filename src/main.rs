#![allow(dead_code, unused_variables, unused_imports)]

pub mod echo {
    tonic::include_proto!("echo");
}
mod backend_registry;
mod config;
mod consul_integration;
mod grpc_service;
mod http_proxy;
mod middleware;
mod tcp_udp_proxy;
mod tls_config;

use backend_registry::BackendRegistry;
use config::Config;
use log::{error, info};
use std::{error::Error, net::SocketAddr, sync::Arc, time::Duration};
use tls_config::TlsConfig;
use tokio::spawn;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let config_path = std::env::var("GATEWAY_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());
    let cfg = Config::from_file(&config_path).map_err(|e| { error!("Config load failed: {}", e); e })?;

    let tls_cfg = TlsConfig::load(&cfg.tls.cert_path, &cfg.tls.key_path).map_err(|e| { error!("TLS load failed: {}", e); e })?;
    let tls_acceptor = tls_cfg.acceptor.clone();
    let registry = Arc::new(BackendRegistry::new());
    for be in &cfg.backends { registry.register(&be.name, &be.address); }

    let bearer = cfg.bearer_token.clone();
    let rate_limit = cfg.rate_limit_per_sec as u64;
    let rate_period = Duration::from_secs(1);
    let https_port = cfg.https_port.unwrap_or(cfg.http_port + 1);

    {
        let reg = registry.clone(); let acceptor = tls_acceptor.clone(); let auth = bearer.clone(); let auth_cfg = cfg.auth.clone(); let proxy = cfg.proxy.clone();
        let bind = cfg.https_bind_addr.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        spawn(async move {
            let addr: SocketAddr = format!("{}:{}", bind, https_port).parse().expect("invalid HTTPS addr");
            http_proxy::run_https_gateway(addr, reg, acceptor, auth, auth_cfg, proxy, rate_limit, rate_period).await;
        });
    }
    {
        let reg = registry.clone(); let auth = bearer.clone(); let auth_cfg = cfg.auth.clone(); let proxy = cfg.proxy.clone(); let http_port = cfg.http_port;
        let bind = cfg.http_bind_addr.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        spawn(async move {
            let addr: SocketAddr = format!("{}:{}", bind, http_port).parse().expect("invalid HTTP addr");
            http_proxy::run_http_gateway(addr, reg, auth, auth_cfg, proxy, rate_limit, rate_period).await;
        });
    }

    {
        let reg = registry.clone();
        let port = cfg.grpc_port.unwrap_or(50051);
        let bind = cfg.grpc_bind_addr.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        spawn(async move {
            let bind = format!("{}:{}", bind, port);
            grpc_service::run_grpc_gateway(&bind, reg).await.unwrap_or_else(|e| error!("gRPC gateway failed: {}", e));
        });
    }

    loop { tokio::time::sleep(Duration::from_secs(3600)).await; }
}
