mod backend_registry;
mod grpc_service;
mod http_proxy;
mod middleware;
mod tcp_udp_proxy;

use backend_registry::BackendRegistry;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::spawn;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Initialize registry and register some backends (hard-coded for demo).
    let registry = Arc::new(BackendRegistry::new());

    // HTTP backends for service “http”
    registry.register("http", "http://127.0.0.1:9000");
    registry.register("http", "http://127.0.0.1:9001");

    // gRPC backends for service “gprc”
    registry.register("grpc", "http://127.0.0.1:50052"); // tonic expects http://

    // TCP backends for “tcpservice”
    registry.register("tcpservice", "127.0.0.1:9100");
    registry.register("tcpservice", "127.0.0.1:9101");

    // UDP backends for “udpservice”
    registry.register("udpservice", "127.0.0.1:9200");
    registry.register("udpservice", "127.0.0.1:9201");

    // 2) Launch HTTP gateway on 0.0.0.0:8080 (with optional basic auth token "Bearer SECRET")
    let http_registry = registry.clone();
    spawn(async move {
        let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
        // If you want to require “Bearer SECRET” on every HTTP request, use Some("Bearer SECRET".into())
        http_proxy::run_http_gateway(addr, http_registry, None).await;
    });

    // 3) Launch gRPC gateway on 0.0.0.0:50051
    let grpc_registry = registry.clone();
    spawn(async move {
        grpc_service::run_grpc_gateway("0.0.0.0:50051", grpc_registry)
            .await
            .unwrap();
    });

    // 4) Launch TCP gateway for service “tcpservice” on 0.0.0.0:91000
    let tcp_registry = registry.clone();
    spawn(async move {
        let listen: SocketAddr = "0.0.0.0:91000".parse().unwrap();
        tcp_udp_proxy::run_tcp_gateway(listen, "tcpservice".into(), tcp_registry)
            .await
            .unwrap();
    });

    // 5) Launch UDP gateway for service “udpservice” on 0.0.0.0:92000
    let udp_registry = registry.clone();
    spawn(async move {
        let listen: SocketAddr = "0.0.0.0:92000".parse().unwrap();
        tcp_udp_proxy::run_udp_gateway(listen, "udpservice".into(), udp_registry)
            .await
            .unwrap();
    });

    // Prevent main from exiting
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}


fn load_tls_config(cert_path: &str, key_path: &str) -> Arc<ServerConfig> {
    let cert_file = &mut BufReader::new(File::open(cert_path).unwrap());
    let key_file = &mut BufReader::new(File::open(key_path).unwrap());

    let cert_chain = rustls_pemfile::certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();

    let mut keys = rustls_pemfile::pkcs8_private_keys(key_file).unwrap();
    let key = PrivateKey(keys.remove(0));

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .unwrap();

    Arc::new(config)
}
