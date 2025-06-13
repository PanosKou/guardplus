// src/grpc_service.rs

use std::sync::Arc;
use tonic::{
    transport::{Server, Channel},
    Request, Response, Status,
    service::Interceptor,
    metadata::{MetadataMap, KeyAndValueRef},
};
use crate::backend_registry::BackendRegistry;
use crate::echo::{
    EchoRequest, EchoResponse,
    echo_server::{Echo, EchoServer},
    echo_client::EchoClient,
};

/// Interceptor that forwards all incoming metadata to the downstream request.
#[derive(Clone)]
struct MetadataForwardInterceptor {
    forward_meta: MetadataMap,
}

impl Interceptor for MetadataForwardInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, Status> {
        //use tonic::metadata::KeyAndValueRef;

        for kv in self.forward_meta.iter() {
            match kv {
                KeyAndValueRef::Ascii(key, value) => {
                    // ASCII metadata
                    req.metadata_mut().insert(key.clone(), value.clone());
                }
                KeyAndValueRef::Binary(key, value) => {
                    // binary metadata
                    req.metadata_mut().insert_bin(key.clone(), value.clone());
                }
            }
        }
        Ok(req)
    }
}

/// Simple gRPC proxy: looks up a backend by metadata then forwards the Echo RPC.
#[derive(Clone)]
pub struct EchoProxy {
    registry: Arc<BackendRegistry>,
}

#[tonic::async_trait]
impl Echo for EchoProxy {
    async fn echo(
        &self,
        req: Request<EchoRequest>,
    ) -> Result<Response<EchoResponse>, Status> {
        // 1) Extract the service name from metadata
        let service_name = req
            .metadata()
            .get("service-name")
            .ok_or_else(|| Status::invalid_argument("Missing service-name header"))?
            .to_str()
            .map_err(|_| Status::invalid_argument("Invalid service-name header"))?;

        // 2) Pick a backend URL by service name
        let target = self
            .registry
            .pick_one(service_name)
            .ok_or_else(|| Status::unavailable("No backend available"))?;

        // 3) Build a channel to the backend
        let channel = Channel::from_shared(target.clone())
            .map_err(|e| Status::internal(format!("Invalid URL: {}", e)))?
            .connect()
            .await
            .map_err(|e| Status::internal(format!("Channel error: {}", e)))?;

        // 4) Forward incoming metadata
        let interceptor = MetadataForwardInterceptor {
            forward_meta: req.metadata().clone(),
        };
        let mut client = EchoClient::with_interceptor(channel, interceptor);

        // 5) Forward the request and return the response
        let response = client
            .echo(req.into_inner())
            .await?
            .into_inner();
        Ok(Response::new(response))
    }
}

/// Launches the gRPC proxy service on the given address (e.g. "0.0.0.0:50051")
pub async fn run_grpc_gateway(
    listen_addr: &str,
    registry: Arc<BackendRegistry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = listen_addr.parse()?;
    let svc = EchoProxy { registry };

    println!("gRPC gateway listening on {}", listen_addr);
    Server::builder()
        .add_service(EchoServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}
