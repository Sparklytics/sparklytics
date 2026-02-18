use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{routes, state::AppState};

/// Construct the Axum [`Router`] with all routes and middleware attached.
///
/// Middleware is applied in outer-to-inner order (outermost runs first on
/// request, last on response):
///
/// 1. `TraceLayer` — structured request/response logging via `tracing`.
/// 2. `CorsLayer` — permissive CORS for the collect endpoint (the script tag
///    is embedded on third-party sites; browsers need CORS headers).
///
/// Rate limiting (60 req/min per IP on `/api/collect`) is tracked in Sprint 1
/// via Tower middleware.
pub fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(routes::health::health))
        .route("/api/collect", post(routes::collect::collect))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
