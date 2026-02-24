use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};

use sparklytics_core::analytics::{
    AnalyticsFilter, CreateFunnelRequest, CreateFunnelStepRequest, UpdateFunnelRequest,
};

use crate::{error::AppError, state::AppState};

const DEFAULT_RESULTS_RANGE_DAYS: i64 = 30;
const MAX_RESULTS_RANGE_DAYS: i64 = 90;
const FUNNELS_READ_RATE_LIMIT: usize = 60;
const FUNNELS_MUTATION_RATE_LIMIT: usize = 30;
const FUNNELS_RESULTS_RATE_LIMIT: usize = 10;
const FUNNELS_SCOPE_READ_RATE_LIMIT: usize = 240;
const FUNNELS_SCOPE_MUTATION_RATE_LIMIT: usize = 60;
const FUNNELS_SCOPE_RESULTS_RATE_LIMIT: usize = 20;

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

fn parse_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let today = chrono::Utc::now().date_naive();
    let start = match start_date {
        Some(raw) => NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("invalid start_date (expected YYYY-MM-DD)".to_string())
        })?,
        None => today - chrono::Duration::days(DEFAULT_RESULTS_RANGE_DAYS - 1),
    };
    let end = match end_date {
        Some(raw) => NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("invalid end_date (expected YYYY-MM-DD)".to_string())
        })?,
        None => today,
    };
    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }
    let range_days = (end - start).num_days() + 1;
    if range_days > MAX_RESULTS_RANGE_DAYS {
        return Err(AppError::BadRequest(format!(
            "date range too large: {range_days} days (max {MAX_RESULTS_RANGE_DAYS})"
        )));
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

fn forwarded_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(parse_ip)
}

fn parse_ip(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return None;
    }
    trimmed.parse::<IpAddr>().ok().map(|ip| ip.to_string())
}

fn fingerprint_key(prefix: &str, value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{prefix}:{:016x}", hasher.finish())
}

fn client_bucket_key(headers: &HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_ip)
        .or_else(|| forwarded_ip(headers))
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .filter(|s| !s.trim().is_empty() && s.len() <= 4096)
                .map(|s| fingerprint_key("auth", s))
        })
        .or_else(|| {
            headers
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .filter(|s| !s.trim().is_empty() && s.len() <= 1024)
                .map(|s| fingerprint_key("ua", s))
        })
        .unwrap_or_else(|| "anonymous".to_string())
}

