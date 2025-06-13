// File: src/tcp_udp_proxy.rs
// Description: Basic TCP and UDP proxies with backend routing from BackendRegistry

use crate::backend_registry::BackendRegistry;
use std::{io, net::SocketAddr, sync::Arc};
use tokio::{io::copy_bidirectional, net::{TcpListener, TcpStream, UdpSocket}};

/// TCP gateway: accepts incoming TCP connections and proxies to backend from registry
pub async fn run_tcp_gateway(
    listen_addr: SocketAddr,
    service_name: String,
    registry: Arc<BackendRegistry>,
) -> io::Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    println!("[tcp] Listening on {} for service '{}'", listen_addr, service_name);

    loop {
        let (mut inbound, _) = listener.accept().await?;
        let registry = registry.clone();
        let service = service_name.clone();  // clone here, service_name itself never moves

        tokio::spawn(async move {
            if let Some(backend) = registry.pick_one(&service) {
                match TcpStream::connect(&backend).await {
                    Ok(mut outbound) => {
                        let _ = copy_bidirectional(&mut inbound, &mut outbound).await;
                    }
                    Err(err) => {
                        eprintln!("[tcp] Failed to connect to backend {}: {}", backend, err);
                    }
                }
            } else {
                eprintln!("[tcp] No backend found for service '{}'", service);
            }
        });
    }
}

/// UDP gateway: forwards incoming datagrams to backend and returns response
pub async fn run_udp_gateway(
    listen_addr: SocketAddr,
    service_name: String,
    registry: Arc<BackendRegistry>,
) -> io::Result<()> {
    // Wrap in Arc so we can clone it into each task
    let socket = Arc::new(UdpSocket::bind(listen_addr).await?);
    println!("[udp] Listening on {} for service '{}'", listen_addr, service_name);
    let mut buf = [0u8; 2048];

    loop {
        let (len, peer_addr) = socket.recv_from(&mut buf).await?;
        let data = buf[..len].to_vec();
        let registry = registry.clone();
        let service = service_name.clone();
        let local_socket = socket.clone();  // Arc<UdpSocket> clone

        tokio::spawn(async move {
            if let Some(backend) = registry.pick_one(&service) {
                if let Ok(tmp_socket) = UdpSocket::bind("0.0.0.0:0").await {
                    // send to backend
                    let _ = tmp_socket.send_to(&data, &backend).await;
                    let mut resp_buf = [0u8; 2048];
                    // receive response from backend
                    if let Ok((resp_len, _)) = tmp_socket.recv_from(&mut resp_buf).await {
                        // echo back to original peer
                        let _ = local_socket.send_to(&resp_buf[..resp_len], peer_addr).await;
                    }
                } else {
                    eprintln!("[udp] Failed to bind ephemeral socket for backend {}", backend);
                }
            } else {
                eprintln!("[udp] No backend found for service '{}'", service);
            }
        });
    }
}
