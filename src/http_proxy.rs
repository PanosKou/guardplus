// src/http_proxy.rs

use crate::backend_registry::BackendRegistry;
use hyper::{
    body::to_bytes, server::conn::Http as HyperHttp, service::make_service_fn, Body,
    Request as HyperRequest, Response as HyperResponse, Server, StatusCode,
};
use reqwest::header::{HeaderName as ReqwestName, HeaderValue as ReqwestValue};
use reqwest::{Client as ReqwestClient, Method as ReqwestMethod};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio_rustls::TlsAcceptor;
use tower::limit::RateLimitLayer;
use tower::ServiceBuilder;

/// Single‐place to do bearer validation.
/// Returns `Ok(req)` if `auth_token` is `None` or matches the `Authorization` header;
/// otherwise returns a 401 response.
async fn authorize<B>(
    req: HyperRequest<B>,
    auth_token: &Option<String>,
) -> Result<HyperRequest<B>, HyperResponse<Body>> {
    if let Some(token) = auth_token {
        // Expect header exactly equal to token
        match req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
        {
            Some(h) if h == token => Ok(req),
            _ => {
                let resp = HyperResponse::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::empty())
                    .unwrap();
                Err(resp)
            }
        }
    } else {
        // no token configured ⇒ allow all
        Ok(req)
    }
}

async fn route_request(
    req: HyperRequest<Body>,
    registry: Arc<BackendRegistry>,
    client: ReqwestClient,
    auth_token: Option<String>,
) -> Result<HyperResponse<Body>, Infallible> {
    // 1) Auth check
    let req = match authorize(req, &auth_token).await {
        Ok(r) => r,
        Err(resp) => return Ok(resp),
    };

    // 2) Extract service + suffix
    let path = req.uri().path().trim_start_matches('/').to_string();
    let mut parts = path.splitn(2, '/');
    let service = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");

    // 3) Pick backend
    if let Some(target) = registry.pick_one(service) {
        let backend_url = format!("{}/{}", target.trim_end_matches('/'), suffix);

        // 4) Build reqwest request
        let method = req
            .method()
            .as_str()
            .parse::<ReqwestMethod>()
            .expect("valid HTTP method");
        let mut rb = client.request(method, &backend_url);

        // 5) Copy headers
        for (name, value) in req.headers().iter() {
            if let Ok(val_str) = value.to_str() {
                let hn =
                    ReqwestName::from_bytes(name.as_str().as_bytes()).expect("header name parse");
                let hv = ReqwestValue::from_str(val_str).expect("header value parse");
                rb = rb.header(hn, hv);
            }
        }

        // 6) Copy body
        let body_bytes = to_bytes(req.into_body()).await.unwrap_or_default();
        rb = rb.body(body_bytes);

        // 7) Dispatch and reassemble
        match rb.send().await {
            Ok(res) => {
                let mut builder = HyperResponse::builder().status(
                    StatusCode::from_u16(res.status().as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                );
                for (name, value) in res.headers().iter() {
                    if let Ok(val_str) = value.to_str() {
                        builder = builder.header(name.as_str(), val_str);
                    }
                }
                let bytes = res.bytes().await.unwrap_or_default();
                let resp = builder.body(Body::from(bytes)).unwrap();
                Ok(resp)
            }
            Err(_) => Ok(HyperResponse::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("Bad gateway"))
                .unwrap()),
        }
    } else {
        // No such service
        Ok(HyperResponse::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Service not found"))
            .unwrap())
    }
}

pub async fn run_http_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: std::time::Duration,
) {
    let client = ReqwestClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    let make_svc = make_service_fn(move |_| {
        let reg = registry.clone();
        let cli = client.clone();
        let auth = auth_token.clone();

        async move {
            // Only rate‐limit at the Tower level
            let svc = ServiceBuilder::new()
                .layer(RateLimitLayer::new(rate_per_sec, rate_burst))
                .service_fn(move |req| route_request(req, reg.clone(), cli.clone(), auth.clone()));

            Ok::<_, Infallible>(svc)
        }
    });

    println!("HTTP listening on http://{}", listen_addr);
    Server::bind(&listen_addr).serve(make_svc).await.unwrap();
}

pub async fn run_https_gateway(
    listen_addr: SocketAddr,
    registry: Arc<BackendRegistry>,
    tls_acceptor: TlsAcceptor,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: std::time::Duration,
) {
    let client = ReqwestClient::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .expect("bind failed");
    println!("HTTPS listening on https://{}", listen_addr);

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let acceptor = tls_acceptor.clone();
        let reg = registry.clone();
        let cli = client.clone();
        let auth = auth_token.clone();
        let rate = rate_per_sec;
        let burst = rate_burst;

        tokio::spawn(async move {
            if let Ok(stream) = acceptor.accept(socket).await {
                let svc = ServiceBuilder::new()
                    .layer(RateLimitLayer::new(rate, burst))
                    .service_fn(move |req| {
                        route_request(req, reg.clone(), cli.clone(), auth.clone())
                    });

                if let Err(err) = HyperHttp::new().serve_connection(stream, svc).await {
                    eprintln!("HTTPS connection error: {}", err);
                }
            }
        });
    }
}