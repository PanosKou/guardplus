use crate::{
    backend_registry::BackendRegistry,
    config::{Auth, ProxyConfig},
};
use futures::TryStreamExt;
use hyper::{
    body::to_bytes, server::conn::Http as HyperHttp, service::make_service_fn, Body,
    Request as HyperRequest, Response as HyperResponse, Server, StatusCode,
};
use reqwest::Client as ReqwestClient;
use serde_json::Value;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_rustls::TlsAcceptor;
use tower::ServiceBuilder;

#[derive(Default)]
struct Metrics {
    request_count: AtomicU64,
    upstream_errors: AtomicU64,
    active_streams: AtomicUsize,
    latency_ms_sum: AtomicU64,
}

fn unauthorized() -> HyperResponse<Body> {
    HyperResponse::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::empty())
        .unwrap()
}
fn forbidden(msg: &str) -> HyperResponse<Body> {
    HyperResponse::builder()
        .status(StatusCode::FORBIDDEN)
        .body(Body::from(msg.to_string()))
        .unwrap()
}
fn bad_request(msg: &str) -> HyperResponse<Body> {
    HyperResponse::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(msg.to_string()))
        .unwrap()
}

fn validate_auth(
    req: &HyperRequest<Body>,
    bearer: &Option<String>,
    cf_secret: &Option<String>,
) -> Result<(), HyperResponse<Body>> {
    if let Some(token) = bearer {
        let good = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|h| h == format!("Bearer {}", token) || h == token)
            .unwrap_or(false);
        if !good {
            return Err(unauthorized());
        }
    }
    if let Some(secret) = cf_secret {
        let ok = req
            .headers()
            .get("cf-access-jwt-assertion")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == secret)
            .unwrap_or(false);
        if !ok {
            return Err(unauthorized());
        }
    }
    Ok(())
}

fn blocked_endpoint(method: &str, path: &str, proxy_cfg: &ProxyConfig) -> bool {
    if !proxy_cfg.endpoint_allowlist.is_empty()
        && !proxy_cfg
            .endpoint_allowlist
            .iter()
            .any(|p| path.starts_with(p))
    {
        return true;
    }
    let method_path = format!("{} {}", method, path);
    proxy_cfg
        .endpoint_denylist
        .iter()
        .any(|d| d == path || d == &method_path)
}

fn inspect_json_policy(
    path: &str,
    body: &[u8],
    proxy_cfg: &ProxyConfig,
) -> Result<(), HyperResponse<Body>> {
    let guarded = [
        "/api/generate",
        "/api/chat",
        "/api/embed",
        "/v1/chat/completions",
    ];
    if !guarded.contains(&path) {
        return Ok(());
    }
    let v: Value = serde_json::from_slice(body).map_err(|_| bad_request("invalid JSON"))?;
    if let Some(model) = v.get("model").and_then(|m| m.as_str()) {
        if !proxy_cfg.model_allowlist.is_empty() && !proxy_cfg.model_allowlist.contains(model) {
            return Err(forbidden("model not allowed"));
        }
    }
    if let Some(prompt) = v.get("prompt").and_then(|p| p.as_str()) {
        if prompt.chars().count() > proxy_cfg.max_prompt_chars {
            return Err(forbidden("prompt too large"));
        }
    }
    if let Some(messages) = v.get("messages").and_then(|m| m.as_array()) {
        let total: usize = messages
            .iter()
            .filter_map(|m| {
                m.get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.chars().count())
            })
            .sum();
        if total > proxy_cfg.max_prompt_chars {
            return Err(forbidden("messages too large"));
        }
    }
    if let Some(n) = v.get("num_ctx").and_then(|n| n.as_u64()) {
        if n > proxy_cfg.max_num_ctx {
            return Err(forbidden("num_ctx too high"));
        }
    }
    if let Some(n) = v.get("num_predict").and_then(|n| n.as_i64()) {
        if n > proxy_cfg.max_num_predict {
            return Err(forbidden("num_predict too high"));
        }
    }
    Ok(())
}

