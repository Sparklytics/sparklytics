use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use sqlx::PgConnection;
use svix::webhooks::Webhook;
use tracing::{error, info, warn};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `POST /webhooks/clerk` — receive and process Clerk webhook events.
///
/// Verification: Svix HMAC-SHA256 via the `svix` crate. The signing secret
/// (`CLERK_WEBHOOK_SIGNING_SECRET`) must be in `whsec_...` format.
///
/// Idempotency: Uses a PostgreSQL transaction to atomically insert the
/// `svix-id` into `processed_webhooks` and process the event. If the INSERT
/// returns 0 rows (ON CONFLICT DO NOTHING), the webhook was already processed
/// and the delivery is silently ignored. If processing fails the transaction
/// is rolled back, removing the idempotency lock so the webhook can be
/// retried.
#[tracing::instrument(skip(state, headers, body))]
pub async fn clerk_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let pool = match state.cloud_pg() {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Webhook handler called outside cloud mode");
            return internal_error("pool unavailable");
        }
    };
    let signing_secret = match state.cloud_cfg() {
        Ok(c) => c.clerk_webhook_signing_secret.clone(),
        Err(e) => {
            error!(error = %e, "Cloud config unavailable in webhook handler");
            return internal_error("config unavailable");
        }
    };

    // --- Signature verification via svix ---
    let svix_id = match header_str(&headers, "svix-id") {
        Some(v) => v,
        None => {
            warn!("Clerk webhook missing svix-id header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": { "code": "unauthorized", "message": "missing svix-id", "field": null } })),
            )
                .into_response();
        }
    };

    let wh = match Webhook::new(&signing_secret) {
        Ok(w) => w,
        Err(e) => {
            error!(error = %e, "Failed to construct Svix Webhook verifier");
            return internal_error("webhook verifier init failed");
        }
    };

    // axum::http::HeaderMap implements svix::webhooks::HeaderMap — pass directly.
    if let Err(e) = wh.verify(&body, &headers) {
        warn!(error = %e, "Clerk webhook signature verification failed");
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": { "code": "unauthorized", "message": "invalid signature", "field": null } })),
        )
            .into_response();
    }

    // --- Parse payload ---
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Malformed Clerk webhook payload");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": { "code": "bad_request", "message": "malformed payload", "field": null } })),
            )
                .into_response();
        }
    };

    let event_type = match payload.get("type").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            warn!("Clerk webhook missing 'type' field");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": { "code": "bad_request", "message": "missing type field", "field": null } })),
            )
                .into_response();
        }
    };
    let data = payload["data"].clone();

    // --- Begin transaction: idempotency lock + event processing are atomic ---
    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to begin webhook transaction");
            return internal_error("transaction begin failed");
        }
    };

    // Try to claim this delivery atomically.
    // ON CONFLICT DO NOTHING means rows_affected() == 0 → already processed.
    // &mut *tx is required (not auto-deref) because Executor<'_> needs &mut PgConnection.
    #[allow(clippy::explicit_auto_deref)]
    let lock_result = sqlx::query(
        "INSERT INTO processed_webhooks (svix_id) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(svix_id)
    .execute(&mut *tx)
    .await;

    match lock_result {
        Ok(r) if r.rows_affected() == 0 => {
            info!(svix_id, "Duplicate webhook delivery — skipping");
            return StatusCode::OK.into_response();
        }
        Ok(_) => {}
        Err(e) => {
            error!(svix_id, error = %e, "Failed to acquire idempotency lock");
            return internal_error("idempotency check failed");
        }
    }

    // --- Dispatch to handler (within the same transaction) ---
    // &mut *tx explicitly dereferences Transaction → PgConnection via DerefMut.
    // Auto-deref cannot be used here because the Executor trait bound requires
    // the concrete &mut PgConnection type — coercion does not apply for generic
    // trait parameters.
    #[allow(clippy::explicit_auto_deref)]
    let result = match event_type.as_str() {
        "organization.created" => process_org_created(&mut *tx, &data).await,
        "organization.updated" => process_org_updated(&mut *tx, &data).await,
        "organization.deleted" => process_org_deleted(&mut *tx, &data).await,
        "organizationMembership.created" => process_membership_created(&mut *tx, &data).await,
        "organizationMembership.deleted" => process_membership_deleted(&mut *tx, &data).await,
        "user.created" => process_user_created(&mut *tx, &data).await,
        "user.updated" => process_user_updated(&mut *tx, &data).await,
        "user.deleted" => process_user_deleted(&mut *tx, &data).await,
        other => {
            warn!(event_type = other, "Unhandled Clerk webhook event type");
            Ok(()) // Not an error — just ignore unknown events
        }
    };

    if let Err(e) = result {
        error!(event_type, svix_id, error = %e, "Clerk webhook processing failed — rolling back");
        // Transaction drops here, rolling back the idempotency lock too.
        return internal_error("webhook processing failed");
    }

    if let Err(e) = tx.commit().await {
        error!(svix_id, error = %e, "Failed to commit webhook transaction");
        return internal_error("transaction commit failed");
    }

    info!(event_type, svix_id, "Clerk webhook processed");
    StatusCode::OK.into_response()
}

