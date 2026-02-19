use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;

use crate::{error::AppError, state::AppState};

use super::{clickhouse, clickhouse::ChFilters, tenant_context::TenantContext};

// ---------------------------------------------------------------------------
// Shared query params (mirrors the self-hosted route structs)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CloudStatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub timezone: Option<String>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloudPageviewsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub granularity: Option<String>,
    pub timezone: Option<String>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloudMetricsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(rename = "type")]
    pub metric_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_dates(start_str: Option<&str>, end_str: Option<&str>) -> (NaiveDate, NaiveDate) {
    let today = chrono::Utc::now().date_naive();
    let start = start_str
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(6));
    let end = end_str
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    (start, end)
}

async fn check_website_ownership(
    state: &AppState,
    website_id: &str,
    tenant_id: &str,
) -> Result<(), AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;
    let owned = clickhouse::website_belongs_to_tenant(pool, website_id, tenant_id)
        .await
        .map_err(AppError::Internal)?;
    if !owned {
        Err(AppError::NotFound("Website not found".to_string()))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Cloud analytics handlers
// ---------------------------------------------------------------------------

/// `GET /api/websites/:id/stats` (cloud mode)
#[tracing::instrument(skip(state))]
pub async fn cloud_stats(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
    Query(q): Query<CloudStatsQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let (start, end) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());
    let tz = q.timezone.as_deref().unwrap_or("UTC");
    let filters = ChFilters {
        country: q.filter_country.as_deref(),
        page: q.filter_page.as_deref(),
        referrer: q.filter_referrer.as_deref(),
        browser: q.filter_browser.as_deref(),
        os: q.filter_os.as_deref(),
        device: q.filter_device.as_deref(),
        utm_source: q.filter_utm_source.as_deref(),
        utm_medium: q.filter_utm_medium.as_deref(),
        utm_campaign: q.filter_utm_campaign.as_deref(),
    };

    let ch = state.cloud_ch().map_err(AppError::Internal)?;
    let result =
        clickhouse::ch_stats(ch, &tenant.tenant_id, &website_id, &start, &end, tz, &filters)
            .await
            .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}

/// `GET /api/websites/:id/pageviews` (cloud mode)
#[tracing::instrument(skip(state))]
pub async fn cloud_pageviews(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
    Query(q): Query<CloudPageviewsQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let (start, end) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());
    let filters = ChFilters {
        country: q.filter_country.as_deref(),
        page: q.filter_page.as_deref(),
        referrer: q.filter_referrer.as_deref(),
        browser: q.filter_browser.as_deref(),
        os: q.filter_os.as_deref(),
        device: q.filter_device.as_deref(),
        utm_source: q.filter_utm_source.as_deref(),
        utm_medium: q.filter_utm_medium.as_deref(),
        utm_campaign: q.filter_utm_campaign.as_deref(),
    };

    let ch = state.cloud_ch().map_err(AppError::Internal)?;
    let result = clickhouse::ch_pageviews(
        ch,
        &tenant.tenant_id,
        &website_id,
        &start,
        &end,
        q.granularity.as_deref(),
        &filters,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "series": result.series,
            "granularity": result.granularity,
        }
    })))
}

/// `GET /api/websites/:id/metrics` (cloud mode)
#[tracing::instrument(skip(state))]
pub async fn cloud_metrics(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
    Query(q): Query<CloudMetricsQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let metric_type = match q.metric_type.as_deref() {
        Some(t) => t,
        None => return Err(AppError::BadRequest("type parameter is required".to_string())),
    };
    if !sparklytics_duckdb::queries::metrics::is_valid_metric_type(metric_type) {
        return Err(AppError::BadRequest(format!(
            "invalid metric type: {metric_type}"
        )));
    }

    let (start, end) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);
    let filters = ChFilters {
        country: q.filter_country.as_deref(),
        page: q.filter_page.as_deref(),
        referrer: q.filter_referrer.as_deref(),
        browser: q.filter_browser.as_deref(),
        os: q.filter_os.as_deref(),
        device: q.filter_device.as_deref(),
        utm_source: q.filter_utm_source.as_deref(),
        utm_medium: q.filter_utm_medium.as_deref(),
        utm_campaign: q.filter_utm_campaign.as_deref(),
    };

    let ch = state.cloud_ch().map_err(AppError::Internal)?;
    let (result, pagination) = clickhouse::ch_metrics(
        ch,
        &tenant.tenant_id,
        &website_id,
        &start,
        &end,
        metric_type,
        limit,
        offset,
        &filters,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": result,
        "pagination": pagination,
    })))
}

