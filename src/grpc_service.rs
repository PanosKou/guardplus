use crate::backend_registry::BackendRegistry;
use tonic::{transport::Server, Request, Response, Status};
use tonic::transport::Channel;
use tonic::codegen::InterceptedService;
use tonic::service::Interceptor;
use tonic::metadata::{MetadataMap, MetadataValue};
use std::sync::Arc;

/// Protobuf definitions (in-line for simplicity)
pub mod echo {
    tonic::include_proto!("echo");
}

/// Simple gRPC client interceptor to forward incoming metadata to backend, if needed.
#[derive(Clone)]
struct MetadataForwardInterceptor {
    forward_meta: MetadataMap,
}

impl Interceptor for MetadataForwardInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        // Attach the forwarded metadata
        for (key, value) in self.forward_meta.iter() {
            request.metadata_mut().insert(key.clone(), value.clone());
        }
        Ok(request)
    }
}

/// Our gRPC proxy service: receives Echo requests, forwards them to backend Echo service.
#[derive(Clone)]
pub struct EchoProxy {
    registry: Arc<BackendRegistry>,
}

#[tonic::async_trait]
impl echo::echo_server::Echo for EchoProxy {
    async fn say_hello(
        &self,
        req: Request<echo::EchoRequest>,
    ) -> Result<Response<echo::EchoReply>, Status> {
        // Determine service name from metadata “service-name”
        let service_name = req
            .metadata()
            .get("service-name")
            .ok_or_else(|| Status::invalid_argument("Missing service-name metadata"))?
            .to_str()
            .map_err(|_| Status::invalid_argument("Invalid service-name"))?;

        // Pick backend URL
        let target = self
            .registry
            .pick_one(service_name)
            .ok_or_else(|| Status::unavailable("No backend"))?;

        // Connect to backend via Channel
        let channel = Channel::from_shared(target)
            .unwrap()
            .connect()
            .await
            .map_err(|e| Status::internal(format!("Channel connect error: {}", e)))?;

        // Forward metadata so backend can know context if needed
        let interceptor = MetadataForwardInterceptor {
            forward_meta: req.metadata().clone(),
        };

        let mut client = echo::echo_client::EchoClient::with_interceptor(channel, interceptor);

        // Forward the request
        let reply = client
            .say_hello(req.into_inner())
            .await?
            .into_inner();

        Ok(Response::new(reply))
    }
}

/// Launch the gRPC server on `listen_addr`
pub async fn run_grpc_gateway(listen_addr: &str, registry: Arc<BackendRegistry>) -> Result<(), Box<dyn std::error::Error>> {
    let addr = listen_addr.parse()?;
    let echo_svc = EchoProxy {
        registry: registry.clone(),
    };

    println!("gRPC gateway listening on {}", listen_addr);
    Server::builder()
        .add_service(echo::echo_server::EchoServer::new(echo_svc))
        .serve(addr)
        .await?;

    Ok(())
}
