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

    let rows = state
        .db
        .export_events(&website_id, start, end)
        .await
        .map_err(AppError::Internal)?;

    // Guard: row count > 500K → 400
    if rows.len() > MAX_EXPORT_ROWS {
        return Err(AppError::BadRequest(format!(
            "result set too large: > {MAX_EXPORT_ROWS} rows; narrow the date range"
        )));
    }

    // Serialise rows to CSV in memory.
    let csv_bytes = build_csv(&rows).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let filename = format!(
        "events-{}-{}-{}.csv",
        website_id,
        q.start_date,
        q.end_date
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(csv_bytes))
        .map_err(|e| AppError::Internal(anyhow::anyhow!("response build failed: {e}")))?;

    Ok(response)
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

fn build_csv(rows: &[sparklytics_duckdb::share::ExportRow]) -> anyhow::Result<Vec<u8>> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

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
        let s = |v: &str| sanitize_csv_field(v).into_owned();
        let sopt = |v: Option<&str>| sanitize_csv_field(v.unwrap_or("")).into_owned();
        wtr.write_record([
            s(&row.id),
            s(&row.website_id),
            s(&row.event_type),
            s(&row.url),
            sopt(row.referrer_domain.as_deref()),
            sopt(row.event_name.as_deref()),
            sopt(row.country.as_deref()),
            sopt(row.browser.as_deref()),
            sopt(row.os.as_deref()),
            sopt(row.device_type.as_deref()),
            sopt(row.language.as_deref()),
            sopt(row.utm_source.as_deref()),
            sopt(row.utm_medium.as_deref()),
            sopt(row.utm_campaign.as_deref()),
            s(&row.created_at),
        ])
        .map_err(|e| anyhow::anyhow!("csv write_record failed: {e}"))?;
    }

    wtr.into_inner().map_err(|e| anyhow::anyhow!("csv flush failed: {e}"))
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