/// `GET /api/websites/:id/realtime` (cloud mode)
#[tracing::instrument(skip(state))]
pub async fn cloud_realtime(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let ch = state.cloud_ch().map_err(AppError::Internal)?;
    let result = clickhouse::ch_realtime(ch, &tenant.tenant_id, &website_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}

/// `GET /api/websites` (cloud mode) — list websites for the authenticated tenant.
#[tracing::instrument(skip(state))]
pub async fn cloud_list_websites(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(q): Query<CloudListWebsitesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);

    let rows = sqlx::query(
        "SELECT id, tenant_id, name, domain, timezone, CAST(created_at AS TEXT) AS created_at
         FROM websites
         WHERE tenant_id = $1
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(&tenant.tenant_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let total: i64 =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM websites WHERE tenant_id = $1")
            .bind(&tenant.tenant_id)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let data: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "tenant_id": r.get::<String, _>("tenant_id"),
                "name": r.get::<String, _>("name"),
                "domain": r.get::<String, _>("domain"),
                "timezone": r.get::<Option<String>, _>("timezone"),
                "created_at": r.get::<Option<String>, _>("created_at"),
            })
        })
        .collect();

    Ok(Json(json!({
        "data": data,
        "pagination": {
            "total": total,
            "limit": limit,
            "cursor": null,
            "has_more": false,
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct CloudListWebsitesQuery {
    pub limit: Option<i64>,
}

/// `POST /api/websites` (cloud mode) — create website for the authenticated tenant.
#[tracing::instrument(skip(state))]
pub async fn cloud_create_website(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(body): Json<CloudCreateWebsiteBody>,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    if body.name.trim().is_empty() || body.domain.trim().is_empty() {
        return Err(AppError::BadRequest(
            "name and domain are required".to_string(),
        ));
    }

    let id = format!("site_{}", nanoid());
    let tz = body.timezone.as_deref().unwrap_or("UTC");
    let now = chrono::Utc::now();
    let tracking_snippet = format!(
        r#"<script defer src="{}/s.js" data-website-id="{}"></script>"#,
        state.config.public_url, id
    );

    sqlx::query(
        "INSERT INTO websites (id, tenant_id, name, domain, timezone, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
    )
    .bind(&id)
    .bind(&tenant.tenant_id)
    .bind(body.name.trim())
    .bind(body.domain.trim())
    .bind(tz)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": {
                "id": id,
                "tenant_id": tenant.tenant_id,
                "name": body.name.trim(),
                "domain": body.domain.trim(),
                "timezone": tz,
                "tracking_snippet": tracking_snippet,
                "created_at": now.to_rfc3339(),
            }
        })),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CloudCreateWebsiteBody {
    pub name: String,
    pub domain: String,
    pub timezone: Option<String>,
}

/// `PUT /api/websites/:id` (cloud mode) — update name, domain, or timezone.
#[tracing::instrument(skip(state))]
pub async fn cloud_update_website(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
    Json(body): Json<CloudUpdateWebsiteBody>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    // Build a selective update — only patch fields provided in the request.
    // We always set updated_at.
    let result = sqlx::query(
        "UPDATE websites
         SET name       = COALESCE($3, name),
             domain     = COALESCE($4, domain),
             timezone   = COALESCE($5, timezone),
             updated_at = NOW()
         WHERE id = $1 AND tenant_id = $2
         RETURNING id, tenant_id, name, domain, timezone,
                   CAST(created_at AS TEXT) AS created_at,
                   CAST(updated_at AS TEXT) AS updated_at",
    )
    .bind(&website_id)
    .bind(&tenant.tenant_id)
    .bind(body.name.as_deref())
    .bind(body.domain.as_deref())
    .bind(body.timezone.as_deref())
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    match result {
        None => Err(AppError::NotFound("Website not found".to_string())),
        Some(row) => Ok(Json(json!({
            "data": {
                "id":         row.get::<String, _>("id"),
                "tenant_id":  row.get::<String, _>("tenant_id"),
                "name":       row.get::<String, _>("name"),
                "domain":     row.get::<String, _>("domain"),
                "timezone":   row.get::<Option<String>, _>("timezone"),
                "created_at": row.get::<Option<String>, _>("created_at"),
                "updated_at": row.get::<Option<String>, _>("updated_at"),
            }
        }))),
    }
}

#[derive(Debug, Deserialize)]
pub struct CloudUpdateWebsiteBody {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub timezone: Option<String>,
}

/// `DELETE /api/websites/:id` (cloud mode)
#[tracing::instrument(skip(state))]
pub async fn cloud_delete_website(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let pool = state.cloud_pg().map_err(AppError::Internal)?;
    let result = sqlx::query("DELETE FROM websites WHERE id = $1 AND tenant_id = $2")
        .bind(&website_id)
        .bind(&tenant.tenant_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

fn nanoid() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

/// `GET /api/usage` (cloud mode) — current-month event usage for the tenant.
#[tracing::instrument(skip(state))]
pub async fn get_usage(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
) -> Result<impl IntoResponse, AppError> {
    let pool = state.cloud_pg().map_err(AppError::Internal)?;
    let info = super::usage::get_usage(pool, &tenant.tenant_id)
        .await
        .map_err(AppError::Internal)?;

    let percent_used = if info.event_limit > 0 {
        (info.event_count as f64 / info.event_limit as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(json!({
        "data": {
            "month": info.month.to_string(),
            "event_count": info.event_count,
            "event_limit": info.event_limit,
            "percent_used": (percent_used * 10.0).round() / 10.0,
            "plan": info.plan,
        }
    })))
}

// ---------------------------------------------------------------------------
// Share management (cloud mode)
// ---------------------------------------------------------------------------

/// `POST /api/websites/:id/share` (cloud mode) — enable public sharing.
///
/// Idempotent: if a `share_id` already exists, returns it (201).
#[tracing::instrument(skip(state))]
pub async fn enable_website_sharing(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    // Atomic upsert: COALESCE keeps the existing share_id if already set,
    // otherwise writes the new UUID. One round-trip; no TOCTOU race window.
    let new_id = uuid::Uuid::new_v4().to_string();
    let share_id: String = sqlx::query_scalar(
        r#"UPDATE websites
           SET    share_id   = COALESCE(share_id, $1),
                  updated_at = NOW()
           WHERE  id = $2 AND tenant_id = $3
           RETURNING share_id"#,
    )
    .bind(&new_id)
    .bind(&website_id)
    .bind(&tenant.tenant_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let share_url = format!("{}/share/{}", state.config.public_url, share_id);

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": {
                "share_id": share_id,
                "share_url": share_url,
            }
        })),
    ))
}

