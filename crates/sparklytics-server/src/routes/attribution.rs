use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_core::analytics::{AnalyticsFilter, AttributionModel, AttributionQuery};

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct AttributionRequestQuery {
    pub goal_id: String,
    pub model: Option<String>,
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

fn parse_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
    timezone: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let today = match timezone {
        None => chrono::Utc::now().date_naive(),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                chrono::Utc::now().date_naive()
            } else {
                let tz = trimmed
                    .parse::<chrono_tz::Tz>()
                    .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))?;
                chrono::Utc::now().with_timezone(&tz).date_naive()
            }
        }
    };
    let parse = |value: Option<&str>, field: &str| -> Result<Option<NaiveDate>, AppError> {
        value
            .map(|raw| {
                NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|_| {
                    AppError::BadRequest(format!("invalid {field} (expected YYYY-MM-DD)"))
                })
            })
            .transpose()
    };
    let start =
        parse(start_date, "start_date")?.unwrap_or_else(|| today - chrono::Duration::days(6));
    let end = parse(end_date, "end_date")?.unwrap_or(today);
    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }
    Ok((start, end))
}

fn parse_model(raw: Option<&str>) -> Result<AttributionModel, AppError> {
    match raw.map(str::trim) {
        None | Some("") | Some("last_touch") => Ok(AttributionModel::LastTouch),
        Some("first_touch") => Ok(AttributionModel::FirstTouch),
        Some(_) => Err(AppError::BadRequest(
            "model must be one of: first_touch, last_touch".to_string(),
        )),
    }
}

fn build_filter(
    query: AttributionRequestQuery,
) -> Result<(AnalyticsFilter, AttributionQuery), AppError> {
    let normalized_timezone = query
        .timezone
        .as_ref()
        .map(|raw| raw.trim())
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            raw.parse::<chrono_tz::Tz>()
                .map(|_| raw.to_string())
                .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))
        })
        .transpose()?;
    let (start_date, end_date) = parse_date_range(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        normalized_timezone.as_deref(),
    )?;
    let model = parse_model(query.model.as_deref())?;

    let filter = AnalyticsFilter {
        start_date,
        end_date,
        timezone: normalized_timezone,
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

    let attribution = AttributionQuery {
        goal_id: query.goal_id,
        model,
    };

    Ok((filter, attribution))
}

pub async fn get_attribution(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<AttributionRequestQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let (filter, attribution_query) = build_filter(query)?;

    let data = state
        .analytics
        .get_attribution(&website_id, None, &filter, &attribution_query)
        .await
        .map_err(|err| {
            if err.to_string().contains("Goal not found") {
                AppError::NotFound("Goal not found".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;

    Ok(Json(json!({ "data": data })))
}

pub async fn get_revenue_summary(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<AttributionRequestQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let (filter, attribution_query) = build_filter(query)?;

    let data = state
        .analytics
        .get_revenue_summary(&website_id, None, &filter, &attribution_query)
        .await
        .map_err(|err| {
            if err.to_string().contains("Goal not found") {
                AppError::NotFound("Goal not found".to_string())
            } else {
                AppError::Internal(err)
            }
        })?;

    Ok(Json(json!({ "data": data })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_defaults_to_last_touch() {
        assert!(matches!(parse_model(None), Ok(AttributionModel::LastTouch)));
    }

    #[test]
    fn parse_model_rejects_invalid() {
        assert!(parse_model(Some("bad")).is_err());
    }

    #[test]
    fn parse_date_range_rejects_invalid_start_date() {
        let result = parse_date_range(Some("2026-13-01"), Some("2026-02-20"), None);
        assert!(result.is_err());
    }
}
