// src/middleware.rs
use std::{convert::Infallible, sync::Arc, time::Duration};
use futures_core::future::BoxFuture;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use async_trait::async_trait;
use tower::{ServiceBuilder, layer::Layer};
use tower::limit::RateLimitLayer;
use tower_http::{
    trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse},
    auth::{AsyncAuthorizeRequest, AsyncRequireAuthorizationLayer},
    add_extension::AddExtensionLayer,
};
use crate::backend_registry::BackendRegistry;

/// Bearer token authorizer
#[derive(Clone)]
pub struct BearerAuth(pub String);

#[async_trait]
impl<B> AsyncAuthorizeRequest<B> for BearerAuth
where B: Send + 'static
{
    type RequestBody  = B;
    type ResponseBody = Body;
    type Future       = BoxFuture<'static, Result<Request<B>, Response<Body>>>;

    fn authorize(&mut self, mut req: Request<B>) -> Self::Future {
        let token = self.0.clone();
        Box::pin(async move {
            if let Some(h) = req.headers().get("authorization").and_then(|v| v.to_str().ok()) {
                if h == token { return Ok(req); }
            }
            let resp = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty()).unwrap();
            Err(resp)
        })
    }
}

/// Compose middleware stack
/// Compose middleware layers for Axum
pub fn http_middleware(
    registry: Arc<BackendRegistry>,
    auth_token: Option<String>,
    rate_per_sec: u64,
    rate_burst: Duration,
) -> (
    TraceLayer<SharedClassifier<ServerErrorsAsFailures>>,
    RateLimitLayer,
    AddExtensionLayer<Arc<BackendRegistry>>,
    Option<AsyncRequireAuthorizationLayer<BearerAuth>>,
) {
    // Tracing layer
    let trace = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().include_headers(true))
        .on_response(DefaultOnResponse::new());
    // Rate limit layer
    let rate = RateLimitLayer::new(rate_per_sec, rate_burst);
    // Registry extension layer
    let ext = AddExtensionLayer::new(registry);
    // Optional auth layer
    let auth_layer = auth_token.map(|tok| AsyncRequireAuthorizationLayer::new(BearerAuth(tok)));

    (trace, rate, ext, auth_layer)
}
    sb
}