/// `DELETE /api/websites/:id/share` (cloud mode) — disable public sharing.
#[tracing::instrument(skip(state))]
pub async fn disable_website_sharing(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let pool = state.cloud_pg().map_err(AppError::Internal)?;

    // Return 404 if sharing was never enabled (share_id already NULL).
    let current: Option<Option<String>> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT share_id FROM websites WHERE id = $1 AND tenant_id = $2",
    )
    .bind(&website_id)
    .bind(&tenant.tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    if current.flatten().is_none() {
        return Err(AppError::NotFound(
            "Sharing is not enabled for this website".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE websites SET share_id = NULL, updated_at = NOW() WHERE id = $1 AND tenant_id = $2",
    )
    .bind(&website_id)
    .bind(&tenant.tenant_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Cloud export (proxies ClickHouse HTTP with FORMAT CSVWithNames)
// ---------------------------------------------------------------------------

/// `GET /api/websites/:id/export` (cloud mode) — download events as CSV.
///
/// Validates ownership and date range, then streams ClickHouse rows using
/// `FORMAT CSVWithNames` so headers are included automatically. Row cap is
/// enforced by counting newlines in the response body.
#[tracing::instrument(skip(state))]
pub async fn cloud_export_events(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(website_id): Path<String>,
    Query(q): Query<crate::routes::export::ExportQuery>,
) -> Result<Response, AppError> {
    check_website_ownership(&state, &website_id, &tenant.tenant_id).await?;

    let start = NaiveDate::parse_from_str(&q.start_date, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("invalid start_date format, expected YYYY-MM-DD".to_string()))?;
    let end = NaiveDate::parse_from_str(&q.end_date, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("invalid end_date format, expected YYYY-MM-DD".to_string()))?;

    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }

    let range_days = (end - start).num_days() + 1;
    if range_days > 90 {
        return Err(AppError::BadRequest(format!(
            "date range too large: {range_days} days (max 90)"
        )));
    }

    if let Some(ref fmt) = q.format {
        if fmt != "csv" {
            return Err(AppError::BadRequest(format!(
                "unsupported format: {fmt}; only 'csv' is supported"
            )));
        }
    }

    let ch = state.cloud_ch().map_err(AppError::Internal)?;
    let end_exclusive = end + chrono::Duration::days(1);
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end_exclusive.format("%Y-%m-%d").to_string();

    // ClickHouse FORMAT CSVWithNames emits header + data rows.
    // LIMIT 500001 lets us detect the overflow case by counting newlines.
    let sql = concat!(
        "SELECT id, website_id, event_type, url, referrer_domain, event_name,\n",
        "       country, browser, os, device_type, language,\n",
        "       utm_source, utm_medium, utm_campaign,\n",
        "       formatDateTime(created_at, '%Y-%m-%d %H:%M:%S') AS created_at\n",
        "FROM events\n",
        "WHERE tenant_id = {tenant_id:String}\n",
        "  AND website_id = {website_id:String}\n",
        "  AND created_at >= {start:DateTime64(3)}\n",
        "  AND created_at <  {end:DateTime64(3)}\n",
        "ORDER BY created_at\n",
        "LIMIT 500001\n",
        "FORMAT CSVWithNames",
    );

    let csv_bytes = ch
        .query_raw_bytes(sql, &[
            ("tenant_id", tenant.tenant_id.as_str()),
            ("website_id", website_id.as_str()),
            ("start", start_str.as_str()),
            ("end", end_str.as_str()),
        ])
        .await
        .map_err(AppError::Internal)?;

    // Count data rows (newlines minus 1 for header row).
    let row_count = csv_bytes.iter().filter(|&&b| b == b'\n').count().saturating_sub(1);
    if row_count > 500_000 {
        return Err(AppError::BadRequest(
            "result set too large: > 500000 rows; narrow the date range".to_string(),
        ));
    }

    let filename = format!("events-{}-{}-{}.csv", website_id, q.start_date, q.end_date);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(csv_bytes))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("response build failed: {e}")))
}
