use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::config::AuthMode;

use crate::{error::AppError, routes::collect, state::AppState};

use super::api_keys::{generate_api_key, generate_key_id};
use super::jwt::{decode_jwt, encode_jwt};
use super::password::{hash_password, validate_password_strength, verify_password};

const LOGIN_RATE_LIMIT_RETRY_AFTER_SECONDS: u64 = 15 * 60;

// ---------------------------------------------------------------------------
// GET /api/auth/status
// ---------------------------------------------------------------------------

/// `GET /api/auth/status` — Public, no auth required.
///
/// Flat response (no `{"data":...}` wrapper). Never returns 401.
/// Returns 404 in `none` mode (endpoint not registered).
pub async fn auth_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (mode_str, setup_required) = match &state.config.auth_mode {
        AuthMode::Password(_) => ("password", false),
        AuthMode::Local => {
            let configured = state
                .db
                .is_admin_configured()
                .await
                .map_err(AppError::Internal)?;
            ("local", !configured)
        }
        AuthMode::None => {
            // Should not be reachable if router is set up correctly.
            return Err(AppError::NotFound("Not found".to_string()));
        }
    };

    // Check if the user is authenticated (cookie JWT).
    let authenticated = is_cookie_authenticated(&state, &headers).await;

    Ok(Json(json!({
        "mode": mode_str,
        "setup_required": setup_required,
        "authenticated": authenticated,
    })))
}

// ---------------------------------------------------------------------------
// POST /api/auth/setup
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub password: String,
}

/// `POST /api/auth/setup` — First-run setup (local mode only).
///
/// Returns 201 first time, 410 after setup is complete.
pub async fn auth_setup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> Result<impl IntoResponse, AppError> {
    match &state.config.auth_mode {
        AuthMode::Local => {}
        _ => {
            return Err(AppError::BadRequest(
                "setup only available in local mode".to_string(),
            ))
        }
    }

    // Check if already configured.
    let configured = state
        .db
        .is_admin_configured()
        .await
        .map_err(AppError::Internal)?;
    if configured {
        return Err(AppError::Gone);
    }

    validate_password_strength(&req.password).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let hash =
        hash_password(&req.password, state.config.argon2_memory_kb).map_err(AppError::Internal)?;

    state
        .db
        .set_setting("admin_password_hash", &hash)
        .await
        .map_err(AppError::Internal)?;

    Ok((StatusCode::CREATED, Json(json!({ "data": { "ok": true } }))))
}

// ---------------------------------------------------------------------------
// POST /api/auth/login
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

/// `POST /api/auth/login` — Login with password.
///
/// Rate limited: 5 failed attempts per 15 min per IP.
pub async fn auth_login(
    State(state): State<Arc<AppState>>,
    maybe_connect_info: collect::MaybeConnectInfo,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = collect::extract_client_ip(&headers, maybe_connect_info.0);

    // Check rate limit.
    let allowed = state
        .db
        .check_login_rate_limit(&client_ip)
        .await
        .map_err(AppError::Internal)?;
    if !allowed {
        return Err(AppError::RateLimitedWithRetry {
            retry_after_seconds: LOGIN_RATE_LIMIT_RETRY_AFTER_SECONDS,
        });
    }

    // Get the expected password hash.
    let expected_hash = match &state.config.auth_mode {
        AuthMode::Password(pw) => {
            // For password mode, we verify against the env var directly.
            if req.password != *pw {
                state
                    .db
                    .record_login_attempt(&client_ip, false)
                    .await
                    .map_err(AppError::Internal)?;
                return Err(AppError::Unauthorized);
            }
            // Password mode doesn't use stored hash — generate JWT directly.
            let jwt_secret = state
                .db
                .ensure_jwt_secret()
                .await
                .map_err(AppError::Internal)?;
            let (token, expires_at) =
                encode_jwt(&jwt_secret, state.config.session_days).map_err(AppError::Internal)?;

            state
                .db
                .record_login_attempt(&client_ip, true)
                .await
                .map_err(AppError::Internal)?;

            let cookie =
                build_session_cookie(&token, state.config.https, state.config.session_days);
            return Ok((
                StatusCode::OK,
                [(header::SET_COOKIE, cookie)],
                Json(json!({ "data": { "expires_at": expires_at } })),
            ));
        }
        AuthMode::Local => {
            match state
                .db
                .get_setting("admin_password_hash")
                .await
                .map_err(AppError::Internal)?
            {
                Some(hash) => hash,
                None => return Err(AppError::SetupRequired),
            }
        }
        AuthMode::None => {
            return Err(AppError::NotFound("Auth not enabled".to_string()));
        }
    };

    // Verify password.
    if !verify_password(&req.password, &expected_hash) {
        state
            .db
            .record_login_attempt(&client_ip, false)
            .await
            .map_err(AppError::Internal)?;
        return Err(AppError::Unauthorized);
    }

    state
        .db
        .record_login_attempt(&client_ip, true)
        .await
        .map_err(AppError::Internal)?;

    let jwt_secret = state
        .db
        .ensure_jwt_secret()
        .await
        .map_err(AppError::Internal)?;
    let (token, expires_at) =
        encode_jwt(&jwt_secret, state.config.session_days).map_err(AppError::Internal)?;

    let cookie = build_session_cookie(&token, state.config.https, state.config.session_days);
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "data": { "expires_at": expires_at } })),
    ))
}

