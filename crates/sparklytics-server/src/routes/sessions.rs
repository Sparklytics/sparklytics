use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{
    AnalyticsFilter, SessionSort, SessionsQuery as BackendSessionsQuery,
};

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct SessionsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
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

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<SessionsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(AppError::BadRequest(
            "limit must be between 1 and 200".to_string(),
        ));
    }

    let (start_date, end_date) =
        parse_date_range(query.start_date.as_deref(), query.end_date.as_deref())?;
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

    let backend_query = BackendSessionsQuery {
        limit,
        cursor: query.cursor,
        sort: SessionSort::LastSeenDesc,
    };

    let result = state
        .analytics
        .get_sessions(&website_id, None, &filter, &backend_query)
        .await
        .map_err(|e| {
            if e.to_string().contains("invalid_cursor") {
                AppError::BadRequest("invalid cursor".to_string())
            } else if e.to_string().contains("invalid limit") {
                AppError::BadRequest("limit must be between 1 and 200".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(json!({
        "data": result.rows,
        "pagination": result.pagination,
    })))
}

pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path((website_id, session_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let result = state
        .analytics
        .get_session_detail(&website_id, None, &session_id)
        .await
        .map_err(|e| {
            if e.to_string().contains("Session not found") {
                AppError::NotFound("Session not found".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(json!({
        "data": {
            "session": result.session,
            "events": result.events,
            "truncated": result.truncated,
        }
    })))
}
