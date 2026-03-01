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
    routes::query::{
        normalize_timezone_non_empty, parse_defaulted_date_range_lenient, parse_optional_bool,
        validate_date_span,
    },
    state::AppState,
};

/// Maximum date range allowed for events analytics endpoints.
const MAX_EVENTS_QUERY_DAYS: i64 = 90;

#[derive(Debug, Deserialize)]
pub struct EventFilterQuery {
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
    pub include_bots: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    #[serde(flatten)]
    pub filter: EventFilterQuery,
}

#[derive(Debug, Deserialize)]
pub struct EventPropertiesQuery {
    pub event_name: Option<String>,
    #[serde(flatten)]
    pub filter: EventFilterQuery,
}

#[derive(Debug, Deserialize)]
pub struct EventTimeseriesQuery {
    pub event_name: Option<String>,
    pub granularity: Option<String>,
    #[serde(flatten)]
    pub filter: EventFilterQuery,
}

fn build_filter(
    query: &EventFilterQuery,
    default_include_bots: bool,
) -> Result<AnalyticsFilter, AppError> {
    let (start_date, end_date) = parse_defaulted_date_range_lenient(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        6,
    )?;
    validate_date_span(start_date, end_date, MAX_EVENTS_QUERY_DAYS, "date range")?;
    let include_bots = parse_optional_bool(query.include_bots.as_deref(), "include_bots")?
        .unwrap_or(default_include_bots);
    Ok(AnalyticsFilter {
        start_date,
        end_date,
        timezone: normalize_timezone_non_empty(query.timezone.as_deref())?,
        filter_country: query.filter_country.clone(),
        filter_page: query.filter_page.clone(),
        filter_referrer: query.filter_referrer.clone(),
        filter_browser: query.filter_browser.clone(),
        filter_os: query.filter_os.clone(),
        filter_device: query.filter_device.clone(),
        filter_language: query.filter_language.clone(),
        filter_utm_source: query.filter_utm_source.clone(),
        filter_utm_medium: query.filter_utm_medium.clone(),
        filter_utm_campaign: query.filter_utm_campaign.clone(),
        filter_region: query.filter_region.clone(),
        filter_city: query.filter_city.clone(),
        filter_hostname: query.filter_hostname.clone(),
        include_bots,
    })
}

fn require_event_name(event_name: Option<String>) -> Result<String, AppError> {
    let Some(event_name) = event_name else {
        return Err(AppError::BadRequest(
            "event_name query parameter is required".to_string(),
        ));
    };
    let trimmed = event_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(
            "event_name must not be empty".to_string(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(AppError::BadRequest(
            "event_name must be 255 characters or fewer".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// `GET /api/websites/:id/events` - List custom event names.
#[tracing::instrument(skip(state))]
pub async fn get_event_names(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let default_include_bots = state.default_include_bots(&website_id).await;
    let filter = build_filter(&query.filter, default_include_bots)?;

    let result = state
        .analytics
        .get_event_names(&website_id, None, &filter)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}

/// `GET /api/websites/:id/events/properties` - Property key/value breakdown for one event.
#[tracing::instrument(skip(state))]
pub async fn get_event_properties(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<EventPropertiesQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let event_name = require_event_name(query.event_name)?;

    let default_include_bots = state.default_include_bots(&website_id).await;
    let filter = build_filter(&query.filter, default_include_bots)?;

    let result = state
        .analytics
        .get_event_properties(&website_id, None, &event_name, &filter)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}

/// `GET /api/websites/:id/events/timeseries` - Time series for one custom event.
#[tracing::instrument(skip(state))]
pub async fn get_event_timeseries(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<EventTimeseriesQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let event_name = require_event_name(query.event_name)?;

    let default_include_bots = state.default_include_bots(&website_id).await;
    let filter = build_filter(&query.filter, default_include_bots)?;

    let result = state
        .analytics
        .get_event_timeseries(
            &website_id,
            None,
            &event_name,
            &filter,
            query.granularity.as_deref(),
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

#[cfg(test)]
mod tests {
    use crate::routes::query::parse_optional_bool;

    #[test]
    fn parse_optional_bool_accepts_common_variants() {
        assert_eq!(
            parse_optional_bool(Some("true"), "include_bots").unwrap(),
            Some(true)
        );
        assert_eq!(
            parse_optional_bool(Some("false"), "include_bots").unwrap(),
            Some(false)
        );
        assert_eq!(
            parse_optional_bool(Some("1"), "include_bots").unwrap(),
            Some(true)
        );
        assert_eq!(
            parse_optional_bool(Some("0"), "include_bots").unwrap(),
            Some(false)
        );
        assert_eq!(parse_optional_bool(None, "include_bots").unwrap(), None);
    }

    #[test]
    fn parse_optional_bool_rejects_invalid_values() {
        assert!(parse_optional_bool(Some("yes"), "include_bots").is_err());
        assert!(parse_optional_bool(Some(""), "include_bots").is_err());
    }
}
