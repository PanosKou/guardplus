// src/lib.rs

// 1) Include the generated code for package "echo"
//    This creates a module `echo` at the crate root.
pub mod echo {
    tonic::include_proto!("echo");
}

// 2) Re-export the gRPC gateway function
// Expose all other modules so they live in the library crate root:
pub mod backend_registry;
pub mod grpc_service;
pub mod http_proxy;
pub mod middleware;
pub mod tcp_udp_proxy;
pub mod config;
pub mod tls_config;