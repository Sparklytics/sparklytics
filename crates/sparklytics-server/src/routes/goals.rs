use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};

use sparklytics_core::analytics::{AnalyticsFilter, CreateGoalRequest, UpdateGoalRequest};

use crate::{error::AppError, state::AppState};

const MAX_GOALS_PER_WEBSITE: i64 = 50;

#[derive(Debug, Deserialize)]
pub struct GoalStatsQuery {
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
    let start = start_date
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(6));
    let end = end_date
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }
    Ok((start, end))
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

fn validate_match_value(match_value: &str) -> Result<(), (StatusCode, Json<Value>)> {
    if match_value.trim().is_empty() {
        return Err(unprocessable(
            "validation_error",
            "match_value must not be empty",
            Some("match_value"),
        ));
    }
    if match_value.len() > 500 {
        return Err(unprocessable(
            "validation_error",
            "match_value must be 500 characters or fewer",
            Some("match_value"),
        ));
    }
    Ok(())
}

pub async fn list_goals(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let goals = state
        .analytics
        .list_goals(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": goals })))
}

pub async fn create_goal(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateGoalRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    if let Err(resp) = validate_name(&req.name) {
        return Ok(resp);
    }
    if let Err(resp) = validate_match_value(&req.match_value) {
        return Ok(resp);
    }

    let count = state
        .analytics
        .count_goals(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    if count >= MAX_GOALS_PER_WEBSITE {
        return Ok(unprocessable(
            "limit_exceeded",
            "maximum of 50 goals per website reached",
            Some("goals"),
        ));
    }

    let exists = state
        .analytics
        .goal_name_exists(&website_id, None, &req.name, None)
        .await
        .map_err(AppError::Internal)?;
    if exists {
        return Ok(unprocessable(
            "duplicate_name",
            "goal name already exists for this website",
            Some("name"),
        ));
    }

    let goal = match state.analytics.create_goal(&website_id, None, req).await {
        Ok(goal) => goal,
        Err(e) => {
            if e.to_string().contains("limit_exceeded") {
                return Ok(unprocessable(
                    "limit_exceeded",
                    "maximum of 50 goals per website reached",
                    Some("goals"),
                ));
            }
            if e.to_string().contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "goal name already exists for this website",
                    Some("name"),
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::CREATED, Json(json!({ "data": goal }))))
}

pub async fn update_goal(
    State(state): State<Arc<AppState>>,
    Path((website_id, goal_id)): Path<(String, String)>,
    Json(req): Json<UpdateGoalRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    if let Some(ref name) = req.name {
        if let Err(resp) = validate_name(name) {
            return Ok(resp);
        }
        let exists = state
            .analytics
            .goal_name_exists(&website_id, None, name, Some(&goal_id))
            .await
            .map_err(AppError::Internal)?;
        if exists {
            return Ok(unprocessable(
                "duplicate_name",
                "goal name already exists for this website",
                Some("name"),
            ));
        }
    }
    if let Some(ref match_value) = req.match_value {
        if let Err(resp) = validate_match_value(match_value) {
            return Ok(resp);
        }
    }

    let goal = match state
        .analytics
        .update_goal(&website_id, None, &goal_id, req)
        .await
    {
        Ok(goal) => goal,
        Err(e) => {
            if e.to_string().contains("Goal not found") {
                return Err(AppError::NotFound("Goal not found".to_string()));
            }
            if e.to_string().contains("duplicate_name") {
                return Ok(unprocessable(
                    "duplicate_name",
                    "goal name already exists for this website",
                    Some("name"),
                ));
            }
            return Err(AppError::Internal(e));
        }
    };

    Ok((StatusCode::OK, Json(json!({ "data": goal }))))
}

pub async fn delete_goal(
    State(state): State<Arc<AppState>>,
    Path((website_id, goal_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    state
        .analytics
        .delete_goal(&website_id, None, &goal_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_goal_stats(
    State(state): State<Arc<AppState>>,
    Path((website_id, goal_id)): Path<(String, String)>,
    Query(query): Query<GoalStatsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let (start_date, end_date) =
        parse_date_range(query.start_date.as_deref(), query.end_date.as_deref())?;
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
    };

    let stats = state
        .analytics
        .get_goal_stats(&website_id, None, &goal_id, &filter)
        .await
        .map_err(|e| {
            if e.to_string().contains("Goal not found") {
                AppError::NotFound("Goal not found".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(json!({ "data": stats })))
}
