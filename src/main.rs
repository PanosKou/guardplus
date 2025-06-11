// src/main.rs

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

    info!("Loaded config: HTTP port {}, gRPC port {:?}, TCP port {:?}, UDP port {:?}",
        cfg.http_port, cfg.grpc_port, cfg.tcp_port, cfg.udp_port);
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
        registry.register(&be.name, be.address.clone());
        info!(
            "Registered backend '{}' via {} at {}",
            be.name, be.protocol, be.address
        );
    }

    // Common parameters
    let bearer = cfg.bearer_token.clone();
    let rate_per_sec = cfg.rate_limit_per_sec;
    let rate_burst = Duration::from_secs(cfg.rate_limit_burst as u64);

    // 4) Spawn HTTPS gateway
    {
        let reg = registry.clone();
        let acceptor = tls_acceptor.clone();
        let auth = Some(bearer.clone());
        let rate = rate_per_sec;
        let burst = rate_burst;
        let port = cfg.http_port;
        spawn(async move {
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            http_proxy::run_https_gateway(addr, reg, acceptor, auth, rate, burst).await;
        });
        info!("Spawned HTTPS gateway on port {}", cfg.http_port);
    }

    // 5) Spawn HTTP gateway
    {
        let reg = registry.clone();
        let auth = Some(bearer.clone());
        let rate = rate_per_sec;
        let burst = rate_burst;
        let port = cfg.http_port;
        spawn(async move {
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            http_proxy::run_http_gateway(addr, reg, auth, rate, burst).await;
        });
        info!("Spawned HTTP gateway on port {}", cfg.http_port);
    }

    // 6) Spawn gRPC gateway
    {
        let reg = registry.clone();
        let port = cfg.grpc_port.unwrap_or(50051);
        let addr_str = format!("0.0.0.0:{}", port);
        spawn(async move {
            // Only two arguments now: &str and Arc<Registry>
            grpc_service::run_grpc_gateway(&addr_str, reg)
                .await
                .expect("gRPC gateway failed");
        });
        info!("Spawned gRPC gateway on port {}", port);
    }

    // 7) Spawn TCP proxy
    {
        let reg = registry.clone();
        let auth = Some(bearer.clone());
        let port = cfg.tcp_port.unwrap_or(91000);
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        spawn(async move {
            tcp_udp_proxy::run_tcp_gateway(addr, auth, reg)
                .await
                .expect("TCP gateway failed");
        });
        info!("Spawned TCP gateway on port {}", port);
    }

    // 8) Spawn UDP proxy
    {
        let reg = registry.clone();
        let auth = Some(bearer.clone());
        let port = cfg.udp_port.unwrap_or(92000);
        let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        spawn(async move {
            tcp_udp_proxy::run_udp_gateway(addr, auth, reg)
                .await
                .expect("UDP gateway failed");
        });
        info!("Spawned UDP gateway on port {}", port);
    }

    // 9) Log configured OIDC providers
    println!("OIDC providers configured:");
    for prov in &cfg.auth.oidc_providers {
        println!("- {} @ {}", prov.name, prov.issuer_url);
    }

    // 10) Prevent exit
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
