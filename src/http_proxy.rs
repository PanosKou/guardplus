use crate::backend_registry::BackendRegistry;
use crate::middleware::http_middleware;
use hyper::{
    client::HttpConnector, server::conn::AddrStream, service::make_service_fn, Body, Client,
    Request, Response, Server, Uri,
};
use std::{net::SocketAddr, sync::Arc};
use tower::{ServiceBuilder, ServiceExt};
use tower_http::add_extension::AddExtensionLayer;

/// The “lazy routing” function: based on request, decide which service name you hit.
/// This mimics Rivet Guard’s routing_fn, but here we just do a path‐based lookup:
///
/// e.g. GET /foo/... → service name “foo” → pick a backend from registry under “foo”
///
async fn route_request(
    req: Request<Body>,
    registry: Arc<BackendRegistry>,
) -> Result<Response<Body>, hyper::Error> {
    // Extract path segment as service name:
    let path = req.uri().path();
    let mut segments = path.trim_start_matches('/').split('/');
    if let Some(service_name) = segments.next() {
        if let Some(target) = registry.pick_one(service_name) {
            // Build new URI for proxied request:
            // If target is “http://127.0.0.1:9000”, and original was “/foo/bar”,
            // then we proxy to “http://127.0.0.1:9000/bar?...”
            let mut parts = req.uri().clone().into_parts();
            // Replace authority/host with backend, preserve scheme
            let backend_uri: Uri =
                format!("{}{}", target, &path[service_name.len() + 1..]).parse().unwrap();
            parts.scheme = backend_uri.scheme().cloned();
            parts.authority = backend_uri.authority().cloned();
            parts.path_and_query = backend_uri.path_and_query().cloned();

            let proxied = Request::from_parts(parts, req.into_body());
            // Send via Hyper client
            let client: Client<HttpConnector> = Client::new();
            client.request(proxied).await
        } else {
            // No backend found → 502
            Ok(Response::builder()
                .status(502)
                .body(Body::from(format!("No backend for service '{}'", service_name)))
                .unwrap())
        }
    } else {
        // Root path or malformed
        Ok(Response::builder()
            .status(404)
            .body(Body::from("No service specified"))
            .unwrap())
    }
}

/// Launch an HTTP/1 server on `listen_addr`.
/// Applies middleware layers (tracing, auth, rate limiting).
pub async fn run_http_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
) {
    // Build a “make_service” that clones registry for each conn
    let make_svc = make_service_fn(move |conn: &AddrStream| {
        let reg = registry.clone();
        let auth = auth_token.clone();

        // Compose per-connection service
        let svc = ServiceBuilder::new()
            // 1) Add our registry as an extension so we can access it in the handler
            .layer(AddExtensionLayer::new(reg.clone()))
            // 2) Middleware: tracing, rate-limit, optional auth
            .layer(http_middleware(auth))
            .service_fn(move |req: Request<Body>| {
                let reg2 = reg.clone();
                async move { route_request(req, reg2).await }
            });

        async move { Ok::<_, hyper::Error>(svc) }
    });

    let server = Server::bind(&listen_addr).serve(make_svc);
    println!("HTTP gateway listening on http://{}", listen_addr);

    if let Err(err) = server.await {
        eprintln!("HTTP gateway error: {}", err);
    }
}
