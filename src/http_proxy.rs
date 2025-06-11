// src/http_proxy.rs
use std::{net::SocketAddr, sync::Arc, time::Duration};
use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, Request, Response, StatusCode, Uri},
    routing::any,
    Router,
};
use axum_server::{tls_rustls::RustlsConfig, Server};
use reqwest::Client;
use crate::{
    backend_registry::BackendRegistry,
    middleware::{http_middleware, BearerAuth},
};

/// Shared reqwest client
fn make_client() -> Client {
    Client::builder().timeout(Duration::from_secs(10)).build().unwrap()
}

/// Proxy handler
async fn route_request(
    mut req: Request<Body>,
    registry: Arc<BackendRegistry>,
    client: Client,
) -> Response<Body> {
    let method = req.method().clone();
    let orig_headers = req.headers().clone();
    let path = req.uri().path().trim_start_matches('/');
    let mut parts = path.splitn(2, '/');
    let svc = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");

    if let Some(base) = registry.pick_one(svc) {
        let full_url = format!("{}/{}", base.trim_end_matches('/'), suffix);
        *req.uri_mut() = Uri::try_from(&full_url).unwrap();
        let bytes = to_bytes(req.into_body(), 1024*1024).await.unwrap_or_default();

        let mut rreq = reqwest::Request::new(
            method.as_str().parse().unwrap(), full_url.parse().unwrap()
        );
        let mut hdrs = reqwest::header::HeaderMap::new();
        for (name, val) in orig_headers.iter() {
            if let (Ok(n), Ok(v)) = (
                name.as_str().parse(), val.to_str().unwrap_or_default().parse()
            ) { hdrs.append(n, v); }
        }
        *rreq.headers_mut() = hdrs;
        *rreq.body_mut()    = Some(bytes.into());

        match client.execute(rreq).await {
            Ok(res) => {
                let status = StatusCode::from_u16(res.status().as_u16()).unwrap();
                let mut bldr = Response::builder().status(status);
                for (hk, hv) in res.headers().iter() {
                    bldr = bldr.header(hk.as_str(), hv.as_bytes());
                }
                let body = res.bytes().await.unwrap_or_default();
                bldr.body(Body::from(body)).unwrap()
            }
            Err(_) => Response::builder().status(StatusCode::BAD_GATEWAY)
                         .body(Body::from("Bad gateway")).unwrap(),
        }
    } else {
        Response::builder().status(StatusCode::NOT_FOUND)
            .body(Body::from("Service not found")).unwrap()
    }
}

/// Run HTTP proxy
pub async fn run_http_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) {
        // Shared HTTP client
    let client = make_client();
    // Compose middleware layers
    let (trace_layer, rate_limit_layer, ext_layer, auth_layer) = http_middleware(
        registry.clone(), auth_token.clone(), rate_per_sec, rate_burst
    );
        // Build the base Router
    let app = Router::new()
        .fallback(any(move |req| route_request(req.clone(), registry.clone(), client.clone())))
        .layer((
            trace_layer.clone(),
            rate_limit_layer.clone(),
            ext_layer.clone(),
            auth_layer.clone().unwrap_or_else(IdentityLayer::new),
        ));
    println!("HTTP proxy on http://{}", listen_addr);
    Server::bind(listen_addr).serve(app.into_make_service()).await.unwrap();
}

/// Run HTTPS proxy
pub async fn run_https_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    cert_pem: &str,
    key_pem: &str,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) {
    let client = make_client();
    let tls = RustlsConfig::from_pem_file(cert_pem, key_pem).await.unwrap();
    let app = Router::new()
        .fallback(any(move |req| route_request(req, registry.clone(), client.clone())))
        .layer(http_middleware(
            registry.clone(), auth_token.clone(), rate_per_sec, rate_burst
        ));
    println!("HTTPS proxy on https://{}", listen_addr);
    Server::bind_rustls(listen_addr, tls).serve(app.into_make_service()).await.unwrap();
}