use std::sync::Arc;
use std::{net::IpAddr, str::FromStr};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use sparklytics_metadata::{CreateWebsiteParams, UpdateWebsiteParams};

use crate::{error::AppError, state::AppState};

fn normalize_domain(raw: &str) -> Result<String, AppError> {
    let domain = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        return Err(AppError::BadRequest("domain is required".to_string()));
    }
    if domain.contains("://")
        || domain.contains('/')
        || domain.contains('@')
        || domain.contains('?')
        || domain.contains('#')
        || domain.contains(':')
    {
        return Err(AppError::BadRequest(
            "domain must be a hostname without scheme, path, credentials, or port".to_string(),
        ));
    }

    if domain == "localhost" || IpAddr::from_str(&domain).is_ok() {
        return Ok(domain);
    }

    url::Host::parse(&domain).map_err(|_| AppError::BadRequest("domain is invalid".to_string()))?;
    if psl::domain(domain.as_bytes()).is_none() {
        return Err(AppError::BadRequest(
            "domain must be a registrable hostname (not a public suffix)".to_string(),
        ));
    }
    Ok(domain)
}

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
    let domain = normalize_domain(&req.domain)?;

    let website = state
        .metadata
        .create_website(CreateWebsiteParams {
            name: req.name,
            domain: domain.clone(),
            timezone: req.timezone,
        })
        .await
        .map_err(AppError::Internal)?;

    state.cache_website_metadata(website.clone()).await;

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
                "domain": domain,
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
        .metadata
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
    Json(mut req): Json<UpdateWebsiteRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(domain) = req.domain.clone() {
        req.domain = Some(normalize_domain(&domain)?);
    }

    let result = state
        .metadata
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
        Some(website) => {
            state.cache_website_metadata(website.clone()).await;
            Ok(Json(json!({
                "data": {
                    "id": website.id,
                    "name": website.name,
                    "domain": website.domain,
                    "timezone": website.timezone,
                    "updated_at": website.updated_at,
                }
            })))
        }
        None => Err(AppError::NotFound("Website not found".to_string())),
    }
}

/// `GET /api/websites/:id` — Get a single website by ID.
#[tracing::instrument(skip(state))]
pub async fn get_website(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let website = state
        .get_website_metadata_cached(&website_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;

    Ok(Json(json!({ "data": website })))
}

/// `DELETE /api/websites/:id` — Delete a website and all analytics data.
pub async fn delete_website(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let deleted = state
        .metadata
        .delete_website(&website_id)
        .await
        .map_err(AppError::Internal)?;

    if !deleted {
        return Err(AppError::NotFound("Website not found".to_string()));
    }

    state.invalidate_website_metadata_cache(&website_id).await;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::normalize_domain;

    #[test]
    fn normalize_domain_rejects_public_suffix() {
        assert!(normalize_domain("com").is_err());
        assert!(normalize_domain("co.uk").is_err());
    }

    #[test]
    fn normalize_domain_accepts_registrable_domain() {
        let normalized = normalize_domain("Example.COM").expect("domain");
        assert_eq!(normalized, "example.com");
    }
}
