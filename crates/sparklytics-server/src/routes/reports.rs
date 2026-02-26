use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde_json::{json, Value};

use sparklytics_core::analytics::{
    AnalyticsFilter, CreateReportRequest, DateRangeType, ReportConfig, ReportRunResult, ReportType,
    UpdateReportRequest, VALID_METRIC_TYPES,
};

use crate::{
    error::AppError,
    routes::compare::{metadata_json, resolve_compare_range},
    state::AppState,
};

const MAX_REPORTS_PER_WEBSITE: i64 = 100;

fn unprocessable(code: &str, message: &str, field: Option<&str>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": {
                "code": code,
                "message": message,
                "field": field
            }
        })),
    )
}

fn validate_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if name.len() > 100 {
        return Err(AppError::BadRequest(
            "name must be 100 characters or fewer".to_string(),
        ));
    }
    Ok(())
}

fn normalize_timezone(timezone: Option<&str>) -> Result<Option<String>, AppError> {
    match timezone {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<chrono_tz::Tz>()
                    .map(|_| Some(trimmed.to_string()))
                    .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))
            }
        }
    }
}

fn today_for_timezone(timezone: Option<&str>) -> Result<NaiveDate, AppError> {
    let now = chrono::Utc::now();
    let today = match timezone {
        None => now.date_naive(),
        Some(raw) => {
            let tz = raw
                .parse::<chrono_tz::Tz>()
                .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))?;
            now.with_timezone(&tz).date_naive()
        }
    };
    Ok(today)
}

fn parse_relative_range(
    relative_days: Option<u32>,
    timezone: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let days = relative_days.unwrap_or(30);
    if !(1..=365).contains(&days) {
        return Err(AppError::BadRequest(
            "relative_days must be between 1 and 365".to_string(),
        ));
    }
    let today = today_for_timezone(timezone)?;
    let start = today - chrono::Duration::days((days - 1) as i64);
    Ok((start, today))
}

