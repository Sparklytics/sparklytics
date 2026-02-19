use std::sync::Arc;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;

use super::api_keys::hash_api_key;
use super::jwt::decode_jwt;

/// Auth context injected into request extensions after successful auth.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub auth_type: String,
    pub api_key_id: Option<String>,
}

/// Internal: require authentication via Bearer API key or cookie JWT.
pub async fn require_auth(state: Arc<AppState>, request: Request, next: Next) -> Response {
    require_auth_inner(state, request, next, false).await
}

/// Internal: require cookie-only authentication.
pub async fn require_cookie_auth(state: Arc<AppState>, request: Request, next: Next) -> Response {
    require_auth_inner(state, request, next, true).await
}

async fn require_auth_inner(
    state: Arc<AppState>,
    mut request: Request,
    next: Next,
    cookie_only: bool,
) -> Response {
    // Check if setup is required in local mode.
    if let sparklytics_core::config::AuthMode::Local = &state.config.auth_mode {
        match state.db.is_admin_configured().await {
            Ok(false) => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": {
                            "code": "setup_required",
                            "message": "Admin setup required. POST /api/auth/setup first.",
                            "field": null
                        }
                    })),
                )
                    .into_response();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to check admin configured");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            _ => {}
        }
    }

    // Check for Bearer API key.
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if cookie_only {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "error": {
                                "code": "forbidden",
                                "message": "API keys cannot manage auth resources",
                                "field": null
                            }
                        })),
                    )
                        .into_response();
                }

                let key_hash = hash_api_key(token);
                match state.db.lookup_api_key(&key_hash).await {
                    Ok(Some(key_record)) => {
                        let key_id = key_record.id.clone();
                        request.extensions_mut().insert(AuthContext {
                            auth_type: "api_key".to_string(),
                            api_key_id: Some(key_record.id),
                        });
                        let resp = next.run(request).await;
                        // Fire-and-forget: update last_used_at.
                        let db = state.db.clone();
                        tokio::spawn(async move {
                            let _ = db.touch_api_key(&key_id).await;
                        });
                        return resp;
                    }
                    Ok(None) => return unauthorized_response(),
                    Err(e) => {
                        tracing::error!(error = %e, "API key lookup failed");
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                }
            }
        }
    }

    // Try cookie JWT.
    // Extract token from headers synchronously to avoid holding &Request across await.
    let cookie_token = request
        .headers()
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str
                .split(';')
                .find_map(|c| c.trim().strip_prefix("spk_session="))
                .map(|t| t.to_string())
        });

    if let Some(token) = cookie_token {
        if let Some(ctx) = validate_cookie_jwt(&state, &token).await {
            request.extensions_mut().insert(ctx);
            return next.run(request).await;
        }
    }

    unauthorized_response()
}

async fn validate_cookie_jwt(state: &AppState, token: &str) -> Option<AuthContext> {
    let jwt_secret = state.db.get_setting("jwt_secret").await.ok()??;
    let _claims = decode_jwt(token, &jwt_secret).ok()?;

    Some(AuthContext {
        auth_type: "cookie".to_string(),
        api_key_id: None,
    })
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "code": "unauthorized",
                "message": "Not authenticated",
                "field": null
            }
        })),
    )
        .into_response()
}
