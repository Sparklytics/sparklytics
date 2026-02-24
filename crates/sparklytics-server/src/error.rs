use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Application-level errors that map directly to HTTP responses.
///
/// Every variant implements [`IntoResponse`] so Axum handlers can use
/// `Result<impl IntoResponse, AppError>` as their return type.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("batch too large: {0} events (max 50)")]
    BatchTooLarge(usize),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("organization context required")]
    OrganizationContextRequired,

    #[error("setup required")]
    SetupRequired,

    #[error("gone")]
    Gone,

    #[error("method not allowed")]
    MethodNotAllowed,

    #[error("rate limited")]
    RateLimited,

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("ingest queue overloaded")]
    IngestOverloaded { retry_after_seconds: u64 },

    /// Cloud-mode billing gate blocked the request (plan event limit exceeded).
    #[error("plan limit exceeded")]
    PlanLimitExceeded,

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, retry_after_seconds) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.as_str(), None),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                msg.as_str(),
                None,
            ),
            AppError::BatchTooLarge(_) => (
                StatusCode::BAD_REQUEST,
                "batch_too_large",
                "Batch exceeds maximum of 50 events",
                None,
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized",
                None,
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Forbidden", None),
            AppError::OrganizationContextRequired => (
                StatusCode::FORBIDDEN,
                "organization_context_required",
                "Organization context required",
                None,
            ),
            AppError::SetupRequired => (
                StatusCode::FORBIDDEN,
                "setup_required",
                "Admin setup required. POST /api/auth/setup first.",
                None,
            ),
            AppError::Gone => (StatusCode::GONE, "gone", "Setup already completed", None),
            AppError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "Method not allowed for this auth mode",
                None,
            ),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Rate limit exceeded",
                None,
            ),
            AppError::PayloadTooLarge => (
                StatusCode::BAD_REQUEST,
                "payload_too_large",
                "Payload exceeds size limit",
                None,
            ),
            AppError::IngestOverloaded {
                retry_after_seconds,
            } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "ingest_overloaded",
                "Ingestion queue is overloaded, retry later",
                Some(*retry_after_seconds),
            ),
            AppError::PlanLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "plan_limit_exceeded",
                "Event limit reached",
                None,
            ),
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error",
                    None,
                )
            }
        };

        let mut response = (
            status,
            Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                    "field": null
                }
            })),
        )
            .into_response();

        if let Some(retry_after_seconds) = retry_after_seconds {
            if let Ok(value) = retry_after_seconds.to_string().parse() {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }

        response
    }
}
