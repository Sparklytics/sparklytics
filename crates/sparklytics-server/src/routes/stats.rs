use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_duckdb::queries::stats::StatsParams;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
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

/// `GET /api/websites/:id/stats` — Summary statistics.
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<StatsQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate website exists.
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    // Validate filter_country if provided.
    if let Some(ref country) = query.filter_country {
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(AppError::BadRequest(
                "filter_country must be ISO 3166-1 alpha-2 (2 chars)".to_string(),
            ));
        }
    }

    let today = chrono::Utc::now().date_naive();
    let start_date = query
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(6));
    let end_date = query
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);

    let params = StatsParams {
        website_id,
        start_date,
        end_date,
        filter_country: query.filter_country,
        filter_page: query.filter_page,
        filter_referrer: query.filter_referrer,
        filter_browser: query.filter_browser,
        filter_os: query.filter_os,
        filter_device: query.filter_device,
        filter_utm_source: query.filter_utm_source,
        filter_utm_medium: query.filter_utm_medium,
        filter_utm_campaign: query.filter_utm_campaign,
    };

    let result = state
        .db
        .get_stats(&params)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}
