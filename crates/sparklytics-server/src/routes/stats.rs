use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::AnalyticsFilter;

use crate::{
    error::AppError,
    routes::compare::{metadata_json, resolve_compare_range},
    routes::query::parse_defaulted_date_range_lenient,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
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

/// `GET /api/websites/:id/stats` - Summary statistics.
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<StatsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    if let Some(ref country) = query.filter_country {
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(AppError::BadRequest(
                "filter_country must be ISO 3166-1 alpha-2 (2 chars)".to_string(),
            ));
        }
    }

    let (start_date, end_date) = parse_defaulted_date_range_lenient(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        6,
    )?;
    let include_bots = query
        .include_bots
        .unwrap_or(state.default_include_bots(&website_id).await);

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

    let result = state
        .analytics
        .get_stats(&website_id, None, &filter, compare.as_ref())
        .await
        .map_err(AppError::Internal)?;

    if let Some(compare_range) = compare {
        let mut data = serde_json::to_value(&result).map_err(|e| AppError::Internal(e.into()))?;
        if let Some(object) = data.as_object_mut() {
            object.remove("compare");
        }
        return Ok(Json(json!({
            "data": data,
            "compare": metadata_json(Some(&compare_range))
        })));
    }

    Ok(Json(json!({ "data": result })))
}
