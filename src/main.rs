// src/main.rs
pub mod echo {
    tonic::include_proto!("echo");
}
mod backend_registry;
mod grpc_service;
mod http_proxy;
mod middleware;
mod tcp_udp_proxy;
mod config;
mod tls_config;

use backend_registry::BackendRegistry;
use config::Config;
use log::{error, info};
use std::{error::Error, net::SocketAddr, sync::Arc, time::Duration};
use tokio::spawn;
use tls_config::TlsConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::init();

    // 1) Load entire configuration
    let cfg = Config::from_file("config.yaml").map_err(|e| {
        error!("Config load failed: {}", e);
        e
    })?;

    info!(
        "Loaded config: HTTP port {}, HTTPS port {:?}, gRPC port {:?}, TCP port {:?}, UDP port {:?}",
        cfg.http_port, cfg.https_port, cfg.grpc_port, cfg.tcp_port, cfg.udp_port
    );
    info!("Consul URL: {}", cfg.consul_url);
    info!("TLS mode: {}", cfg.tls_mode);

    // 2) Initialize TLS acceptor once
    let tls_cfg = TlsConfig::load(&cfg.tls.cert_path, &cfg.tls.key_path).map_err(|e| {
        error!("TLS load failed: {}", e);
        e
    })?;
    let tls_acceptor = tls_cfg.acceptor.clone();

    // 3) Build backend registry
    let registry = Arc::new(BackendRegistry::new());
    for be in &cfg.backends {
        registry.register(&be.name, &be.address);
        info!(
            "Registered backend '{}' via {} at {}",
            be.name, be.protocol, be.address
        );
    }

    // Common parameters
    let bearer      = cfg.bearer_token.clone();
    let rate_per_sec = cfg.rate_limit_per_sec;
    let rate_burst   = Duration::from_secs(cfg.rate_limit_burst as u64);

    // Determine HTTPS port (fallback to http_port + 1)
    let https_port = cfg.https_port.unwrap_or(cfg.http_port + 1);

    // 4) Spawn HTTPS gateway
    {
        let reg      = registry.clone();
        let acceptor = tls_acceptor.clone();
        let auth     = Some(bearer.clone());
        let rate     = rate_per_sec;
        let burst    = rate_burst;
        spawn(async move {
            let addr: SocketAddr =
                format!("0.0.0.0:{}", https_port).parse().expect("invalid HTTPS addr");
            http_proxy::run_https_gateway(addr, reg, acceptor, auth, rate, burst).await;
        });
        info!("Spawned HTTPS gateway on port {}", https_port);
    }

    // 5) Spawn HTTP gateway
    {
        let reg   = registry.clone();
        let auth  = Some(bearer.clone());
        let rate  = rate_per_sec;
        let burst = rate_burst;
        spawn(async move {
            let addr: SocketAddr =
                format!("0.0.0.0:{}", cfg.http_port).parse().expect("invalid HTTP addr");
            http_proxy::run_http_gateway(addr, reg, auth, rate, burst).await;
        });
        info!("Spawned HTTP gateway on port {}", cfg.http_port);
    }

    // 6) Spawn gRPC gateway
    {
        let reg   = registry.clone();
        let port  = cfg.grpc_port.unwrap_or(50051);
        spawn(async move {
            let bind = format!("0.0.0.0:{}", port);
            grpc_service::run_grpc_gateway(&bind, reg)
                .await
                .unwrap_or_else(|e| error!("gRPC gateway failed: {}", e));
        });
        info!("Spawned gRPC gateway on port {}", port);
    }

    // 7) Spawn TCP proxy
    {
        let reg  = registry.clone();
        let auth = Some(bearer.clone());
        let port = cfg.tcp_port.unwrap_or(9100);
        spawn(async move {
            let addr: SocketAddr =
                format!("0.0.0.0:{}", port).parse().expect("invalid TCP addr");
            tcp_udp_proxy::run_tcp_gateway(addr, auth, reg)
                .await
                .unwrap_or_else(|e| error!("TCP gateway failed: {}", e));
        });
        info!("Spawned TCP gateway on port {}", port);
    }

    // 8) Spawn UDP proxy
    {
        let reg  = registry.clone();
        let auth = Some(bearer);
        let port = cfg.udp_port.unwrap_or(9200);
        spawn(async move {
            let addr: SocketAddr =
                format!("0.0.0.0:{}", port).parse().expect("invalid UDP addr");
            tcp_udp_proxy::run_udp_gateway(addr, auth, reg)
                .await
                .unwrap_or_else(|e| error!("UDP gateway failed: {}", e));
        });
        info!("Spawned UDP gateway on port {}", port);
    }

    // 9) Log configured OIDC providers
    info!("OIDC providers configured:");
    for prov in &cfg.auth.oidc_providers {
        info!("- {} @ {}", prov.name, prov.issuer_url);
    }

    // 10) Prevent exit
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
