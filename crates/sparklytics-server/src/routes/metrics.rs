use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use sparklytics_duckdb::queries::metrics::is_valid_metric_type;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
}

/// `GET /api/websites/:id/metrics` — Breakdown by dimension.
pub async fn get_metrics(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<MetricsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    if !is_valid_metric_type(&query.metric_type) {
        return Err(AppError::BadRequest(format!(
            "invalid metric type: {}",
            query.metric_type
        )));
    }

    let today = chrono::Utc::now().date_naive();
    let start_date = query
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(6));
    let end_date = query
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);

    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let (result, pagination) = state
        .db
        .get_metrics(
            &website_id,
            &start_date,
            &end_date,
            &query.metric_type,
            limit,
            offset,
            query.filter_country.as_deref(),
            query.filter_page.as_deref(),
        )
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": result,
        "pagination": pagination,
    })))
}
