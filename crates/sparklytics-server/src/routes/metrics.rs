use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{AnalyticsFilter, VALID_METRIC_TYPES};

use crate::{
    error::AppError,
    routes::compare::{metadata_json, resolve_compare_range},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub timezone: Option<String>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_language: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
    pub filter_region: Option<String>,
    pub filter_city: Option<String>,
    pub filter_hostname: Option<String>,
    pub include_bots: Option<bool>,
    pub compare_mode: Option<String>,
    pub compare_start_date: Option<String>,
    pub compare_end_date: Option<String>,
}

/// `GET /api/websites/:id/metrics` - Breakdown by dimension.
pub async fn get_metrics(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<MetricsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    if !VALID_METRIC_TYPES.contains(&query.metric_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid metric type: {}",
            query.metric_type
        )));
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
    let include_bots = query
        .include_bots
        .unwrap_or(state.default_include_bots(&website_id).await);

    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let filter = AnalyticsFilter {
        start_date,
        end_date,
        timezone: query.timezone,
        filter_country: query.filter_country,
        filter_page: query.filter_page,
        filter_referrer: query.filter_referrer,
        filter_browser: query.filter_browser,
        filter_os: query.filter_os,
        filter_device: query.filter_device,
        filter_language: query.filter_language,
        filter_utm_source: query.filter_utm_source,
        filter_utm_medium: query.filter_utm_medium,
        filter_utm_campaign: query.filter_utm_campaign,
        filter_region: query.filter_region,
        filter_city: query.filter_city,
        filter_hostname: query.filter_hostname,
        include_bots,
    };

    let compare = resolve_compare_range(
        start_date,
        end_date,
        query.compare_mode.as_deref(),
        query.compare_start_date.as_deref(),
        query.compare_end_date.as_deref(),
    )?;

    let page = state
        .analytics
        .get_metrics(
            &website_id,
            None,
            &query.metric_type,
            limit,
            offset,
            &filter,
            compare.as_ref(),
        )
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "type": query.metric_type,
            "rows": page.rows,
        },
        "pagination": {
            "total": page.total,
            "limit": limit,
            "offset": offset,
            "has_more": offset + limit < page.total,
        },
        "compare": metadata_json(compare.as_ref()),
    })))
}
