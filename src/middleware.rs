// src/middleware.rs

use crate::backend_registry::BackendRegistry;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use futures_core::future::BoxFuture;
use std::{sync::Arc, time::Duration};
use tower::layer::Layer;
use tower::limit::RateLimitLayer;
use tower::ServiceBuilder;
use tower_http::{
    add_extension::AddExtensionLayer,
    auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};

/// Simple bearer‐token authorizer
#[derive(Clone)]
pub struct BearerAuth(pub String);

#[async_trait]
impl<B> AsyncAuthorizeRequest<B> for BearerAuth
where
    B: Send + 'static,
{
    type RequestBody = B;
    type ResponseBody = Body;
    type Future = BoxFuture<'static, Result<Request<B>, Response<Body>>>;

    fn authorize(&mut self, req: Request<B>) -> Self::Future {
        let token = self.0.clone();
        Box::pin(async move {
            if req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .filter(|h| *h == token)
                .is_some()
            {
                Ok(req)
            } else {
                let resp = Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::empty())
                    .unwrap();
                Err(resp)
            }
        })
    }
}

/// Build a `Layer` stacking:
/// 1. tracing
/// 2. rate limit
/// 3. registry extension
/// 4. optional bearer auth (if token non‐empty)
pub fn http_middleware(
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) -> impl Layer<Router> + Clone + Send + Sync + 'static {
    // 1) HTTP request tracing
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().include_headers(true))
        .on_response(DefaultOnResponse::new());

    // 2) Rate limiting
    let rate = RateLimitLayer::new(rate_per_sec, rate_burst);

    // 3) Inject our registry via extension
    let ext = AddExtensionLayer::new(registry);

    // 4) Optional bearer‐token auth
    //    `option_layer` keeps the ServiceBuilder monomorphic.
    ServiceBuilder::new()
        .layer(trace)
        .layer(rate)
        .layer(ext)
        .option_layer(auth_token.map(|tok| AsyncRequireAuthorizationLayer::new(BearerAuth(tok))))
        .into_inner()
}
