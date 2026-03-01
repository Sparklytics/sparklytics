use std::{sync::Arc, time::Duration};

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{AnalyticsFilter, AnchorType, JourneyDirection, JourneyQuery};

use crate::{
    error::AppError,
    routes::query::{
        normalize_optional_filter, normalize_timezone_non_empty, parse_required_date_range,
    },
    state::AppState,
};

const DEFAULT_DEPTH: u32 = 3;

#[derive(Debug, Deserialize)]
pub struct JourneyQueryParams {
    pub anchor_type: Option<String>,
    pub anchor_value: Option<String>,
    pub direction: Option<String>,
    pub max_depth: Option<u32>,

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
}

fn parse_anchor_type(raw: Option<&str>) -> Result<AnchorType, AppError> {
    match raw.map(str::trim) {
        Some("page") => Ok(AnchorType::Page),
        Some("event") => Ok(AnchorType::Event),
        Some(_) => Err(AppError::BadRequest(
            "anchor_type must be either 'page' or 'event'".to_string(),
        )),
        None => Err(AppError::BadRequest("anchor_type is required".to_string())),
    }
}

fn parse_direction(raw: Option<&str>) -> Result<JourneyDirection, AppError> {
    match raw.map(str::trim) {
        Some("next") => Ok(JourneyDirection::Next),
        Some("previous") => Ok(JourneyDirection::Previous),
        Some(_) => Err(AppError::BadRequest(
            "direction must be either 'next' or 'previous'".to_string(),
        )),
        None => Err(AppError::BadRequest("direction is required".to_string())),
    }
}

pub async fn get_journey(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<JourneyQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let anchor_type = parse_anchor_type(query.anchor_type.as_deref())?;
    let anchor_value = query
        .anchor_value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("anchor_value is required".to_string()))?
        .to_string();
    if anchor_value.len() > 500 {
        return Err(AppError::BadRequest(
            "anchor_value must be at most 500 characters".to_string(),
        ));
    }

    let direction = parse_direction(query.direction.as_deref())?;

    let max_depth = query.max_depth.unwrap_or(DEFAULT_DEPTH);
    if !(1..=5).contains(&max_depth) {
        return Err(AppError::BadRequest(
            "max_depth must be between 1 and 5".to_string(),
        ));
    }

    let (start_date, end_date) =
        parse_required_date_range(query.start_date.as_deref(), query.end_date.as_deref())?;
    let include_bots = query
        .include_bots
        .unwrap_or(state.default_include_bots(&website_id).await);

    let filter = AnalyticsFilter {
        start_date,
        end_date,
        timezone: normalize_timezone_non_empty(query.timezone.as_deref())?,
        filter_country: normalize_optional_filter("filter_country", query.filter_country, 64)?,
        filter_page: normalize_optional_filter("filter_page", query.filter_page, 512)?,
        filter_referrer: normalize_optional_filter("filter_referrer", query.filter_referrer, 512)?,
        filter_browser: normalize_optional_filter("filter_browser", query.filter_browser, 128)?,
        filter_os: normalize_optional_filter("filter_os", query.filter_os, 128)?,
        filter_device: normalize_optional_filter("filter_device", query.filter_device, 64)?,
        filter_language: normalize_optional_filter("filter_language", query.filter_language, 64)?,
        filter_utm_source: normalize_optional_filter(
            "filter_utm_source",
            query.filter_utm_source,
            256,
        )?,
        filter_utm_medium: normalize_optional_filter(
            "filter_utm_medium",
            query.filter_utm_medium,
            256,
        )?,
        filter_utm_campaign: normalize_optional_filter(
            "filter_utm_campaign",
            query.filter_utm_campaign,
            256,
        )?,
        filter_region: normalize_optional_filter("filter_region", query.filter_region, 128)?,
        filter_city: normalize_optional_filter("filter_city", query.filter_city, 128)?,
        filter_hostname: normalize_optional_filter("filter_hostname", query.filter_hostname, 255)?,
        include_bots,
    };

    let journey_query = JourneyQuery {
        anchor_type,
        anchor_value,
        direction,
        max_depth,
    };

    let _permit = tokio::time::timeout(Duration::from_secs(5), state.journey_semaphore.acquire())
        .await
        .map_err(|_| AppError::RateLimited)?
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let data = state
        .analytics
        .get_journey(&website_id, None, &filter, &journey_query)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("invalid_anchor_value") {
                AppError::BadRequest("anchor_value is required".to_string())
            } else if msg.contains("invalid_timezone")
                || msg.contains("invalid_timezone_transition")
                || msg.contains("invalid_date_boundary")
            {
                AppError::BadRequest("invalid timezone".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(json!({ "data": data })))
}