fn parse_absolute_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let Some(start_raw) = start_date else {
        return Err(AppError::BadRequest(
            "start_date is required for absolute range".to_string(),
        ));
    };
    let Some(end_raw) = end_date else {
        return Err(AppError::BadRequest(
            "end_date is required for absolute range".to_string(),
        ));
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

fn build_analytics_context(
    config: &ReportConfig,
) -> Result<
    (
        AnalyticsFilter,
        Option<sparklytics_core::analytics::ComparisonRange>,
    ),
    AppError,
> {
    let timezone = normalize_timezone(config.timezone.as_deref())?;

    let (start_date, end_date) = match config.date_range_type {
        DateRangeType::Relative => parse_relative_range(config.relative_days, timezone.as_deref())?,
        DateRangeType::Absolute => {
            parse_absolute_range(config.start_date.as_deref(), config.end_date.as_deref())?
        }
    };

    if matches!(config.report_type, ReportType::Metrics) {
        let Some(metric_type) = config.metric_type.as_deref() else {
            return Err(AppError::BadRequest(
                "metric_type is required for metrics reports".to_string(),
            ));
        };
        if !VALID_METRIC_TYPES.contains(&metric_type) {
            return Err(AppError::BadRequest(format!(
                "invalid metric type: {metric_type}"
            )));
        }
    }

    let compare_mode = config
        .compare_mode
        .clone()
        .unwrap_or(sparklytics_core::analytics::CompareMode::None);
    let comparison = resolve_compare_range(
        start_date,
        end_date,
        Some(match compare_mode {
            sparklytics_core::analytics::CompareMode::None => "none",
            sparklytics_core::analytics::CompareMode::PreviousPeriod => "previous_period",
            sparklytics_core::analytics::CompareMode::PreviousYear => "previous_year",
            sparklytics_core::analytics::CompareMode::Custom => "custom",
        }),
        config.compare_start_date.as_deref(),
        config.compare_end_date.as_deref(),
    )?;

    Ok((
        AnalyticsFilter {
            start_date,
            end_date,
            timezone,
            filter_country: config.filter_country.clone(),
            filter_page: config.filter_page.clone(),
            filter_referrer: config.filter_referrer.clone(),
            filter_browser: config.filter_browser.clone(),
            filter_os: config.filter_os.clone(),
            filter_device: config.filter_device.clone(),
            filter_language: None,
            filter_utm_source: config.filter_utm_source.clone(),
            filter_utm_medium: config.filter_utm_medium.clone(),
            filter_utm_campaign: config.filter_utm_campaign.clone(),
            filter_region: config.filter_region.clone(),
            filter_city: config.filter_city.clone(),
            filter_hostname: config.filter_hostname.clone(),
            include_bots: false,
        },
        comparison,
    ))
}

pub(crate) async fn execute_report_config_with_backend(
    analytics: &dyn sparklytics_core::analytics::AnalyticsBackend,
    website_id: &str,
    config: &ReportConfig,
) -> Result<Value, AppError> {
    let (filter, comparison) = build_analytics_context(config)?;
    match config.report_type {
        ReportType::Stats => {
            let data = analytics
                .get_stats(website_id, None, &filter, comparison.as_ref())
                .await
                .map_err(AppError::Internal)?;
            if comparison.is_some() {
                serde_json::to_value(json!({
                    "data": data,
                    "compare": metadata_json(comparison.as_ref()),
                }))
                .map_err(|e| AppError::Internal(e.into()))
            } else {
                serde_json::to_value(data).map_err(|e| AppError::Internal(e.into()))
            }
        }
        ReportType::Pageviews => {
            let data = analytics
                .get_timeseries(website_id, None, &filter, None, comparison.as_ref())
                .await
                .map_err(AppError::Internal)?;
            if comparison.is_some() {
                serde_json::to_value(json!({
                    "data": {
                        "series": data.series,
                        "granularity": data.granularity,
                        "compare_series": data.compare_series,
                    },
                    "compare": metadata_json(comparison.as_ref()),
                }))
                .map_err(|e| AppError::Internal(e.into()))
            } else {
                serde_json::to_value(data).map_err(|e| AppError::Internal(e.into()))
            }
        }
        ReportType::Metrics => {
            let metric_type = config.metric_type.as_deref().ok_or_else(|| {
                AppError::BadRequest("metric_type is required for metrics reports".to_string())
            })?;
            let page = analytics
                .get_metrics(
                    website_id,
                    None,
                    metric_type,
                    25,
                    0,
                    &filter,
                    comparison.as_ref(),
                )
                .await
                .map_err(AppError::Internal)?;
            serde_json::to_value(json!({
                "type": metric_type,
                "rows": page.rows,
                "total": page.total,
                "compare": metadata_json(comparison.as_ref()),
            }))
            .map_err(|e| AppError::Internal(e.into()))
        }
        ReportType::Events => {
            let data = analytics
                .get_event_names(website_id, None, &filter)
                .await
                .map_err(AppError::Internal)?;
            serde_json::to_value(data).map_err(|e| AppError::Internal(e.into()))
        }
    }
}

pub(crate) async fn execute_report_config(
    state: &AppState,
    website_id: &str,
    config: &ReportConfig,
) -> Result<Value, AppError> {
    execute_report_config_with_backend(state.analytics.as_ref(), website_id, config).await
}

pub async fn list_reports(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let reports = state
        .analytics
        .list_reports(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": reports })))
}

pub async fn get_report(
    State(state): State<Arc<AppState>>,
    Path((website_id, report_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let report = state
        .analytics
        .get_report(&website_id, None, &report_id)
        .await
        .map_err(AppError::Internal)?;
    match report {
        Some(report) => Ok(Json(json!({ "data": report }))),
        None => Err(AppError::NotFound("Report not found".to_string())),
    }
}

pub async fn create_report(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateReportRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_name(&req.name)?;
    // Validate config semantics before persisting.
    build_analytics_context(&req.config)?;

    let count = state
        .analytics
        .count_reports(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    if count >= MAX_REPORTS_PER_WEBSITE {
        return Ok(unprocessable(
            "limit_exceeded",
            "maximum of 100 reports per website reached",
            Some("reports"),
        ));
    }

    let exists = state
        .analytics
        .report_name_exists(&website_id, None, &req.name, None)
        .await
        .map_err(AppError::Internal)?;
    if exists {
        return Ok(unprocessable(
            "duplicate_name",
            "report name already exists for this website",
            Some("name"),
        ));
    }

    let report = match state.analytics.create_report(&website_id, None, req).await {
        Ok(report) => report,
        Err(e) => {
            if e.to_string().contains("limit_exceeded") {
                return Ok(unprocessable(
                    "limit_exceeded",
                    "maximum of 100 reports per website reached",
                    Some("reports"),
                ));
            }
            if e.to_string().contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "report name already exists for this website",
                    Some("name"),
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::CREATED, Json(json!({ "data": report }))))
}

pub async fn update_report(
    State(state): State<Arc<AppState>>,
    Path((website_id, report_id)): Path<(String, String)>,
    Json(req): Json<UpdateReportRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    if let Some(ref name) = req.name {
        validate_name(name)?;
        let exists = state
            .analytics
            .report_name_exists(&website_id, None, name, Some(&report_id))
            .await
            .map_err(AppError::Internal)?;
        if exists {
            return Ok(unprocessable(
                "duplicate_name",
                "report name already exists for this website",
                Some("name"),
            ));
        }
    }
    if let Some(ref config) = req.config {
        build_analytics_context(config)?;
    }

    let report = match state
        .analytics
        .update_report(&website_id, None, &report_id, req)
        .await
    {
        Ok(Some(report)) => report,
        Ok(None) => return Err(AppError::NotFound("Report not found".to_string())),
        Err(e) => {
            if e.to_string().contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "report name already exists for this website",
                    Some("name"),
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::OK, Json(json!({ "data": report }))))
}

pub async fn delete_report(
    State(state): State<Arc<AppState>>,
    Path((website_id, report_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    state
        .analytics
        .delete_report(&website_id, None, &report_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn preview_report(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(config): Json<ReportConfig>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let data = execute_report_config(&state, &website_id, &config).await?;
    let result = ReportRunResult {
        report_id: None,
        config,
        ran_at: chrono::Utc::now().to_rfc3339(),
        data,
    };
    Ok(Json(json!({ "data": result })))
}

pub async fn run_report(
    State(state): State<Arc<AppState>>,
    Path((website_id, report_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let Some(report) = state
        .analytics
        .get_report(&website_id, None, &report_id)
        .await
        .map_err(AppError::Internal)?
    else {
        return Err(AppError::NotFound("Report not found".to_string()));
    };

    let data = execute_report_config(&state, &website_id, &report.config).await?;
    state
        .analytics
        .touch_report_last_run(&website_id, None, &report_id)
        .await
        .map_err(AppError::Internal)?;

    let result = ReportRunResult {
        report_id: Some(report_id),
        config: report.config,
        ran_at: chrono::Utc::now().to_rfc3339(),
        data,
    };
    Ok(Json(json!({ "data": result })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ReportConfig {
        ReportConfig {
            version: 1,
            report_type: ReportType::Stats,
            date_range_type: DateRangeType::Relative,
            relative_days: Some(30),
            start_date: None,
            end_date: None,
            compare_mode: None,
            compare_start_date: None,
            compare_end_date: None,
            timezone: Some("UTC".to_string()),
            metric_type: None,
            filter_country: None,
            filter_browser: None,
            filter_os: None,
            filter_device: None,
            filter_page: None,
            filter_referrer: None,
            filter_utm_source: None,
            filter_utm_medium: None,
            filter_utm_campaign: None,
            filter_region: None,
            filter_city: None,
            filter_hostname: None,
        }
    }

    #[test]
    fn relative_range_rejects_out_of_bounds() {
        assert!(parse_relative_range(Some(0), None).is_err());
        assert!(parse_relative_range(Some(366), None).is_err());
    }

    #[test]
    fn absolute_range_rejects_inverted_dates() {
        let err = parse_absolute_range(Some("2026-02-10"), Some("2026-02-01"));
        assert!(err.is_err());
    }

    #[test]
    fn absolute_range_accepts_valid_dates() {
        let (start, end) = parse_absolute_range(Some("2026-02-01"), Some("2026-02-10"))
            .expect("valid absolute range");
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 2, 1).expect("date"));
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 2, 10).expect("date"));
    }

    #[test]
    fn relative_range_resolves_expected_window() {
        let today = chrono::Utc::now().date_naive();
        let (start, end) = parse_relative_range(Some(2), None).expect("valid relative range");
        assert_eq!(end, today);
        assert_eq!(start, today - chrono::Duration::days(1));
    }

    #[test]
    fn relative_range_rejects_invalid_timezone() {
        let err = parse_relative_range(Some(7), Some("Mars/Base"));
        assert!(err.is_err());
    }

    #[test]
    fn metrics_requires_metric_type() {
        let mut cfg = default_config();
        cfg.report_type = ReportType::Metrics;
        cfg.metric_type = None;
        let err = build_analytics_context(&cfg).expect_err("missing metric type should fail");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn metrics_rejects_unknown_metric_type() {
        let mut cfg = default_config();
        cfg.report_type = ReportType::Metrics;
        cfg.metric_type = Some("bogus".to_string());
        let err = build_analytics_context(&cfg).expect_err("unknown metric type should fail");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_name_rejects_empty_and_too_long() {
        assert!(validate_name("").is_err());
        let long_name = "x".repeat(101);
        assert!(validate_name(&long_name).is_err());
    }
}
