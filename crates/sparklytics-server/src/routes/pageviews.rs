use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct PageviewsQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub granularity: Option<String>,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
}

/// `GET /api/websites/:id/pageviews` — Time series data.
pub async fn get_pageviews(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<PageviewsQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
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

    let result = state
        .db
        .get_timeseries(
            &website_id,
            &start_date,
            &end_date,
            query.granularity.as_deref(),
            query.filter_country.as_deref(),
            query.filter_page.as_deref(),
        )
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({
        "data": {
            "series": result.series,
            "granularity": result.granularity,
        }
    })))
}
