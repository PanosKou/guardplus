use hyper::{Body, Request, Response, StatusCode};
use std::convert::Infallible;
use std::time::Duration;
use tower::{Layer, ServiceBuilder};
use tower_http::{
    auth::RequireAuthorizationLayer,
    classify::{ServerErrorsFailureClass, SharedClassifier},
    limit::RateLimitLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};

/// Compose a stack of middleware layers:
/// 1. Tracing/logging  
/// 2. Basic auth (if header matches)  
/// 3. Rate limiting  
pub fn http_middleware<B>(
    auth_token: Option<String>, // e.g. "Bearer SECRET"
) -> impl Layer<
    tower_http::middleware::Next<B>,
    Service = tower_http::middleware::Next<B>,
    ServiceRequest = Request<B>,
    ServiceResponse = Response<B>,
    Error = Infallible,
    Future = impl std::future::Future<Output = Result<Response<B>, Infallible>>,
>
where
    B: Send + 'static,
{
    // Tracing layer
    let trace_layer = TraceLayer::<Request<B>, Response<B>, SharedClassifier>::new_for_http()
        .on_request(DefaultOnRequest::new().level(tower_http::trace::Level::INFO))
        .make_span_with(DefaultMakeSpan::new().include_headers(true))
        .on_response(DefaultOnResponse::new().level(tower_http::trace::Level::INFO));

    // Basic auth layer (if provided)
    let auth_layer = auth_token.clone().map(|token| {
        // Expects header "authorization: Bearer SECRET"
        RequireAuthorizationLayer::bearer(&token[7..]) // skip “Bearer ”
    });

    // Rate limit: 100 requests per second with burst capacity of 50
    let rate_limit_layer = RateLimitLayer::new(100, Duration::from_secs(1));

    // Build the stack
    let mut builder = ServiceBuilder::new().layer(trace_layer).layer(rate_limit_layer);

    if let Some(auth) = auth_layer {
        builder = builder.layer(auth);
    }

    builder
}

/// Simple function to check a header token if you want custom logic:
pub async fn check_basic_auth(req: Request<Body>) -> Result<Request<Body>, (Response<Body>, Request<Body>)> {
    // e.g. expect header “authorization: Bearer SECRET”
    const SECRET: &str = "SECRET";
    if let Some(auth) = req.headers().get("authorization") {
        if auth.to_str().unwrap_or("") == format!("Bearer {}", SECRET) {
            Ok(req)
        } else {
            let resp = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from("Invalid token"))
                .unwrap();
            Err((resp, req))
        }
    } else {
        let resp = Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::from("Missing auth"))
            .unwrap();
        Err((resp, req))
    }
}