async fn route_request(
    req: HyperRequest<Body>,
    client: ReqwestClient,
    bearer: Option<String>,
    auth: Auth,
    proxy_cfg: ProxyConfig,
    metrics: Arc<Metrics>,
) -> Result<HyperResponse<Body>, Infallible> {
    if req.uri().path() == "/metrics" {
        let body = format!("gamb_requests_total {}\ngamb_upstream_errors_total {}\ngamb_active_streams {}\ngamb_latency_ms_sum {}\n", metrics.request_count.load(Ordering::Relaxed), metrics.upstream_errors.load(Ordering::Relaxed), metrics.active_streams.load(Ordering::Relaxed), metrics.latency_ms_sum.load(Ordering::Relaxed));
        return Ok(HyperResponse::builder()
            .status(200)
            .body(Body::from(body))
            .unwrap());
    }
    metrics.request_count.fetch_add(1, Ordering::Relaxed);
    let start = Instant::now();

    if let Err(resp) = validate_auth(&req, &bearer, &auth.cloudflare_jwt_secret) {
        return Ok(resp);
    }
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    if blocked_endpoint(&method, &path, &proxy_cfg) {
        return Ok(forbidden("endpoint blocked"));
    }

    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let url = format!(
        "{}{}{}",
        proxy_cfg.upstream.trim_end_matches('/'),
        path,
        query
    );

    let method = req
        .method()
        .as_str()
        .parse::<reqwest::Method>()
        .unwrap_or(reqwest::Method::GET);
    let mut rb = client.request(method, &url);
    for (name, value) in req.headers() {
        let n = name.as_str().to_ascii_lowercase();
        if n == "authorization" || n == "cf-access-jwt-assertion" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            rb = rb.header(name.as_str(), v);
        }
    }

    let body_bytes = to_bytes(req.into_body()).await.unwrap_or_default();
    if body_bytes.len() > proxy_cfg.max_body_bytes {
        return Ok(HyperResponse::builder()
            .status(413)
            .body(Body::from("body too large"))
            .unwrap());
    }
    if let Err(resp) = inspect_json_policy(&path, &body_bytes, &proxy_cfg) {
        return Ok(resp);
    }
    rb = rb.body(body_bytes);

    match rb.send().await {
        Ok(res) => {
            let mut builder = HyperResponse::builder().status(
                StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            );
            for (name, value) in res.headers() {
                if let Ok(v) = value.to_str() {
                    builder = builder.header(name.as_str(), v);
                }
            }
            metrics.active_streams.fetch_add(1, Ordering::Relaxed);
            let data = res.bytes().await.unwrap_or_default();
            let elapsed = start.elapsed().as_millis() as u64;
            metrics.latency_ms_sum.fetch_add(elapsed, Ordering::Relaxed);
            metrics.active_streams.fetch_sub(1, Ordering::Relaxed);
            Ok(builder.body(Body::from(data)).unwrap())
        }
        Err(_) => {
            metrics.upstream_errors.fetch_add(1, Ordering::Relaxed);
            Ok(HyperResponse::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("Bad gateway"))
                .unwrap())
        }
    }
}

pub async fn run_http_gateway(
    listen_addr: SocketAddr,
    _registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    auth: Auth,
    proxy_cfg: ProxyConfig,
    _rate_per_sec: u64,
    _rate_burst: Duration,
) {
    let client = ReqwestClient::builder()
        .connect_timeout(Duration::from_secs(3))
        .read_timeout(Duration::from_secs(300))
        .build()
        .unwrap();
    let metrics = Arc::new(Metrics::default());
    let make_svc = make_service_fn(move |_| {
        let cli = client.clone();
        let bearer = auth_token.clone();
        let auth = auth.clone();
        let proxy = proxy_cfg.clone();
        let metrics = metrics.clone();
        async move {
            let svc = ServiceBuilder::new().service_fn(move |req| {
                route_request(
                    req,
                    cli.clone(),
                    bearer.clone(),
                    auth.clone(),
                    proxy.clone(),
                    metrics.clone(),
                )
            });
            Ok::<_, Infallible>(svc)
        }
    });
    Server::bind(&listen_addr).serve(make_svc).await.unwrap();
}

pub async fn run_https_gateway(
    listen_addr: SocketAddr,
    _registry: Arc<BackendRegistry>,
    tls_acceptor: TlsAcceptor,
    auth_token: Option<String>,
    auth: Auth,
    proxy_cfg: ProxyConfig,
    _rate_per_sec: u64,
    _rate_burst: Duration,
) {
    let client = ReqwestClient::builder()
        .connect_timeout(Duration::from_secs(3))
        .read_timeout(Duration::from_secs(300))
        .build()
        .unwrap();
    let listener = tokio::net::TcpListener::bind(&listen_addr)
        .await
        .expect("bind failed");
    let metrics = Arc::new(Metrics::default());
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let acceptor = tls_acceptor.clone();
        let cli = client.clone();
        let bearer = auth_token.clone();
        let auth = auth.clone();
        let proxy = proxy_cfg.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            if let Ok(stream) = acceptor.accept(socket).await {
                let svc = ServiceBuilder::new().service_fn(move |req| {
                    route_request(
                        req,
                        cli.clone(),
                        bearer.clone(),
                        auth.clone(),
                        proxy.clone(),
                        metrics.clone(),
                    )
                });
                let _ = HyperHttp::new().serve_connection(stream, svc).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_endpoint_block() {
        let cfg = ProxyConfig::default();
        assert!(blocked_endpoint("POST", "/api/pull", &cfg));
        assert!(!blocked_endpoint("POST", "/api/chat", &cfg));
    }

    #[test]
    fn test_model_allowlist() {
        let mut cfg = ProxyConfig::default();
        cfg.model_allowlist.insert("m1".to_string());
        let b = br#"{"model":"m2","prompt":"hi"}"#;
        assert!(inspect_json_policy("/api/generate", b, &cfg).is_err());
    }

    #[test]
    fn test_auth_deny() {
        let req = HyperRequest::builder()
            .uri("/api/chat")
            .body(Body::empty())
            .unwrap();
        assert!(validate_auth(&req, &Some("abc".into()), &None).is_err());
    }
}
