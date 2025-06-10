use crate::backend_registry::BackendRegistry;
use crate::middleware::http_middleware;
use hyper::{
    client::HttpConnector,
    server::conn::Http,
    service::service_fn,
    Body, Client, Request, Response, Server,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::{ServiceBuilder, ServiceExt};
use tower::limit::rate::RateLimitLayer;
use tower_http::add_extension::AddExtensionLayer;
use std::time::Duration;

/// Shared logic for request routing
async fn route_request(
    req: Request<Body>,
    registry: Arc<BackendRegistry>,
) -> Result<Response<Body>, hyper::Error> {
    let path = req.uri().path();
    let mut segments = path.trim_start_matches('/').split('/');
    if let Some(service_name) = segments.next() {
        if let Some(target) = registry.pick_one(service_name) {
            let mut parts = req.uri().clone().into_parts();
            let backend_uri: hyper::Uri =
                format!("{}{}", target, &path[service_name.len() + 1..])
                    .parse()
                    .unwrap();
            parts.scheme = backend_uri.scheme().cloned();
            parts.authority = backend_uri.authority().cloned();
            parts.path_and_query = backend_uri.path_and_query().cloned();

            let proxied = Request::from_parts(parts, req.into_body());
            let client: Client<HttpConnector> = Client::new();
            client.request(proxied).await
        } else {
            Ok(Response::builder()
                .status(502)
                .body(Body::from(format!("No backend for '{}'", service_name)))
                .unwrap())
        }
    } else {
        Ok(Response::builder()
            .status(404)
            .body(Body::from("No service specified"))
            .unwrap())
    }
}

/// Launch an unencrypted HTTP gateway with rate limiting
pub async fn run_http_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) {
    let make_svc = hyper::service::make_service_fn(move |_| {
        let reg = registry.clone();
        let auth = auth_token.clone();

        let svc = ServiceBuilder::new()
            .layer(RateLimitLayer::new(rate_per_sec, rate_burst))
            .layer(AddExtensionLayer::new(reg.clone()))
            .layer(http_middleware(auth))
            .service_fn(move |req| {
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

/// Launch a TLS‑secured HTTPS gateway with the same rate limiting
pub async fn run_https_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    tls_acceptor: TlsAcceptor,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) {
    let listener = TcpListener::bind(&listen_addr)
        .await
        .expect("Cannot bind HTTPS listener");
    println!("HTTPS gateway listening on https://{}", listen_addr);

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("TCP accept error: {}", e);
                continue;
            }
        };

        let acceptor = tls_acceptor.clone();
        let reg = registry.clone();
        let auth = auth_token.clone();

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(stream).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("TLS handshake failed: {}", e);
                    return;
                }
            };

            let svc = ServiceBuilder::new()
                .layer(RateLimitLayer::new(rate_per_sec, rate_burst))
                .layer(AddExtensionLayer::new(reg.clone()))
                .layer(http_middleware(auth))
                .service_fn(move |req| {
                    let reg2 = reg.clone();
                    async move { route_request(req, reg2).await }
                });

            if let Err(err) = Http::new()
                .serve_connection(tls_stream, svc)
                .with_upgrades()
                .await
            {
                eprintln!("HTTPS connection error: {}", err);
            }
        });
    }
}