// ---------------------------------------------------------------------------
// POST /api/auth/logout
// ---------------------------------------------------------------------------

/// `POST /api/auth/logout` — Clear session cookie. Always 200.
pub async fn auth_logout(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cookie = clear_session_cookie(state.config.https);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "data": { "ok": true } })),
    )
}

// ---------------------------------------------------------------------------
// GET /api/auth/session
// ---------------------------------------------------------------------------

/// `GET /api/auth/session` — Returns 401 if not authed. Returns session info if authed.
pub async fn auth_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let cookie_header = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = cookie_header
        .split(';')
        .find_map(|c| c.trim().strip_prefix("spk_session="));

    let token = match token {
        Some(t) => t,
        None => return Err(AppError::Unauthorized),
    };

    let jwt_secret = state
        .db
        .get_setting("jwt_secret")
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::Unauthorized)?;

    let claims = decode_jwt(token, &jwt_secret).map_err(|_| AppError::Unauthorized)?;

    let mode_str = match &state.config.auth_mode {
        AuthMode::Password(_) => "password",
        AuthMode::Local => "local",
        AuthMode::None => "none",
    };

    Ok(Json(json!({
        "data": {
            "authenticated": true,
            "mode": mode_str,
            "expires_at": chrono::DateTime::from_timestamp(claims.exp, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
        }
    })))
}

// ---------------------------------------------------------------------------
// PUT /api/auth/password
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// `PUT /api/auth/password` — Change password (local mode only).
///
/// Rotates jwt_secret to invalidate all existing sessions.
pub async fn auth_change_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    match &state.config.auth_mode {
        AuthMode::Local => {}
        AuthMode::Password(_) => {
            return Err(AppError::MethodNotAllowed);
        }
        AuthMode::None => {
            return Err(AppError::NotFound("Auth not enabled".to_string()));
        }
    }

    // Verify current password.
    let current_hash = state
        .db
        .get_setting("admin_password_hash")
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::SetupRequired)?;

    if !verify_password(&req.current_password, &current_hash) {
        return Err(AppError::Unauthorized);
    }

    validate_password_strength(&req.new_password)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Hash new password.
    let new_hash = hash_password(&req.new_password, state.config.argon2_memory_kb)
        .map_err(AppError::Internal)?;
    state
        .db
        .set_setting("admin_password_hash", &new_hash)
        .await
        .map_err(AppError::Internal)?;

    // Rotate JWT secret to invalidate all sessions.
    let new_secret = {
        use rand::RngCore;
        let mut buf = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut buf);
        hex::encode(buf)
    };
    state
        .db
        .set_setting("jwt_secret", &new_secret)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": { "ok": true } })))
}

// ---------------------------------------------------------------------------
// GET /api/auth/keys
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListKeysQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// `GET /api/auth/keys` — List API keys (cookie-only).
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListKeysQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let (keys, total) = state
        .db
        .list_api_keys(limit, offset)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": keys,
        "pagination": {
            "total": total,
            "limit": limit,
            "offset": offset,
            "has_more": offset + limit < total,
        }
    })))
}

// ---------------------------------------------------------------------------
// POST /api/auth/keys
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

/// `POST /api/auth/keys` — Create API key (cookie-only). Returns raw key once.
pub async fn create_api_key_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    let key_id = generate_key_id();
    let (raw_key, hash, prefix) = generate_api_key();
    state
        .db
        .create_api_key(&key_id, &req.name, &hash, &prefix)
        .await
        .map_err(AppError::Internal)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": {
                "id": key_id,
                "name": req.name,
                "key": raw_key,
                "prefix": prefix,
                "created_at": chrono::Utc::now().to_rfc3339(),
            }
        })),
    ))
}

// ---------------------------------------------------------------------------
// DELETE /api/auth/keys/:id
// ---------------------------------------------------------------------------

/// `DELETE /api/auth/keys/:id` — Revoke API key (cookie-only).
pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Path(key_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let revoked = state
        .db
        .revoke_api_key(&key_id)
        .await
        .map_err(AppError::Internal)?;

    if !revoked {
        return Err(AppError::NotFound("API key not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_session_cookie(token: &str, https: bool, session_days: u32) -> String {
    let secure = if https { "; Secure" } else { "" };
    format!(
        "spk_session={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        token,
        u64::from(session_days) * 86_400,
        secure,
    )
}

fn clear_session_cookie(https: bool) -> String {
    let secure = if https { "; Secure" } else { "" };
    format!(
        "spk_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        secure,
    )
}

async fn is_cookie_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    let cookie_header = match headers.get(header::COOKIE) {
        Some(v) => match v.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return false,
        },
        None => return false,
    };

    let token = match cookie_header
        .split(';')
        .find_map(|c| c.trim().strip_prefix("spk_session="))
    {
        Some(t) => t.to_string(),
        None => return false,
    };

    let jwt_secret = match state.db.get_setting("jwt_secret").await {
        Ok(Some(s)) => s,
        _ => return false,
    };

    decode_jwt(&token, &jwt_secret).is_ok()
}
