use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sparklytics_core::analytics::{
    CreateAlertRuleRequest, CreateReportSubscriptionRequest, NotificationChannel,
    NotificationSourceType, UpdateAlertRuleRequest, UpdateReportSubscriptionRequest,
};

use crate::{
    error::AppError,
    routes::reports::execute_report_config,
    scheduler::delivery::deliver_and_record,
    state::AppState,
};

fn validate_timezone(timezone: &str) -> Result<(), AppError> {
    timezone
        .trim()
        .parse::<chrono_tz::Tz>()
        .map(|_| ())
        .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))
}

fn validate_target(channel: &NotificationChannel, target: &str) -> Result<(), AppError> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("target is required".to_string()));
    }
    match channel {
        NotificationChannel::Email => {
            let Some((local, domain)) = trimmed.split_once('@') else {
                return Err(AppError::BadRequest("invalid email target".to_string()));
            };
            if local.is_empty() || !domain.contains('.') || domain.starts_with('.') {
                return Err(AppError::BadRequest("invalid email target".to_string()));
            }
        }
        NotificationChannel::Webhook => {
            let url = url::Url::parse(trimmed)
                .map_err(|_| AppError::BadRequest("invalid webhook target".to_string()))?;
            if url.scheme() != "http" && url.scheme() != "https" {
                return Err(AppError::BadRequest(
                    "webhook target must use http or https".to_string(),
                ));
            }
            let host = url
                .host_str()
                .ok_or_else(|| AppError::BadRequest("webhook target missing host".to_string()))?;
            if host.eq_ignore_ascii_case("localhost") {
                return Err(AppError::BadRequest(
                    "webhook target host is not allowed".to_string(),
                ));
            }
            if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                if is_blocked_ip(ip) {
                    return Err(AppError::BadRequest(
                        "webhook target host is not allowed".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn is_blocked_v4(v4: std::net::Ipv4Addr) -> bool {
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_multicast()
        || v4.is_broadcast()
        || v4.is_unspecified()
        || v4.octets()[0] == 0
}

fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => is_blocked_v4(v4),
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6.to_ipv4().map(is_blocked_v4).unwrap_or(false)
        }
    }
}

fn unique_test_idempotency_key(prefix: &str, source_id: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut rng = rand::thread_rng();
    let nonce = rng.next_u64();
    format!("{prefix}:{source_id}:{nanos:x}:{nonce:x}")
}

#[derive(Debug, Deserialize)]
pub struct NotificationHistoryQuery {
    pub limit: Option<i64>,
}

pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let data = state
        .db
        .list_report_subscriptions(&website_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": data })))
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateReportSubscriptionRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_target(&req.channel, &req.target)?;
    if let Some(ref tz) = req.timezone {
        validate_timezone(tz)?;
    }
    if state
        .analytics
        .get_report(&website_id, None, &req.report_id)
        .await
        .map_err(AppError::Internal)?
        .is_none()
    {
        return Err(AppError::NotFound("Report not found".to_string()));
    }
    let data = state
        .db
        .create_report_subscription(&website_id, req)
        .await
        .map_err(AppError::Internal)?;
    Ok((StatusCode::CREATED, Json(json!({ "data": data }))))
}

pub async fn update_subscription(
    State(state): State<Arc<AppState>>,
    Path((website_id, subscription_id)): Path<(String, String)>,
    Json(req): Json<UpdateReportSubscriptionRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    if let Some(Some(ref tz)) = req.timezone {
        validate_timezone(tz)?;
    }
    let channel_for_target = req.channel.clone();
    if let Some(Some(ref target)) = req.target {
        validate_target(
            channel_for_target
                .as_ref()
                .unwrap_or(&NotificationChannel::Email),
            target,
        )?;
    }
    if let Some(ref report_id) = req.report_id {
        if state
            .analytics
            .get_report(&website_id, None, report_id)
            .await
            .map_err(AppError::Internal)?
            .is_none()
        {
            return Err(AppError::NotFound("Report not found".to_string()));
        }
    }
    let data = state
        .db
        .update_report_subscription(&website_id, &subscription_id, req)
        .await
        .map_err(AppError::Internal)?;
    match data {
        Some(data) => Ok((StatusCode::OK, Json(json!({ "data": data }))).into_response()),
        None => Err(AppError::NotFound("Subscription not found".to_string())),
    }
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path((website_id, subscription_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .db
        .delete_report_subscription(&website_id, &subscription_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("Subscription not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_subscription(
    State(state): State<Arc<AppState>>,
    Path((website_id, subscription_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let subscription = state
        .db
        .get_report_subscription(&website_id, &subscription_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;
    let report = state
        .analytics
        .get_report(&website_id, None, &subscription.report_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Report not found".to_string()))?;
    let report_data = execute_report_config(state.as_ref(), &website_id, &report.config).await?;
    let idempotency_key = unique_test_idempotency_key("sub-test", &subscription_id);
    let payload = json!({
        "kind": "report_subscription_test",
        "website_id": website_id,
        "subscription_id": subscription.id,
        "report_id": subscription.report_id,
        "generated_at": Utc::now().to_rfc3339(),
        "data": report_data
    });
    let delivery = deliver_and_record(
        &state,
        NotificationSourceType::Subscription,
        &subscription.id,
        &idempotency_key,
        subscription.channel,
        subscription.target,
        payload,
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": delivery })))
}

pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let data = state
        .db
        .list_alert_rules(&website_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": data })))
}

pub async fn create_alert(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateAlertRuleRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_target(&req.channel, &req.target)?;
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    if req.threshold_value.is_nan() || !req.threshold_value.is_finite() {
        return Err(AppError::BadRequest(
            "threshold_value must be finite".to_string(),
        ));
    }
    let data = state
        .db
        .create_alert_rule(&website_id, req)
        .await
        .map_err(AppError::Internal)?;
    Ok((StatusCode::CREATED, Json(json!({ "data": data }))))
}

pub async fn update_alert(
    State(state): State<Arc<AppState>>,
    Path((website_id, alert_id)): Path<(String, String)>,
    Json(req): Json<UpdateAlertRuleRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let channel_for_target = req.channel.clone();
    if let Some(Some(ref target)) = req.target {
        validate_target(
            channel_for_target
                .as_ref()
                .unwrap_or(&NotificationChannel::Email),
            target,
        )?;
    }
    if let Some(name) = req.name.as_deref() {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name is required".to_string()));
        }
    }
    if let Some(threshold_value) = req.threshold_value {
        if threshold_value.is_nan() || !threshold_value.is_finite() {
            return Err(AppError::BadRequest(
                "threshold_value must be finite".to_string(),
            ));
        }
    }
    let data = state
        .db
        .update_alert_rule(&website_id, &alert_id, req)
        .await
        .map_err(AppError::Internal)?;
    match data {
        Some(data) => Ok((StatusCode::OK, Json(json!({ "data": data }))).into_response()),
        None => Err(AppError::NotFound("Alert not found".to_string())),
    }
}

pub async fn delete_alert(
    State(state): State<Arc<AppState>>,
    Path((website_id, alert_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .db
        .delete_alert_rule(&website_id, &alert_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("Alert not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_alert(
    State(state): State<Arc<AppState>>,
    Path((website_id, alert_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let alert = state
        .db
        .get_alert_rule(&website_id, &alert_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Alert not found".to_string()))?;
    let idempotency_key = unique_test_idempotency_key("alert-test", &alert_id);
    let payload = json!({
        "kind": "alert_test",
        "website_id": website_id,
        "alert_id": alert.id,
        "name": alert.name,
        "metric": alert.metric,
        "condition_type": alert.condition_type,
        "threshold_value": alert.threshold_value,
        "triggered_at": Utc::now().to_rfc3339(),
    });
    let delivery = deliver_and_record(
        &state,
        NotificationSourceType::Alert,
        &alert.id,
        &idempotency_key,
        alert.channel,
        alert.target,
        payload,
    )
    .await
    .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": delivery })))
}

pub async fn notification_history(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<NotificationHistoryQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let data = state
        .db
        .list_notification_deliveries_for_website(&website_id, limit)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": data })))
}
