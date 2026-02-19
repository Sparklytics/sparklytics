use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use sparklytics_duckdb::website::{CreateWebsiteParams, UpdateWebsiteParams};

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct CreateWebsiteRequest {
    pub name: String,
    pub domain: String,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebsiteRequest {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListWebsitesQuery {
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

/// `POST /api/websites` — Create a new website.
pub async fn create_website(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateWebsiteRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.name.is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    if req.domain.is_empty() {
        return Err(AppError::BadRequest("domain is required".to_string()));
    }

    let website = state
        .db
        .create_website(CreateWebsiteParams {
            name: req.name,
            domain: req.domain.clone(),
            timezone: req.timezone,
        })
        .await
        .map_err(AppError::Internal)?;

    // Add to website cache.
    {
        let mut cache = state.website_cache.write().await;
        cache.insert(website.id.clone());
    }

    let tracking_snippet = format!(
        r#"<script defer src="{}/s.js" data-website-id="{}"></script>"#,
        state.config.public_url, website.id
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": {
                "id": website.id,
                "tenant_id": website.tenant_id,
                "name": website.name,
                "domain": website.domain,
                "timezone": website.timezone,
                "tracking_snippet": tracking_snippet,
                "created_at": website.created_at,
            }
        })),
    ))
}

/// `GET /api/websites` — List all websites.
pub async fn list_websites(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListWebsitesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let cursor = query.cursor.as_deref();

    let (websites, total, has_more) = state
        .db
        .list_websites(limit, cursor)
        .await
        .map_err(AppError::Internal)?;

    let next_cursor = if has_more {
        websites.last().map(|w| w.id.clone())
    } else {
        None
    };

    Ok(Json(json!({
        "data": websites,
        "pagination": {
            "total": total,
            "limit": limit,
            "cursor": next_cursor,
            "has_more": has_more,
        }
    })))
}

/// `PUT /api/websites/:id` — Update a website.
pub async fn update_website(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<UpdateWebsiteRequest>,
) -> Result<impl IntoResponse, AppError> {
    let result = state
        .db
        .update_website(
            &website_id,
            UpdateWebsiteParams {
                name: req.name,
                domain: req.domain,
                timezone: req.timezone,
            },
        )
        .await
        .map_err(AppError::Internal)?;

    match result {
        Some(website) => Ok(Json(json!({
            "data": {
                "id": website.id,
                "name": website.name,
                "domain": website.domain,
                "timezone": website.timezone,
                "updated_at": website.updated_at,
            }
        }))),
        None => Err(AppError::NotFound("Website not found".to_string())),
    }
}

/// `DELETE /api/websites/:id` — Delete a website and all analytics data.
pub async fn delete_website(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let deleted = state
        .db
        .delete_website(&website_id)
        .await
        .map_err(AppError::Internal)?;

    if !deleted {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    // Evict from website cache.
    {
        let mut cache = state.website_cache.write().await;
        cache.remove(&website_id);
    }

    Ok(StatusCode::NO_CONTENT)
}
