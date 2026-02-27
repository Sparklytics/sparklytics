use std::sync::Arc;
use std::{net::IpAddr, str::FromStr};

use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{
    BotMatchType, BotPolicyMode, BotRecomputeStatus, CreateBotListEntryRequest,
    UpdateBotPolicyRequest,
};

use crate::bot_detection::{classify_event, BotOverrideDecision, BotPolicyInput};
use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct BotSummaryQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BotListQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct BotReportQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub granularity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BotRecomputeRequest {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BotAuditQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

fn parse_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
    default_days: i64,
    max_days: i64,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let today = Utc::now().date_naive();
    let start = start_date
        .and_then(|raw| NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - Duration::days(default_days.saturating_sub(1)));
    let end = end_date
        .and_then(|raw| NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok())
        .unwrap_or(today);

    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }
    let range_days = (end - start).num_days() + 1;
    if range_days > max_days {
        return Err(AppError::BadRequest(format!(
            "date range too large: {range_days} days (max {max_days})"
        )));
    }

    let start_dt = start
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::BadRequest("invalid start_date".to_string()))?;
    let end_dt = (end + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::BadRequest("invalid end_date".to_string()))?;
    Ok((
        DateTime::<Utc>::from_naive_utc_and_offset(start_dt, Utc),
        DateTime::<Utc>::from_naive_utc_and_offset(end_dt, Utc),
    ))
}

fn actor_label() -> &'static str {
    "selfhosted"
}

fn validate_policy(req: &UpdateBotPolicyRequest) -> Result<(), AppError> {
    if !(0..=100).contains(&req.threshold_score) {
        return Err(AppError::BadRequest(
            "threshold_score must be between 0 and 100".to_string(),
        ));
    }
    if matches!(req.mode, BotPolicyMode::Off) {
        return Ok(());
    }
    Ok(())
}

fn validate_list_entry(req: &CreateBotListEntryRequest) -> Result<(), AppError> {
    let value = req.match_value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(
            "match_value cannot be empty".to_string(),
        ));
    }
    if value.len() > 255 {
        return Err(AppError::BadRequest(
            "match_value must be 255 characters or fewer".to_string(),
        ));
    }
    if let Some(note) = req.note.as_ref() {
        if note.len() > 255 {
            return Err(AppError::BadRequest(
                "note must be 255 characters or fewer".to_string(),
            ));
        }
    }
    Ok(())
}

fn override_match_rule(
    match_type: &BotMatchType,
    match_value: &str,
    parsed_ip: Option<IpAddr>,
    user_agent_lc: &str,
) -> bool {
    let value = match_value.trim();
    match match_type {
        BotMatchType::UaContains => user_agent_lc.contains(&value.to_ascii_lowercase()),
        BotMatchType::IpExact => parsed_ip
            .map(|ip| ip.to_string().eq_ignore_ascii_case(value))
            .unwrap_or(false),
        BotMatchType::IpCidr => match (parsed_ip, ipnet::IpNet::from_str(value).ok()) {
            (Some(ip), Some(net)) => net.contains(&ip),
            _ => false,
        },
    }
}

fn classify_override_from_rules(
    source_ip: &str,
    user_agent: &str,
    block_rules: &[(BotMatchType, String)],
    allow_rules: &[(BotMatchType, String)],
) -> Option<BotOverrideDecision> {
    let parsed_ip = source_ip.parse::<IpAddr>().ok();
    let ua = user_agent.to_ascii_lowercase();

    for (match_type, match_value) in block_rules {
        if override_match_rule(match_type, match_value, parsed_ip, &ua) {
            return Some(BotOverrideDecision::ForceBot);
        }
    }
    for (match_type, match_value) in allow_rules {
        if override_match_rule(match_type, match_value, parsed_ip, &ua) {
            return Some(BotOverrideDecision::ForceHuman);
        }
    }
    None
}

