use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::{error::AppError, state::AppState};

use super::tenant_context::TenantContext;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListKeysQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/keys` — List API keys for the authenticated tenant (prefix only).
#[tracing::instrument(skip(state))]
pub async fn list_keys(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(q): Query<ListKeysQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let rows = sqlx::query(
        r#"SELECT id, name, key_prefix AS prefix,
                  last_used_at, created_at, revoked_at
           FROM api_keys
           WHERE tenant_id = $1
           ORDER BY created_at DESC
           LIMIT $2 OFFSET $3"#,
    )
    .bind(&tenant.tenant_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let total: i64 =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM api_keys WHERE tenant_id = $1")
            .bind(&tenant.tenant_id)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let data: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "name": r.get::<String, _>("name"),
                "prefix": r.get::<String, _>("prefix"),
                "last_used_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_used_at"),
                "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                "revoked_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("revoked_at"),
            })
        })
        .collect();

    Ok(Json(json!({
        "data": data,
        "pagination": {
            "total": total,
            "limit": limit,
            "offset": offset,
            "has_more": offset + limit < total,
        }
    })))
}

/// `POST /api/keys` — Create a new API key.
///
/// Returns the full raw key ONCE in the response. Only the sha256 hash is
/// stored in the database — the raw key cannot be retrieved later.
#[tracing::instrument(skip(state))]
pub async fn create_key(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(body): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    // Generate: "spk_live_" + 32 random alphanumeric chars.
    let random_suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let raw_key = format!("spk_live_{}", random_suffix);

    // Hash the raw key — only this is stored.
    let hash = format!("{:x}", Sha256::digest(raw_key.as_bytes()));

    // Store the first 20 chars of the raw key as the display prefix.
    let prefix = raw_key.chars().take(20).collect::<String>();

    let id = format!("key_{}", nanoid());
    let now = chrono::Utc::now();

    sqlx::query(
        "INSERT INTO api_keys (id, tenant_id, user_id, name, key_hash, key_prefix, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .bind(&tenant.user_id)
    .bind(body.name.trim())
    .bind(&hash)
    .bind(&prefix)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": {
                "id": id,
                "name": body.name.trim(),
                "key": raw_key,
                "prefix": prefix,
                "created_at": now.to_rfc3339(),
            }
        })),
    ))
}

/// `DELETE /api/keys/:key_id` — Revoke a key (sets `revoked_at = NOW()`).
#[tracing::instrument(skip(state))]
pub async fn revoke_key(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(key_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    let result = sqlx::query(
        "UPDATE api_keys SET revoked_at = NOW()
         WHERE id = $1 AND tenant_id = $2 AND revoked_at IS NULL",
    )
    .bind(&key_id)
    .bind(&tenant.tenant_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "API key not found or already revoked".to_string(),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Authenticate a request using a cloud API key (`spk_live_...`).
///
/// Returns the `tenant_id` the key belongs to, or `None` if the key is not
/// found, revoked, or the prefix doesn't match.
pub async fn authenticate_api_key(
    pool: &sqlx::PgPool,
    raw_key: &str,
) -> anyhow::Result<Option<String>> {
    if !raw_key.starts_with("spk_live_") {
        return Ok(None);
    }

    let hash = format!("{:x}", Sha256::digest(raw_key.as_bytes()));

    let row = sqlx::query(
        "SELECT tenant_id FROM api_keys
         WHERE key_hash = $1 AND revoked_at IS NULL",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = row {
        let tenant_id: String = r.get("tenant_id");
        // Fire-and-forget: update last_used_at asynchronously.
        let pool = pool.clone();
        let hash_clone = hash.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE key_hash = $1")
                .bind(hash_clone)
                .execute(&pool)
                .await;
        });

        return Ok(Some(tenant_id));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a short random ID for new API key records.
fn nanoid() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}