// ---------------------------------------------------------------------------
// Event processors — accept &mut PgConnection to participate in the caller's
// transaction.
// ---------------------------------------------------------------------------

async fn process_org_created(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;
    let name = str_field(data, "name")?;
    let slug = str_field(data, "slug")?;

    sqlx::query(
        "INSERT INTO tenants (id, name, slug, plan, event_limit)
         VALUES ($1, $2, $3, 'free', 10000)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .execute(conn)
    .await?;

    info!(org_id = id, "Tenant created from Clerk org");
    Ok(())
}

async fn process_org_updated(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;
    let name = str_field(data, "name")?;
    let slug = str_field(data, "slug")?;

    sqlx::query(
        "UPDATE tenants SET name = $2, slug = $3, updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .execute(conn)
    .await?;

    info!(org_id = id, "Tenant updated from Clerk org");
    Ok(())
}

async fn process_org_deleted(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;

    // Soft-delete the tenant.
    sqlx::query("UPDATE tenants SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&mut *conn)
        .await?;

    // Revoke all active API keys for this tenant (same transaction).
    sqlx::query(
        "UPDATE api_keys SET revoked_at = NOW()
         WHERE tenant_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(conn)
    .await?;

    info!(org_id = id, "Tenant soft-deleted, API keys revoked");
    Ok(())
}

async fn process_membership_created(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    // Clerk payload structure for organizationMembership.created:
    // data.organization.id  = org_id
    // data.public_user_data.user_id = user_id
    // data.role = "org:admin" | "org:member" | "org:viewer"
    let tenant_id = data
        .pointer("/organization/id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing organization.id"))?;
    let user_id = data
        .pointer("/public_user_data/user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing public_user_data.user_id"))?;
    let role_raw = data
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("org:member");
    // Normalise "org:admin" → "admin".
    let role = role_raw.strip_prefix("org:").unwrap_or(role_raw);

    sqlx::query(
        "INSERT INTO tenant_members (tenant_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (tenant_id, user_id) DO UPDATE SET role = $3",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(role)
    .execute(conn)
    .await?;

    info!(tenant_id, user_id, role, "Tenant member added");
    Ok(())
}

async fn process_membership_deleted(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let tenant_id = data
        .pointer("/organization/id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing organization.id"))?;
    let user_id = data
        .pointer("/public_user_data/user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing public_user_data.user_id"))?;

    sqlx::query("DELETE FROM tenant_members WHERE tenant_id = $1 AND user_id = $2")
        .bind(tenant_id)
        .bind(user_id)
        .execute(conn)
        .await?;

    info!(tenant_id, user_id, "Tenant member removed");
    Ok(())
}

async fn process_user_created(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;
    let email = extract_primary_email(data)?;
    let name = extract_full_name(data);

    sqlx::query(
        "INSERT INTO users (id, email, name)
         VALUES ($1, $2, $3)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id)
    .bind(email)
    .bind(name.as_deref())
    .execute(conn)
    .await?;

    info!(user_id = id, "User created");
    Ok(())
}

async fn process_user_updated(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;
    let email = extract_primary_email(data)?;
    let name = extract_full_name(data);

    sqlx::query("UPDATE users SET email = $2, name = $3 WHERE id = $1")
        .bind(id)
        .bind(email)
        .bind(name.as_deref())
        .execute(conn)
        .await?;

    info!(user_id = id, "User updated");
    Ok(())
}

async fn process_user_deleted(conn: &mut PgConnection, data: &Value) -> anyhow::Result<()> {
    let id = str_field(data, "id")?;

    // Delete user — FK cascade removes tenant_members rows.
    // Log for GDPR audit trail.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    info!(user_id = id, "User deleted (GDPR)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn internal_error(msg: &str) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": { "code": "internal_error", "message": msg, "field": null } })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Payload helpers
// ---------------------------------------------------------------------------

fn str_field<'a>(data: &'a Value, field: &str) -> anyhow::Result<&'a str> {
    data.get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing field '{field}' in Clerk webhook payload"))
}

fn extract_primary_email(data: &Value) -> anyhow::Result<String> {
    let primary_id = data
        .get("primary_email_address_id")
        .and_then(|v| v.as_str());

    let emails = data
        .get("email_addresses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing email_addresses"))?;

    // Prefer the primary email address; fall back to the first one.
    for entry in emails {
        let matches_primary = primary_id
            .and_then(|pid| entry.get("id").and_then(|v| v.as_str()).map(|id| id == pid))
            .unwrap_or(false);
        if matches_primary {
            if let Some(email) = entry.get("email_address").and_then(|v| v.as_str()) {
                return Ok(email.to_string());
            }
        }
    }

    // Fallback to first entry.
    emails
        .first()
        .and_then(|e| e.get("email_address").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no email found in Clerk user payload"))
}

fn extract_full_name(data: &Value) -> Option<String> {
    let first = data
        .get("first_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let last = data
        .get("last_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let full = format!("{} {}", first, last).trim().to_string();
    if full.is_empty() {
        None
    } else {
        Some(full)
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}
