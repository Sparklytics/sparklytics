use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::{json, Value};
use url::Url;

use sparklytics_core::{
    analytics::{CreateTrackingPixelRequest, UpdateTrackingPixelRequest},
    event::Event,
    visitor::{compute_visitor_id, extract_referrer_domain},
};

use crate::{error::AppError, routes::collect, state::AppState};

const TRANSPARENT_GIF: &[u8] = &[
    71, 73, 70, 56, 57, 97, 1, 0, 1, 0, 128, 0, 0, 0, 0, 0, 255, 255, 255, 33, 249, 4, 1, 0, 0, 0,
    0, 44, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 68, 1, 0, 59,
];
const MAX_PUBLIC_QUERY_PARAMS: usize = 32;
const MAX_PUBLIC_QUERY_KEY_BYTES: usize = 64;
const MAX_PUBLIC_QUERY_VALUE_BYTES: usize = 256;
const MAX_PIXEL_EVENT_URL_BYTES: usize = 2048;
const MAX_PUBLIC_EVENT_DATA_BYTES: usize = 4096;

fn validate_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }
    if name.len() > 100 {
        return Err(AppError::BadRequest(
            "name must be 100 characters or fewer".to_string(),
        ));
    }
    Ok(())
}

fn parse_default_url(url: &str) -> Result<Url, AppError> {
    let parsed = Url::parse(url)
        .map_err(|_| AppError::BadRequest("default_url must be valid".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "default_url must use http:// or https://".to_string(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::BadRequest(
            "default_url must include a hostname".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::BadRequest(
            "default_url cannot include credentials".to_string(),
        ));
    }
    Ok(parsed)
}

fn url_matches_website(url: &Url, website_domain: &str) -> bool {
    let host = match url.host_str() {
        Some(host) => host.to_ascii_lowercase(),
        None => return false,
    };
    let website_domain = website_domain
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_ascii_lowercase();

    host == website_domain || host.ends_with(&format!(".{website_domain}"))
}

fn pixel_response(pixel: &sparklytics_core::analytics::TrackingPixel, public_url: &str) -> Value {
    json!({
        "id": pixel.id,
        "website_id": pixel.website_id,
        "name": pixel.name,
        "pixel_key": pixel.pixel_key,
        "default_url": pixel.default_url,
        "is_active": pixel.is_active,
        "created_at": pixel.created_at,
        "views": pixel.views.unwrap_or(0),
        "unique_visitors": pixel.unique_visitors.unwrap_or(0),
        "pixel_url": format!("{public_url}/p/{}.gif", pixel.pixel_key),
        "snippet": format!("<img src=\"{public_url}/p/{}.gif\" width=\"1\" height=\"1\" style=\"display:none\" alt=\"\" />", pixel.pixel_key),
    })
}

fn normalize_pixel_key(raw: &str) -> String {
    raw.trim_end_matches(".gif").to_string()
}

pub async fn list_pixels(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let pixels = state
        .analytics
        .list_tracking_pixels(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    let data: Vec<Value> = pixels
        .iter()
        .map(|pixel| pixel_response(pixel, &state.config.public_url))
        .collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_pixel(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateTrackingPixelRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_name(&req.name)?;
    if let Some(ref default_url) = req.default_url {
        let parsed = parse_default_url(default_url)?;
        let website = state
            .db
            .get_website(&website_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;
        if !url_matches_website(&parsed, &website.domain) {
            return Err(AppError::BadRequest(
                "default_url host must match website domain".to_string(),
            ));
        }
    }

    let pixel = state
        .analytics
        .create_tracking_pixel(&website_id, None, req)
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_acquisition_cache().await;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": pixel_response(&pixel, &state.config.public_url)
        })),
    ))
}

