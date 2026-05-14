use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::state::AppState;

/// `GET /health` — liveness check.
///
/// Returns `200 OK` when DuckDB is reachable (normal idle state).
/// Returns `503 Service Unavailable` only when DuckDB is unreachable.
///
/// Response shape:
/// ```json
/// { "status": "ok", "version": "0.1.0" }
/// ```
#[tracing::instrument(skip(state))]
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION")
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Health check: DuckDB unreachable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "degraded",
                    "version": env!("CARGO_PKG_VERSION")
                })),
            )
                .into_response()
        }
    }
}