async fn enforce_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    scope_key: Option<&str>,
    max_per_min: usize,
    scope_max_per_min: usize,
) -> Result<(), AppError> {
    if state.config.rate_limit_disable {
        return Ok(());
    }
    let bucket = client_bucket_key(headers);
    if !state
        .check_rate_limit_with_max(&format!("bucket:{bucket}"), max_per_min)
        .await
    {
        return Err(AppError::RateLimited);
    }
    if let Some(scope_key) = scope_key {
        if !state
            .check_rate_limit_with_max(&format!("scope:{scope_key}"), scope_max_per_min)
            .await
        {
            return Err(AppError::RateLimited);
        }
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), (StatusCode, Json<Value>)> {
    if name.trim().is_empty() {
        return Err(unprocessable(
            "validation_error",
            "name must not be empty",
            Some("name"),
        ));
    }
    if name.len() > 100 {
        return Err(unprocessable(
            "validation_error",
            "name must be 100 characters or fewer",
            Some("name"),
        ));
    }
    Ok(())
}

fn validate_steps(steps: &[CreateFunnelStepRequest]) -> Result<(), (StatusCode, Json<Value>)> {
    if !(2..=8).contains(&steps.len()) {
        return Err(unprocessable(
            "validation_error",
            "funnels must have between 2 and 8 steps",
            Some("steps"),
        ));
    }

    for step in steps {
        if step.match_value.trim().is_empty() {
            return Err(unprocessable(
                "validation_error",
                "match_value must not be empty",
                Some("match_value"),
            ));
        }
        if step.match_value.len() > 500 {
            return Err(unprocessable(
                "validation_error",
                "match_value must be 500 characters or fewer",
                Some("match_value"),
            ));
        }
        if let Some(label) = &step.label {
            if label.trim().is_empty() {
                return Err(unprocessable(
                    "validation_error",
                    "label must not be empty",
                    Some("label"),
                ));
            }
            if label.len() > 120 {
                return Err(unprocessable(
                    "validation_error",
                    "label must be 120 characters or fewer",
                    Some("label"),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct FunnelResultsQuery {
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
}

pub async fn list_funnels(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnels:read")),
        FUNNELS_READ_RATE_LIMIT,
        FUNNELS_SCOPE_READ_RATE_LIMIT,
    )
    .await?;

    let data = state
        .analytics
        .list_funnels(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": data })))
}

pub async fn get_funnel(
    State(state): State<Arc<AppState>>,
    Path((website_id, funnel_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnel:{funnel_id}:read")),
        FUNNELS_READ_RATE_LIMIT,
        FUNNELS_SCOPE_READ_RATE_LIMIT,
    )
    .await?;

    let data = state
        .analytics
        .get_funnel(&website_id, None, &funnel_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Funnel not found".to_string()))?;
    Ok(Json(json!({ "data": data })))
}

pub async fn create_funnel(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<CreateFunnelRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnels:mutation")),
        FUNNELS_MUTATION_RATE_LIMIT,
        FUNNELS_SCOPE_MUTATION_RATE_LIMIT,
    )
    .await?;

    if let Err(resp) = validate_name(&req.name) {
        return Ok(resp);
    }
    if let Err(resp) = validate_steps(&req.steps) {
        return Ok(resp);
    }

    let data = match state.analytics.create_funnel(&website_id, None, req).await {
        Ok(data) => data,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("limit_exceeded") {
                return Ok(unprocessable(
                    "limit_exceeded",
                    "maximum of 20 funnels per website reached",
                    Some("funnels"),
                ));
            }
            if msg.contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "funnel name already exists for this website",
                    Some("name"),
                ));
            }
            if msg.contains("validation_error") {
                return Ok(unprocessable(
                    "validation_error",
                    "invalid funnel payload",
                    None,
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::CREATED, Json(json!({ "data": data }))))
}

pub async fn update_funnel(
    State(state): State<Arc<AppState>>,
    Path((website_id, funnel_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<UpdateFunnelRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnel:{funnel_id}:mutation")),
        FUNNELS_MUTATION_RATE_LIMIT,
        FUNNELS_SCOPE_MUTATION_RATE_LIMIT,
    )
    .await?;

    if req.name.is_none() && req.steps.is_none() {
        return Err(AppError::BadRequest(
            "request must include at least one updatable field".to_string(),
        ));
    }
    if let Some(name) = &req.name {
        if let Err(resp) = validate_name(name) {
            return Ok(resp);
        }
    }
    if let Some(steps) = &req.steps {
        if let Err(resp) = validate_steps(steps) {
            return Ok(resp);
        }
    }

    let data = match state
        .analytics
        .update_funnel(&website_id, None, &funnel_id, req)
        .await
    {
        Ok(Some(data)) => data,
        Ok(None) => return Err(AppError::NotFound("Funnel not found".to_string())),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "funnel name already exists for this website",
                    Some("name"),
                ));
            }
            if msg.contains("validation_error") {
                return Ok(unprocessable(
                    "validation_error",
                    "invalid funnel payload",
                    None,
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::OK, Json(json!({ "data": data }))))
}

pub async fn delete_funnel(
    State(state): State<Arc<AppState>>,
    Path((website_id, funnel_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnel:{funnel_id}:mutation")),
        FUNNELS_MUTATION_RATE_LIMIT,
        FUNNELS_SCOPE_MUTATION_RATE_LIMIT,
    )
    .await?;

    let deleted = state
        .analytics
        .delete_funnel(&website_id, None, &funnel_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("Funnel not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_funnel_results(
    State(state): State<Arc<AppState>>,
    Path((website_id, funnel_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(query): Query<FunnelResultsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    enforce_rate_limit(
        &state,
        &headers,
        Some(&format!("website:{website_id}:funnel:{funnel_id}:results")),
        FUNNELS_RESULTS_RATE_LIMIT,
        FUNNELS_SCOPE_RESULTS_RATE_LIMIT,
    )
    .await?;

    let (start_date, end_date) =
        parse_date_range(query.start_date.as_deref(), query.end_date.as_deref())?;
    let filter_country = normalize_optional_filter("filter_country", query.filter_country, 64)?;
    let filter_page = normalize_optional_filter("filter_page", query.filter_page, 512)?;
    let filter_referrer = normalize_optional_filter("filter_referrer", query.filter_referrer, 512)?;
    let filter_browser = normalize_optional_filter("filter_browser", query.filter_browser, 128)?;
    let filter_os = normalize_optional_filter("filter_os", query.filter_os, 128)?;
    let filter_device = normalize_optional_filter("filter_device", query.filter_device, 64)?;
    let filter_language = normalize_optional_filter("filter_language", query.filter_language, 64)?;
    let filter_utm_source =
        normalize_optional_filter("filter_utm_source", query.filter_utm_source, 256)?;
    let filter_utm_medium =
        normalize_optional_filter("filter_utm_medium", query.filter_utm_medium, 256)?;
    let filter_utm_campaign =
        normalize_optional_filter("filter_utm_campaign", query.filter_utm_campaign, 256)?;
    let filter_region = normalize_optional_filter("filter_region", query.filter_region, 128)?;
    let filter_city = normalize_optional_filter("filter_city", query.filter_city, 128)?;
    let filter_hostname = normalize_optional_filter("filter_hostname", query.filter_hostname, 255)?;

    let filter = AnalyticsFilter {
        start_date,
        end_date,
        timezone: normalize_timezone(query.timezone.as_deref())?,
        filter_country,
        filter_page,
        filter_referrer,
        filter_browser,
        filter_os,
        filter_device,
        filter_language,
        filter_utm_source,
        filter_utm_medium,
        filter_utm_campaign,
        filter_region,
        filter_city,
        filter_hostname,
    };

    let _permit = tokio::time::timeout(
        Duration::from_secs(1),
        state.funnel_results_semaphore.acquire(),
    )
    .await
    .map_err(|_| AppError::RateLimited)?
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let data = state
        .analytics
        .get_funnel_results(&website_id, None, &funnel_id, &filter)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("Funnel not found") {
                AppError::NotFound("Funnel not found".to_string())
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