pub async fn update_pixel(
    State(state): State<Arc<AppState>>,
    Path((website_id, pixel_id)): Path<(String, String)>,
    Json(req): Json<UpdateTrackingPixelRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    if let Some(ref name) = req.name {
        validate_name(name)?;
    }
    if let Some(Some(ref default_url)) = req.default_url {
        let parsed = parse_default_url(default_url)?;
        let website = state
            .db
            .get_website(&website_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;
        if !url_matches_website(&parsed, &website.domain) {
            return Err(AppError::BadRequest(
                "default_url host must match website domain".to_string(),
            ));
        }
    }

    let pixel = state
        .analytics
        .update_tracking_pixel(&website_id, None, &pixel_id, req)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Tracking pixel not found".to_string()))?;
    state.invalidate_acquisition_cache().await;
    Ok(Json(json!({
        "data": pixel_response(&pixel, &state.config.public_url)
    })))
}

pub async fn delete_pixel(
    State(state): State<Arc<AppState>>,
    Path((website_id, pixel_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .analytics
        .delete_tracking_pixel(&website_id, None, &pixel_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("Tracking pixel not found".to_string()));
    }
    state.invalidate_acquisition_cache().await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_pixel_stats(
    State(state): State<Arc<AppState>>,
    Path((website_id, pixel_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let stats = state
        .analytics
        .get_tracking_pixel_stats(&website_id, None, &pixel_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": stats })))
}

pub async fn track_pixel(
    State(state): State<Arc<AppState>>,
    Path(raw_pixel_key): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    maybe_connect_info: collect::MaybeConnectInfo,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let client_ip = collect::extract_client_ip(&headers, maybe_connect_info.0);
    if !state.check_rate_limit_with_max(&client_ip, 240).await {
        return Err(AppError::RateLimited);
    }

    let pixel_key = normalize_pixel_key(&raw_pixel_key);
    let pixel = state
        .get_tracking_pixel_by_key_cached(&pixel_key)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Tracking pixel not found".to_string()))?;
    if !pixel.is_active {
        return Err(AppError::NotFound("Tracking pixel not found".to_string()));
    }

    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let geo = collect::lookup_geo(&state.config.geoip_path, &client_ip);
    let ua = collect::parse_user_agent(&user_agent);
    let visitor_id = compute_visitor_id(&client_ip, &user_agent);
    let referrer_url = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let event_url = query
        .get("url")
        .cloned()
        .or_else(|| pixel.default_url.clone())
        .unwrap_or_else(|| format!("pixel://{}", pixel.pixel_key));
    if event_url.len() > MAX_PIXEL_EVENT_URL_BYTES {
        return Err(AppError::BadRequest("url exceeds max length".to_string()));
    }
    if query.len() > MAX_PUBLIC_QUERY_PARAMS {
        return Err(AppError::BadRequest(
            "too many query parameters".to_string(),
        ));
    }
    let url_utm = collect::extract_utm_from_url(&event_url);

    let mut sanitized_query = serde_json::Map::new();
    for (k, v) in query {
        if !k.is_empty()
            && k.len() <= MAX_PUBLIC_QUERY_KEY_BYTES
            && v.len() <= MAX_PUBLIC_QUERY_VALUE_BYTES
        {
            sanitized_query.insert(k, Value::String(v));
        }
    }

    let pixel_id = pixel.id.clone();
    let event_data = json!({
        "pixel_id": pixel_id,
        "pixel_key": pixel.pixel_key,
        "url": event_url,
        "query": sanitized_query,
    });
    let serialized_event_data = event_data.to_string();
    if serialized_event_data.len() > MAX_PUBLIC_EVENT_DATA_BYTES {
        return Err(AppError::PayloadTooLarge);
    }

    let event = Event {
        id: uuid::Uuid::new_v4().to_string(),
        website_id: pixel.website_id,
        tenant_id: None,
        session_id: AppState::pending_session_marker().to_string(),
        visitor_id,
        event_type: "event".to_string(),
        url: event_url,
        referrer_url: referrer_url.clone(),
        referrer_domain: referrer_url.as_deref().and_then(extract_referrer_domain),
        event_name: Some("pixel_view".to_string()),
        event_data: Some(serialized_event_data),
        country: geo.as_ref().and_then(|g| g.country.clone()),
        region: geo.as_ref().and_then(|g| g.region.clone()),
        city: geo.as_ref().and_then(|g| g.city.clone()),
        browser: ua.as_ref().map(|u| u.browser.clone()),
        browser_version: ua.as_ref().and_then(|u| u.browser_version.clone()),
        os: ua.as_ref().map(|u| u.os.clone()),
        os_version: ua.as_ref().and_then(|u| u.os_version.clone()),
        device_type: ua.as_ref().map(|u| u.device_type.clone()),
        screen: None,
        language: headers
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
        utm_source: url_utm.get("utm_source").cloned(),
        utm_medium: url_utm.get("utm_medium").cloned(),
        utm_campaign: url_utm.get("utm_campaign").cloned(),
        utm_term: url_utm.get("utm_term").cloned(),
        utm_content: url_utm.get("utm_content").cloned(),
        link_id: None,
        pixel_id: Some(pixel.id),
        source_ip: Some(client_ip),
        user_agent: Some(user_agent),
        is_bot: false,
        bot_score: 0,
        bot_reason: None,
        created_at: Utc::now(),
    };
    state.enqueue_ingest_events(vec![event]).await?;

    let mut response = Response::new(axum::body::Body::from(TRANSPARENT_GIF.to_vec()));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/gif"));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pixel_key_strips_gif_suffix() {
        assert_eq!(normalize_pixel_key("px_abc.gif"), "px_abc");
        assert_eq!(normalize_pixel_key("px_abc"), "px_abc");
    }

    #[test]
    fn parse_default_url_rejects_non_http() {
        let result = parse_default_url("ftp://example.com/file");
        assert!(result.is_err());
    }

    #[test]
    fn transparent_gif_has_valid_header() {
        assert_eq!(&TRANSPARENT_GIF[0..6], b"GIF89a");
    }
}
