use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::{error::AppError, state::AppState};

/// `GET /api/websites/:id/realtime` — Current active visitors + recent events.
pub async fn get_realtime(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    let result = state
        .analytics
        .get_realtime(&website_id, None)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(json!({ "data": result })))
}
