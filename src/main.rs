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
use std::{error::Error, net::SocketAddr, sync::Arc};
use tokio::spawn;
use tls_config::TlsConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Load entire configuration
    let cfg = Config::from_file("config.yaml").map_err(|e| {
        error!("Config load failed: {}", e);
        e
    })?;

    info!("Loaded config: listening HTTP/HTTPS on port {}", cfg.http_port);
    info!("Consul URL: {}", cfg.consul_url);
    info!("TLS mode: {}", cfg.tls_mode);

    // Initialize TLS acceptor
    let tls_config = TlsConfig::load(&cfg.tls.cert_path, &cfg.tls.key_path).map_err(|e| {
        error!("TLS load failed: {}", e);
        e
    })?;
    let tls_acceptor = tls_config.acceptor.clone();

    // Set up backend registry and register from config
    let registry = Arc::new(BackendRegistry::new());
    for be in &cfg.backends {
        for route in &be.routes {
            let key = route.trim_start_matches('/').split('/').next().unwrap_or_default();
            registry.register(key, be.address.clone());
            info!("Registered backend '{}' → {}", key, be.address);
        }
    }

    // Spawn HTTPS proxy
    {
        let reg = registry.clone();
        let acceptor = tls_acceptor.clone();
        let port = cfg.http_port;
        spawn(async move {
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            http_proxy::run_https_gateway(addr, reg, acceptor, None).await;
        });
    }

    // Spawn HTTP proxy on same port (if needed)
    {
        let reg = registry.clone();
        let port = cfg.http_port;
        spawn(async move {
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            http_proxy::run_http_gateway(addr, reg, None).await;
        });
    }

    // Spawn gRPC, TCP and UDP using config defaults or fallback ports
    let grpc_addr = format!("0.0.0.0:{}", cfg.grpc_port.unwrap_or(50051));
    spawn(grpc_service::run_grpc_gateway(grpc_addr.clone(), registry.clone()));

    let tcp_addr = format!("0.0.0.0:{}", cfg.tcp_port.unwrap_or(91000));
    spawn(async move {
        let addr: SocketAddr = tcp_addr.parse().unwrap();
        tcp_udp_proxy::run_tcp_gateway(addr, "tcpservice".into(), registry.clone())
            .await
            .unwrap();
    });

    let udp_addr = format!("0.0.0.0:{}", cfg.udp_port.unwrap_or(92000));
    spawn(async move {
        let addr: SocketAddr = udp_addr.parse().unwrap();
        tcp_udp_proxy::run_udp_gateway(addr, "udpservice".into(), registry.clone())
            .await
            .unwrap();
    });

    // Print key config info
    println!("OIDC providers:");
    for prov in &cfg.auth.oidc_providers {
        println!("- {} @ {}", prov.name, prov.issuer_url);
    }

    // Prevent exit
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
