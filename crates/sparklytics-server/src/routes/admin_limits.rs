use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, NaiveDate};
use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::json;

use sparklytics_core::config::AppMode;

use super::bearer_jwt::verify_and_decode_bearer_claims;
use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct UpdatePlanLimitRequest {
    pub peak_events_per_sec: u32,
    pub monthly_event_limit: u64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantOverrideRequest {
    #[serde(default, deserialize_with = "deserialize_tri_state")]
    pub peak_events_per_sec: Option<Option<u32>>,
    #[serde(default, deserialize_with = "deserialize_tri_state")]
    pub monthly_event_limit: Option<Option<u64>>,
    pub clear: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub month: Option<String>,
}

fn platform_admin_ids() -> &'static HashSet<String> {
    static IDS: OnceLock<HashSet<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        std::env::var("SPARKLYTICS_CLOUD_PLATFORM_ADMIN_USER_IDS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    })
}

async fn extract_bearer_subject(headers: &HeaderMap) -> Result<String, AppError> {
    let claims = verify_and_decode_bearer_claims(headers).await?;
    claims
        .get("sub")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or(AppError::Unauthorized)
}

async fn require_platform_admin(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    if state.config.mode != AppMode::Cloud {
        return Err(AppError::NotFound("Not found".to_string()));
    }

    let subject = extract_bearer_subject(headers).await?;
    let admins = platform_admin_ids();
    if admins.is_empty() || !admins.contains(&subject) {
        return Err(AppError::Forbidden);
    }
    Ok(subject)
}

fn validate_month_query(month: Option<&str>) -> Result<(), AppError> {
    let Some(raw) = month else {
        return Ok(());
    };

    if raw.len() == 7 {
        let full = format!("{raw}-01");
        NaiveDate::parse_from_str(&full, "%Y-%m-%d").map_err(|_| {
            AppError::BadRequest("invalid month format, expected YYYY-MM".to_string())
        })?;
        return Ok(());
    }

    let parsed = NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest("invalid month format, expected YYYY-MM or YYYY-MM-DD".to_string())
    })?;
    NaiveDate::from_ymd_opt(parsed.year(), parsed.month(), 1)
        .ok_or_else(|| AppError::BadRequest("invalid month value".to_string()))?;
    Ok(())
}

fn deserialize_tri_state<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(Some(None)),
        Some(raw) => T::deserialize(raw)
            .map(|parsed| Some(Some(parsed)))
            .map_err(D::Error::custom),
    }
}

pub async fn list_plan_limits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let _ = require_platform_admin(&state, &headers).await?;
    let plans = state
        .billing_gate
        .list_plan_limits()
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": plans })))
}

pub async fn update_plan_limit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(plan): Path<String>,
    Json(body): Json<UpdatePlanLimitRequest>,
) -> Result<impl IntoResponse, AppError> {
    let _ = require_platform_admin(&state, &headers).await?;
    if body.peak_events_per_sec == 0 {
        return Err(AppError::BadRequest(
            "peak_events_per_sec must be > 0".to_string(),
        ));
    }
    if body.monthly_event_limit == 0 {
        return Err(AppError::BadRequest(
            "monthly_event_limit must be > 0".to_string(),
        ));
    }

    let updated = state
        .billing_gate
        .upsert_plan_limit(&plan, body.peak_events_per_sec, body.monthly_event_limit)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": updated })))
}

pub async fn get_tenant_limits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let _ = require_platform_admin(&state, &headers).await?;
    let effective = state
        .billing_gate
        .get_tenant_effective_limits(&tenant_id)
        .await
        .map_err(AppError::Internal)?;
    let override_config = state
        .billing_gate
        .get_tenant_override(&tenant_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "effective": effective,
            "override": override_config,
        }
    })))
}

pub async fn update_tenant_limits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
    Json(body): Json<UpdateTenantOverrideRequest>,
) -> Result<impl IntoResponse, AppError> {
    let updated_by = require_platform_admin(&state, &headers).await?;

    if body.clear.unwrap_or(false) {
        state
            .billing_gate
            .clear_tenant_override(&tenant_id, &updated_by)
            .await
            .map_err(AppError::Internal)?;
        let effective = state
            .billing_gate
            .get_tenant_effective_limits(&tenant_id)
            .await
            .map_err(AppError::Internal)?;
        return Ok(Json(json!({
            "data": {
                "effective": effective,
                "override": null
            }
        })));
    }

    if body.peak_events_per_sec.is_none() && body.monthly_event_limit.is_none() {
        return Err(AppError::BadRequest(
            "at least one override field must be provided".to_string(),
        ));
    }

    let existing_override = state
        .billing_gate
        .get_tenant_override(&tenant_id)
        .await
        .map_err(AppError::Internal)?;

    let peak_events_per_sec = match body.peak_events_per_sec {
        Some(value) => value,
        None => existing_override
            .as_ref()
            .and_then(|value| value.peak_events_per_sec),
    };
    let monthly_event_limit = match body.monthly_event_limit {
        Some(value) => value,
        None => existing_override
            .as_ref()
            .and_then(|value| value.monthly_event_limit),
    };

    if peak_events_per_sec.is_some_and(|value| value == 0) {
        return Err(AppError::BadRequest(
            "peak_events_per_sec must be > 0".to_string(),
        ));
    }
    if monthly_event_limit.is_some_and(|value| value == 0) {
        return Err(AppError::BadRequest(
            "monthly_event_limit must be > 0".to_string(),
        ));
    }

    if peak_events_per_sec.is_none() && monthly_event_limit.is_none() {
        state
            .billing_gate
            .clear_tenant_override(&tenant_id, &updated_by)
            .await
            .map_err(AppError::Internal)?;
        let effective = state
            .billing_gate
            .get_tenant_effective_limits(&tenant_id)
            .await
            .map_err(AppError::Internal)?;
        return Ok(Json(json!({
            "data": {
                "effective": effective,
                "override": null
            }
        })));
    }

    let override_config = state
        .billing_gate
        .upsert_tenant_override(
            &tenant_id,
            peak_events_per_sec,
            monthly_event_limit,
            &updated_by,
        )
        .await
        .map_err(AppError::Internal)?;
    let effective = state
        .billing_gate
        .get_tenant_effective_limits(&tenant_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "effective": effective,
            "override": override_config,
        }
    })))
}

pub async fn get_tenant_usage(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
    Query(query): Query<UsageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let _ = require_platform_admin(&state, &headers).await?;
    validate_month_query(query.month.as_deref())?;
    let usage = state
        .billing_gate
        .get_tenant_monthly_usage(&tenant_id, query.month.as_deref())
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": usage })))
}
