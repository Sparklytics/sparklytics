use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use crate::{error::AppError, state::AppState};

/// Maximum date range allowed for export (90 days).
const MAX_EXPORT_DAYS: i64 = 90;

/// Maximum number of rows allowed in a single export (500 000).
const MAX_EXPORT_ROWS: usize = 500_000;

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub start_date: String,
    pub end_date: String,
    pub format: Option<String>,
}

/// `GET /api/websites/:id/export` — download events as CSV.
///
/// Date range validation: > 90 days → 400.
/// Row cap: > 500 000 rows → 400.
/// Response: `Content-Type: text/csv` with `Content-Disposition: attachment`.
#[tracing::instrument(skip(state))]
pub async fn export_events(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(q): Query<ExportQuery>,
) -> Result<Response, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    // Validate and parse dates.
    let start = NaiveDate::parse_from_str(&q.start_date, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest("invalid start_date format, expected YYYY-MM-DD".to_string())
    })?;
    let end = NaiveDate::parse_from_str(&q.end_date, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest("invalid end_date format, expected YYYY-MM-DD".to_string())
    })?;

    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }

    let range_days = (end - start).num_days() + 1;
    if range_days > MAX_EXPORT_DAYS {
        return Err(AppError::BadRequest(format!(
            "date range too large: {range_days} days (max {MAX_EXPORT_DAYS})"
        )));
    }

    // Validate format (only csv supported right now).
    if let Some(ref fmt) = q.format {
        if fmt != "csv" {
            return Err(AppError::BadRequest(format!(
                "unsupported format: {fmt}; only 'csv' is supported"
            )));
        }
    }

    let cache_key = state.export_cache_key(&website_id, &q.start_date, &q.end_date);
    let filename = format!("events-{}-{}-{}.csv", website_id, q.start_date, q.end_date);

    if let Some(csv_bytes) = state.get_cached_export_csv(&cache_key).await {
        return build_csv_response(&filename, csv_bytes);
    }

    let _export_cache_guard = state.lock_export_cache_compute().await;
    if let Some(csv_bytes) = state.get_cached_export_csv(&cache_key).await {
        return build_csv_response(&filename, csv_bytes);
    }

    let _permit = state
        .export_semaphore
        .acquire()
        .await
        .map_err(|_| AppError::Internal(anyhow::anyhow!("export semaphore closed")))?;

    let rows = state
        .analytics
        .export_events(&website_id, None, start, end)
        .await
        .map_err(AppError::Internal)?;

    // Guard: row count > 500K → 400
    if rows.len() > MAX_EXPORT_ROWS {
        return Err(AppError::BadRequest(format!(
            "result set too large: > {MAX_EXPORT_ROWS} rows; narrow the date range"
        )));
    }

    // Serialise rows to CSV in memory.
    let csv_bytes =
        Bytes::from(build_csv(&rows).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?);
    state
        .put_cached_export_csv(cache_key, csv_bytes.clone())
        .await;

    build_csv_response(&filename, csv_bytes)
}

/// Sanitize a CSV field value against formula injection.
///
/// Spreadsheet apps (Excel, Google Sheets, LibreOffice) interpret values that
/// begin with `=`, `+`, `-`, `@`, TAB, or CR as formula expressions. Prepending
/// a single quote (`'`) causes them to treat the value as a literal string.
fn sanitize_csv_field(val: &str) -> std::borrow::Cow<'_, str> {
    if val.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        std::borrow::Cow::Owned(format!("'{val}"))
    } else {
        std::borrow::Cow::Borrowed(val)
    }
}

fn build_csv(rows: &[sparklytics_core::analytics::ExportRow]) -> anyhow::Result<Vec<u8>> {
    let mut wtr = csv::Writer::from_writer(Vec::with_capacity(rows.len().saturating_mul(256)));

    // Write CSV headers.
    wtr.write_record([
        "id",
        "website_id",
        "event_type",
        "url",
        "referrer_domain",
        "event_name",
        "country",
        "browser",
        "os",
        "device_type",
        "language",
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "created_at",
    ])
    .map_err(|e| anyhow::anyhow!("csv write_record failed: {e}"))?;

    for row in rows {
        let id = sanitize_csv_field(&row.id);
        let website_id = sanitize_csv_field(&row.website_id);
        let event_type = sanitize_csv_field(&row.event_type);
        let url = sanitize_csv_field(&row.url);
        let referrer_domain = sanitize_csv_field(row.referrer_domain.as_deref().unwrap_or(""));
        let event_name = sanitize_csv_field(row.event_name.as_deref().unwrap_or(""));
        let country = sanitize_csv_field(row.country.as_deref().unwrap_or(""));
        let browser = sanitize_csv_field(row.browser.as_deref().unwrap_or(""));
        let os = sanitize_csv_field(row.os.as_deref().unwrap_or(""));
        let device_type = sanitize_csv_field(row.device_type.as_deref().unwrap_or(""));
        let language = sanitize_csv_field(row.language.as_deref().unwrap_or(""));
        let utm_source = sanitize_csv_field(row.utm_source.as_deref().unwrap_or(""));
        let utm_medium = sanitize_csv_field(row.utm_medium.as_deref().unwrap_or(""));
        let utm_campaign = sanitize_csv_field(row.utm_campaign.as_deref().unwrap_or(""));
        let created_at = sanitize_csv_field(&row.created_at);

        wtr.write_record([
            id.as_ref(),
            website_id.as_ref(),
            event_type.as_ref(),
            url.as_ref(),
            referrer_domain.as_ref(),
            event_name.as_ref(),
            country.as_ref(),
            browser.as_ref(),
            os.as_ref(),
            device_type.as_ref(),
            language.as_ref(),
            utm_source.as_ref(),
            utm_medium.as_ref(),
            utm_campaign.as_ref(),
            created_at.as_ref(),
        ])
        .map_err(|e| anyhow::anyhow!("csv write_record failed: {e}"))?;
    }

    wtr.into_inner()
        .map_err(|e| anyhow::anyhow!("csv flush failed: {e}"))
}

fn build_csv_response(filename: &str, csv_bytes: Bytes) -> Result<Response, AppError> {
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

/// `GET /api/usage` — not available in self-hosted mode (returns 404).
pub async fn usage_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "not_found",
                "message": "Usage tracking unavailable in self-hosted mode",
                "field": null
            }
        })),
    )
}