async fn run_recompute_job(state: Arc<AppState>, website_id: String, run_id: String) {
    let result = async {
        state
            .db
            .update_bot_recompute_status(&run_id, BotRecomputeStatus::Running, None)
            .await?;
        let Some((start_date, end_date)) = state
            .db
            .parse_bot_recompute_window(&website_id, &run_id)
            .await?
        else {
            return Err(anyhow!("recompute job not found"));
        };
        let policy = state.db.get_bot_policy(&website_id).await?;
        let policy_input = BotPolicyInput {
            mode: policy.mode,
            threshold_score: policy.threshold_score,
        };
        let (block_rules, allow_rules) = state.db.list_bot_override_rules(&website_id).await?;

        let mut cursor: Option<(String, String)> = None;
        loop {
            let (events, next_cursor) = state
                .db
                .list_events_for_recompute(&website_id, start_date, end_date, cursor.clone(), 500)
                .await?;
            if events.is_empty() {
                break;
            }
            for (event_id, visitor_id, url, source_ip, user_agent) in events {
                let source_ip = source_ip.unwrap_or_else(|| "unknown".to_string());
                let user_agent = user_agent.unwrap_or_default();
                let override_decision = classify_override_from_rules(
                    &source_ip,
                    &user_agent,
                    &block_rules,
                    &allow_rules,
                );

                let classification = classify_event(
                    &website_id,
                    &visitor_id,
                    &url,
                    &user_agent,
                    true,
                    true,
                    &policy_input,
                    override_decision,
                );
                state
                    .db
                    .update_event_bot_classification(
                        &event_id,
                        classification.is_bot,
                        classification.bot_score,
                        classification.bot_reason.as_deref(),
                    )
                    .await?;
            }
            cursor = next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        state
            .db
            .recompute_sessions_bot_rollup_in_window(&website_id, start_date, end_date)
            .await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(err) = result {
        let _ = state
            .db
            .update_bot_recompute_status(
                &run_id,
                BotRecomputeStatus::Failed,
                Some(&err.to_string()),
            )
            .await;
        tracing::error!(error = %err, website_id = %website_id, run_id = %run_id, "Bot recompute failed");
        return;
    }

    let _ = state
        .db
        .update_bot_recompute_status(&run_id, BotRecomputeStatus::Success, None)
        .await;
}

pub async fn get_bot_summary(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<BotSummaryQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (start_date, end_date) = parse_date_range(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        7,
        90,
    )?;
    let summary = state
        .db
        .get_bot_summary(&website_id, start_date, end_date)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": summary })))
}

pub async fn get_bot_policy(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let policy = state
        .db
        .get_bot_policy(&website_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": policy })))
}

pub async fn put_bot_policy(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<UpdateBotPolicyRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_policy(&req)?;

    let policy = state
        .db
        .upsert_bot_policy(&website_id, &req)
        .await
        .map_err(AppError::Internal)?;
    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "policy_update",
            &json!({
                "mode": req.mode,
                "threshold_score": req.threshold_score
            }),
        )
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_bot_policy_cache(&website_id).await;

    Ok(Json(json!({ "data": policy })))
}

pub async fn list_bot_allowlist(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<BotListQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (rows, next_cursor) = state
        .db
        .list_bot_entries(
            &website_id,
            "allow",
            query.cursor.clone(),
            query.limit.unwrap_or(50),
        )
        .await
        .map_err(|err| {
            if err.to_string().contains("invalid_cursor") {
                AppError::BadRequest("invalid cursor".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;
    Ok(Json(json!({ "data": rows, "next_cursor": next_cursor })))
}

pub async fn create_bot_allowlist(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateBotListEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_list_entry(&req)?;
    let entry = state
        .db
        .create_bot_entry(&website_id, "allow", &req)
        .await
        .map_err(|err| {
            if err.to_string().contains("UNIQUE") {
                AppError::BadRequest("entry already exists".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;
    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "allow_add",
            &json!({
                "id": entry.id,
                "match_type": entry.match_type,
                "match_value": entry.match_value
            }),
        )
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_bot_override_cache(&website_id).await;
    Ok((StatusCode::CREATED, Json(json!({ "data": entry }))))
}

pub async fn delete_bot_allowlist(
    State(state): State<Arc<AppState>>,
    Path((website_id, entry_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .db
        .delete_bot_entry(&website_id, "allow", &entry_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("allowlist entry not found".to_string()));
    }
    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "allow_remove",
            &json!({ "id": entry_id }),
        )
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_bot_override_cache(&website_id).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_bot_blocklist(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<BotListQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (rows, next_cursor) = state
        .db
        .list_bot_entries(
            &website_id,
            "block",
            query.cursor.clone(),
            query.limit.unwrap_or(50),
        )
        .await
        .map_err(|err| {
            if err.to_string().contains("invalid_cursor") {
                AppError::BadRequest("invalid cursor".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;
    Ok(Json(json!({ "data": rows, "next_cursor": next_cursor })))
}

pub async fn create_bot_blocklist(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateBotListEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_list_entry(&req)?;
    let entry = state
        .db
        .create_bot_entry(&website_id, "block", &req)
        .await
        .map_err(|err| {
            if err.to_string().contains("UNIQUE") {
                AppError::BadRequest("entry already exists".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;
    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "block_add",
            &json!({
                "id": entry.id,
                "match_type": entry.match_type,
                "match_value": entry.match_value
            }),
        )
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_bot_override_cache(&website_id).await;
    Ok((StatusCode::CREATED, Json(json!({ "data": entry }))))
}

pub async fn delete_bot_blocklist(
    State(state): State<Arc<AppState>>,
    Path((website_id, entry_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .db
        .delete_bot_entry(&website_id, "block", &entry_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("blocklist entry not found".to_string()));
    }
    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "block_remove",
            &json!({ "id": entry_id }),
        )
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_bot_override_cache(&website_id).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_bot_report(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<BotReportQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (start_date, end_date) = parse_date_range(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        7,
        90,
    )?;
    let granularity = match query.granularity.as_deref() {
        Some("hour") => "hour",
        _ => "day",
    };
    let report = state
        .db
        .get_bot_report(&website_id, start_date, end_date, granularity)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": report })))
}

pub async fn post_bot_recompute(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<BotRecomputeRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (start_date, end_date) =
        parse_date_range(req.start_date.as_deref(), req.end_date.as_deref(), 7, 30)?;

    if state
        .db
        .has_active_bot_recompute(&website_id)
        .await
        .map_err(AppError::Internal)?
    {
        return Ok((
            StatusCode::CONFLICT,
            Json(
                json!({ "error": { "code": "conflict", "message": "recompute already running" } }),
            ),
        ));
    }

    let run = state
        .db
        .create_bot_recompute_run(&website_id, start_date, end_date)
        .await
        .map_err(AppError::Internal)?;

    state
        .db
        .add_bot_policy_audit(
            &website_id,
            actor_label(),
            "recompute_start",
            &json!({
                "job_id": run.job_id,
                "start_date": run.start_date,
                "end_date": run.end_date
            }),
        )
        .await
        .map_err(AppError::Internal)?;

    let state_clone = Arc::clone(&state);
    let website_id_clone = website_id.clone();
    let run_id = run.job_id.clone();
    tokio::spawn(async move {
        run_recompute_job(state_clone, website_id_clone, run_id).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "job_id": run.job_id, "status": "queued" })),
    ))
}

pub async fn get_bot_recompute(
    State(state): State<Arc<AppState>>,
    Path((website_id, job_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let Some(run) = state
        .db
        .get_bot_recompute_run(&website_id, &job_id)
        .await
        .map_err(AppError::Internal)?
    else {
        return Err(AppError::NotFound("recompute job not found".to_string()));
    };
    Ok(Json(json!({ "data": run })))
}

pub async fn list_bot_audit(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<BotAuditQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let (rows, next_cursor) = state
        .db
        .list_bot_policy_audit(&website_id, query.cursor.clone(), query.limit.unwrap_or(50))
        .await
        .map_err(|err| {
            if err.to_string().contains("invalid_cursor") {
                AppError::BadRequest("invalid cursor".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;
    Ok(Json(json!({ "data": rows, "next_cursor": next_cursor })))
}
