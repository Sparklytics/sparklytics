use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct RealtimeQuery {
    pub include_bots: Option<bool>,
}

/// `GET /api/websites/:id/realtime` — Current active visitors + recent events.
pub async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Query(query): Query<RealtimeQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let include_bots = query
        .include_bots
        .unwrap_or(state.default_include_bots(&website_id).await);
    let result = state
        .analytics
        .get_realtime(&website_id, None, include_bots)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}
