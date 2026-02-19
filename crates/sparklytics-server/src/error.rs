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

    /// Cloud-mode billing gate blocked the request (plan event limit exceeded).
    #[error("plan limit exceeded")]
    PlanLimitExceeded,

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.as_str()),
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "validation_error", msg.as_str())
            }
            AppError::BatchTooLarge(_) => (
                StatusCode::BAD_REQUEST,
                "batch_too_large",
                "Batch exceeds maximum of 50 events",
            ),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "Unauthorized"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "Forbidden"),
            AppError::SetupRequired => (
                StatusCode::FORBIDDEN,
                "setup_required",
                "Admin setup required. POST /api/auth/setup first.",
            ),
            AppError::Gone => (StatusCode::GONE, "gone", "Setup already completed"),
            AppError::MethodNotAllowed => (
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "Method not allowed for this auth mode",
            ),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Rate limit exceeded",
            ),
            AppError::PayloadTooLarge => (
                StatusCode::BAD_REQUEST,
                "payload_too_large",
                "Payload exceeds size limit",
            ),
            AppError::PlanLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "plan_limit_exceeded",
                "Event limit reached",
            ),
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error",
                )
            }
        };

        (
            status,
            Json(json!({
                "error": {
                    "code": code,
                    "message": message,
                    "field": null
                }
            })),
        )
            .into_response()
    }
}
