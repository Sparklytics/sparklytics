use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use sparklytics_core::analytics::AnalyticsFilter;

use crate::{error::AppError, state::AppState};

/// Rate limit for public share endpoints: 30 req/min per IP.
const SHARE_RATE_LIMIT: usize = 30;

// ---------------------------------------------------------------------------
// Query param structs (date range only — no filter params on public share)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ShareDateQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShareMetricsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(rename = "type")]
    pub metric_type: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract client IP from `X-Forwarded-For` or fall back to "unknown".
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn parse_dates(start: Option<&str>, end: Option<&str>) -> (NaiveDate, NaiveDate) {
    let today = chrono::Utc::now().date_naive();
    let s = start
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(6));
    let e = end
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    (s, e)
}

/// Resolve a `share_id` to a `website_id`, applying the share rate limit.
///
/// Returns `AppError::NotFound` when the share_id is unknown and
/// `AppError::RateLimited` when the IP exceeds 30 req/min.
async fn resolve_share(
    state: &AppState,
    share_id: &str,
    headers: &HeaderMap,
) -> Result<String, AppError> {
    let ip = client_ip(headers);
    if !state.check_rate_limit_with_max(&ip, SHARE_RATE_LIMIT).await {
        return Err(AppError::RateLimited);
    }

    let row = state
        .db
        .get_website_by_share_id(share_id)
        .await
        .map_err(AppError::Internal)?;

    match row {
        Some((website_id, _tenant_id)) => Ok(website_id),
        None => Err(AppError::NotFound("Share link not found".to_string())),
    }
}

// ---------------------------------------------------------------------------
// Public share analytics endpoints (no auth)
// ---------------------------------------------------------------------------

/// `GET /api/share/:share_id/stats`
#[tracing::instrument(skip(state))]
pub async fn share_stats(
    State(state): State<Arc<AppState>>,
    Path(share_id): Path<String>,
    Query(q): Query<ShareDateQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let website_id = resolve_share(&state, &share_id, &headers).await?;
    let (start_date, end_date) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());

    let result = state
        .analytics
        .get_stats(
            &website_id,
            None,
            &AnalyticsFilter {
                start_date,
                end_date,
                timezone: None,
                filter_country: None,
                filter_page: None,
                filter_referrer: None,
                filter_browser: None,
                filter_os: None,
                filter_device: None,
                filter_language: None,
                filter_utm_source: None,
                filter_utm_medium: None,
                filter_utm_campaign: None,
            },
        )
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}

/// `GET /api/share/:share_id/pageviews`
#[tracing::instrument(skip(state))]
pub async fn share_pageviews(
    State(state): State<Arc<AppState>>,
    Path(share_id): Path<String>,
    Query(q): Query<ShareDateQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let website_id = resolve_share(&state, &share_id, &headers).await?;
    let (start_date, end_date) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());

    let result = state
        .analytics
        .get_timeseries(
            &website_id,
            None,
            &AnalyticsFilter {
                start_date,
                end_date,
                timezone: None,
                filter_country: None,
                filter_page: None,
                filter_referrer: None,
                filter_browser: None,
                filter_os: None,
                filter_device: None,
                filter_language: None,
                filter_utm_source: None,
                filter_utm_medium: None,
                filter_utm_campaign: None,
            },
            None,
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

/// `GET /api/share/:share_id/metrics`
#[tracing::instrument(skip(state))]
pub async fn share_metrics(
    State(state): State<Arc<AppState>>,
    Path(share_id): Path<String>,
    Query(q): Query<ShareMetricsQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let website_id = resolve_share(&state, &share_id, &headers).await?;

    let metric_type = match q.metric_type.as_deref() {
        Some(t) => t,
        None => {
            return Err(AppError::BadRequest(
                "type parameter is required".to_string(),
            ))
        }
    };

    if !sparklytics_core::analytics::VALID_METRIC_TYPES.contains(&metric_type) {
        return Err(AppError::BadRequest(format!(
            "invalid metric type: {metric_type}"
        )));
    }

    let (start_date, end_date) = parse_dates(q.start_date.as_deref(), q.end_date.as_deref());
    let limit = q.limit.unwrap_or(10).clamp(1, 100);
    let offset = q.offset.unwrap_or(0).max(0);

    let page = state
        .analytics
        .get_metrics(
            &website_id,
            None,
            metric_type,
            limit,
            offset,
            &AnalyticsFilter {
                start_date,
                end_date,
                timezone: None,
                filter_country: None,
                filter_page: None,
                filter_referrer: None,
                filter_browser: None,
                filter_os: None,
                filter_device: None,
                filter_language: None,
                filter_utm_source: None,
                filter_utm_medium: None,
                filter_utm_campaign: None,
            },
        )
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "type": metric_type,
            "rows": page.rows,
        },
        "pagination": {
            "total": page.total,
            "limit": limit,
            "offset": offset,
            "has_more": offset + limit < page.total,
        },
    })))
}

// ---------------------------------------------------------------------------
// Self-hosted share management (behind require_auth)
// ---------------------------------------------------------------------------

/// `POST /api/websites/:id/share` — enable public sharing (self-hosted).
///
/// Generates a UUID v4 share_id. Idempotent: if sharing is already enabled,
/// returns the existing share_id with 201.
#[tracing::instrument(skip(state))]
pub async fn enable_sharing(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    // Check if sharing is already enabled.
    let existing = state
        .db
        .get_share_id(&website_id)
        .await
        .map_err(AppError::Internal)?;

    let share_id = match existing {
        Some(id) => id,
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            state
                .db
                .set_share_id(&website_id, &new_id)
                .await
                .map_err(AppError::Internal)?;
            new_id
        }
    };

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

/// `DELETE /api/websites/:id/share` — disable public sharing (self-hosted).
#[tracing::instrument(skip(state))]
pub async fn disable_sharing(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let existing = state
        .db
        .get_share_id(&website_id)
        .await
        .map_err(AppError::Internal)?;

    if existing.is_none() {
        return Err(AppError::NotFound(
            "Sharing is not enabled for this website".to_string(),
        ));
    }

    state
        .db
        .clear_share_id(&website_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}
