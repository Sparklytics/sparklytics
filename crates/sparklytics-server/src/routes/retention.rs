use std::{sync::Arc, time::Duration};

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{AnalyticsFilter, RetentionGranularity, RetentionQuery};

use crate::{error::AppError, state::AppState};

const RETENTION_QUEUE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const RETENTION_QUERY_TIMEOUT_RETRY_AFTER_SECONDS: u64 = 2;

#[derive(Debug, Deserialize)]
pub struct RetentionParams {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub timezone: Option<String>,
    pub cohort_granularity: Option<String>,
    pub max_periods: Option<u32>,
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
}

fn parse_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let Some(start_raw) = start_date else {
        return Err(AppError::BadRequest("start_date is required".to_string()));
    };
    let Some(end_raw) = end_date else {
        return Err(AppError::BadRequest("end_date is required".to_string()));
    };

    let start = NaiveDate::parse_from_str(start_raw.trim(), "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest("invalid start_date (expected YYYY-MM-DD)".to_string())
    })?;
    let end = NaiveDate::parse_from_str(end_raw.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("invalid end_date (expected YYYY-MM-DD)".to_string()))?;

    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }

    Ok((start, end))
}

fn normalize_timezone(timezone: Option<&str>) -> Result<Option<String>, AppError> {
    match timezone {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest(
                    "timezone cannot be empty when provided".to_string(),
                ));
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn normalize_optional_filter(
    field: &str,
    value: Option<String>,
    max_len: usize,
) -> Result<Option<String>, AppError> {
    if let Some(raw) = value {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "{field} cannot be empty when provided"
            )));
        }
        if trimmed.len() > max_len {
            return Err(AppError::BadRequest(format!(
                "{field} is too long (max {max_len} characters)"
            )));
        }
        return Ok(Some(trimmed));
    }
    Ok(None)
}

fn parse_granularity(raw: Option<&str>) -> Result<RetentionGranularity, AppError> {
    match raw.map(str::trim) {
        None => Ok(RetentionGranularity::Week),
        Some("day") => Ok(RetentionGranularity::Day),
        Some("week") => Ok(RetentionGranularity::Week),
        Some("month") => Ok(RetentionGranularity::Month),
        Some(_) => Err(AppError::BadRequest(
            "cohort_granularity must be one of: day, week, month".to_string(),
        )),
    }
}

fn default_periods(granularity: &RetentionGranularity) -> u32 {
    match granularity {
        RetentionGranularity::Day => 30,
        RetentionGranularity::Week => 8,
        RetentionGranularity::Month => 12,
    }
}

fn validate_max_periods(
    granularity: &RetentionGranularity,
    max_periods: u32,
) -> Result<(), AppError> {
    let (min, max, label) = match granularity {
        RetentionGranularity::Day => (1, 30, "daily"),
        RetentionGranularity::Week => (1, 12, "weekly"),
        RetentionGranularity::Month => (1, 12, "monthly"),
    };

    if (min..=max).contains(&max_periods) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "max_periods must be between {min} and {max} for {label} granularity"
        )))
    }
}

fn map_retention_backend_error(error: anyhow::Error) -> AppError {
    let msg = error.to_string();
    if msg.contains("retention_query_timeout") {
        AppError::QueryTimeout {
            retry_after_seconds: RETENTION_QUERY_TIMEOUT_RETRY_AFTER_SECONDS,
        }
    } else if msg.contains("invalid_timezone")
        || msg.contains("invalid_timezone_transition")
        || msg.contains("invalid_date_boundary")
    {
        AppError::BadRequest("invalid timezone".to_string())
    } else {
        AppError::Internal(error)
    }
}

pub async fn get_retention(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(params): Query<RetentionParams>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let granularity = parse_granularity(params.cohort_granularity.as_deref())?;
    let max_periods = params
        .max_periods
        .unwrap_or_else(|| default_periods(&granularity));
    validate_max_periods(&granularity, max_periods)?;

    let (start_date, end_date) =
        parse_date_range(params.start_date.as_deref(), params.end_date.as_deref())?;

    let filter = AnalyticsFilter {
        start_date,
        end_date,
        timezone: normalize_timezone(params.timezone.as_deref())?,
        filter_country: normalize_optional_filter("filter_country", params.filter_country, 64)?,
        filter_page: normalize_optional_filter("filter_page", params.filter_page, 512)?,
        filter_referrer: normalize_optional_filter("filter_referrer", params.filter_referrer, 512)?,
        filter_browser: normalize_optional_filter("filter_browser", params.filter_browser, 128)?,
        filter_os: normalize_optional_filter("filter_os", params.filter_os, 128)?,
        filter_device: normalize_optional_filter("filter_device", params.filter_device, 64)?,
        filter_language: normalize_optional_filter("filter_language", params.filter_language, 64)?,
        filter_utm_source: normalize_optional_filter(
            "filter_utm_source",
            params.filter_utm_source,
            256,
        )?,
        filter_utm_medium: normalize_optional_filter(
            "filter_utm_medium",
            params.filter_utm_medium,
            256,
        )?,
        filter_utm_campaign: normalize_optional_filter(
            "filter_utm_campaign",
            params.filter_utm_campaign,
            256,
        )?,
        filter_region: normalize_optional_filter("filter_region", params.filter_region, 128)?,
        filter_city: normalize_optional_filter("filter_city", params.filter_city, 128)?,
        filter_hostname: normalize_optional_filter("filter_hostname", params.filter_hostname, 255)?,
    };

    let retention_query = RetentionQuery {
        granularity,
        max_periods,
    };

    let _permit = tokio::time::timeout(
        RETENTION_QUEUE_WAIT_TIMEOUT,
        state.retention_semaphore.acquire(),
    )
    .await
    .map_err(|_| AppError::RateLimited)?
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let data = state
        .analytics
        .get_retention(&website_id, None, &filter, &retention_query)
        .await
        .map_err(map_retention_backend_error)?;

    Ok(Json(json!({ "data": data })))
}

#[cfg(test)]
mod tests {
    use super::map_retention_backend_error;
    use crate::error::AppError;

    #[test]
    fn timeout_marker_maps_to_query_timeout() {
        let error = anyhow::anyhow!("retention_query_timeout");
        let app_error = map_retention_backend_error(error);
        match app_error {
            AppError::QueryTimeout {
                retry_after_seconds,
            } => assert_eq!(retry_after_seconds, 2),
            other => panic!("unexpected error mapping: {other:?}"),
        }
    }
}
