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
    error::AppError, routes::compare::metadata_json, routes::compare::resolve_compare_range,
    routes::query::parse_defaulted_date_range_lenient, state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct PageviewsQuery {
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

/// `GET /api/websites/:id/pageviews` - Time series data.
pub async fn get_pageviews(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<PageviewsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
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
        .get_timeseries(
            &website_id,
            None,
            &filter,
            query.granularity.as_deref(),
            compare.as_ref(),
        )
        .await
        .map_err(AppError::Internal)?;

    if let Some(compare_range) = compare {
        return Ok(Json(json!({
            "data": {
                "series": result.series,
                "granularity": result.granularity,
                "compare_series": result.compare_series.unwrap_or_default(),
            },
            "compare": metadata_json(Some(&compare_range))
        })));
    }

    Ok(Json(json!({
        "data": {
            "series": result.series,
            "granularity": result.granularity,
            "compare_series": result.compare_series,
        }
    })))
}
