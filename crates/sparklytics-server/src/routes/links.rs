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
    analytics::{CreateCampaignLinkRequest, UpdateCampaignLinkRequest},
    event::Event,
    visitor::{compute_visitor_id, extract_referrer_domain},
};

use crate::{error::AppError, routes::collect, state::AppState};

const MAX_PUBLIC_QUERY_PARAMS: usize = 32;
const MAX_PUBLIC_QUERY_KEY_BYTES: usize = 64;
const MAX_PUBLIC_QUERY_VALUE_BYTES: usize = 256;
const MAX_TRACKING_URL_BYTES: usize = 2048;
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

fn parse_destination_url(url: &str) -> Result<Url, AppError> {
    let parsed = Url::parse(url)
        .map_err(|_| AppError::BadRequest("destination_url must be a valid URL".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "destination_url must use http:// or https://".to_string(),
        ));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::BadRequest(
            "destination_url must include a hostname".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::BadRequest(
            "destination_url cannot include credentials".to_string(),
        ));
    }
    Ok(parsed)
}

fn destination_matches_website(url: &Url, website_domain: &str) -> bool {
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

fn encode_query_component(raw: &str) -> String {
    let mut out = String::new();
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

fn append_query_param(url: &mut String, has_query: &mut bool, key: &str, value: &str) {
    url.push(if *has_query { '&' } else { '?' });
    *has_query = true;
    url.push_str(key);
    url.push('=');
    url.push_str(&encode_query_component(value));
}

fn build_tracking_destination(link: &sparklytics_core::analytics::CampaignLink) -> String {
    let mut destination = link.destination_url.clone();
    let mut has_query = destination.contains('?');

    if let Some(ref value) = link.utm_source {
        append_query_param(&mut destination, &mut has_query, "utm_source", value);
    }
    if let Some(ref value) = link.utm_medium {
        append_query_param(&mut destination, &mut has_query, "utm_medium", value);
    }
    if let Some(ref value) = link.utm_campaign {
        append_query_param(&mut destination, &mut has_query, "utm_campaign", value);
    }
    if let Some(ref value) = link.utm_term {
        append_query_param(&mut destination, &mut has_query, "utm_term", value);
    }
    if let Some(ref value) = link.utm_content {
        append_query_param(&mut destination, &mut has_query, "utm_content", value);
    }

    destination
}

fn link_response(link: &sparklytics_core::analytics::CampaignLink, public_url: &str) -> Value {
    json!({
        "id": link.id,
        "website_id": link.website_id,
        "name": link.name,
        "slug": link.slug,
        "destination_url": link.destination_url,
        "utm_source": link.utm_source,
        "utm_medium": link.utm_medium,
        "utm_campaign": link.utm_campaign,
        "utm_term": link.utm_term,
        "utm_content": link.utm_content,
        "is_active": link.is_active,
        "created_at": link.created_at,
        "clicks": link.clicks.unwrap_or(0),
        "unique_visitors": link.unique_visitors.unwrap_or(0),
        "conversions": link.conversions.unwrap_or(0),
        "revenue": link.revenue.unwrap_or(0.0),
        "tracking_url": format!("{public_url}/l/{}", link.slug),
    })
}

pub async fn list_links(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let links = state
        .analytics
        .list_campaign_links(&website_id, None)
        .await
        .map_err(AppError::Internal)?;
    let data: Vec<Value> = links
        .iter()
        .map(|link| link_response(link, &state.config.public_url))
        .collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_link(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(req): Json<CreateCampaignLinkRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    validate_name(&req.name)?;
    let destination_url = parse_destination_url(&req.destination_url)?;
    let website = state
        .db
        .get_website(&website_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;
    if !destination_matches_website(&destination_url, &website.domain) {
        return Err(AppError::BadRequest(
            "destination_url host must match website domain".to_string(),
        ));
    }

    let link = state
        .analytics
        .create_campaign_link(&website_id, None, req)
        .await
        .map_err(AppError::Internal)?;
    state.invalidate_acquisition_cache().await;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "data": link_response(&link, &state.config.public_url)
        })),
    ))
}

pub async fn update_link(
    State(state): State<Arc<AppState>>,
    Path((website_id, link_id)): Path<(String, String)>,
    Json(req): Json<UpdateCampaignLinkRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    if let Some(ref name) = req.name {
        validate_name(name)?;
    }
    if let Some(ref destination_url) = req.destination_url {
        let parsed = parse_destination_url(destination_url)?;
        let website = state
            .db
            .get_website(&website_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;
        if !destination_matches_website(&parsed, &website.domain) {
            return Err(AppError::BadRequest(
                "destination_url host must match website domain".to_string(),
            ));
        }
    }

    let link = state
        .analytics
        .update_campaign_link(&website_id, None, &link_id, req)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Campaign link not found".to_string()))?;
    state.invalidate_acquisition_cache().await;

    Ok(Json(json!({
        "data": link_response(&link, &state.config.public_url)
    })))
}

pub async fn delete_link(
    State(state): State<Arc<AppState>>,
    Path((website_id, link_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let deleted = state
        .analytics
        .delete_campaign_link(&website_id, None, &link_id)
        .await
        .map_err(AppError::Internal)?;
    if !deleted {
        return Err(AppError::NotFound("Campaign link not found".to_string()));
    }
    state.invalidate_acquisition_cache().await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_link_stats(
    State(state): State<Arc<AppState>>,
    Path((website_id, link_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    if !state.is_valid_website(&website_id).await {
        return Err(AppError::NotFound("Website not found".to_string()));
    }
    let stats = state
        .analytics
        .get_campaign_link_stats(&website_id, None, &link_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({ "data": stats })))
}

pub async fn track_link_redirect(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    maybe_connect_info: collect::MaybeConnectInfo,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let client_ip = collect::extract_client_ip(
        &headers,
        maybe_connect_info.0,
    );
    if !state.check_rate_limit_with_max(&client_ip, 120).await {
        return Err(AppError::RateLimited);
    }

    let link = state
        .get_campaign_link_by_slug_cached(&slug)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Campaign link not found".to_string()))?;

    if !link.is_active {
        return Err(AppError::NotFound("Campaign link not found".to_string()));
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
    let destination_url = build_tracking_destination(&link);
    if destination_url.len() > MAX_TRACKING_URL_BYTES {
        return Err(AppError::BadRequest(
            "destination_url exceeds max length".to_string(),
        ));
    }
    if query.len() > MAX_PUBLIC_QUERY_PARAMS {
        return Err(AppError::BadRequest(
            "too many query parameters".to_string(),
        ));
    }

    let mut sanitized_query = serde_json::Map::new();
    for (k, v) in query {
        if !k.is_empty()
            && k.len() <= MAX_PUBLIC_QUERY_KEY_BYTES
            && v.len() <= MAX_PUBLIC_QUERY_VALUE_BYTES
        {
            sanitized_query.insert(k, Value::String(v));
        }
    }

    let link_id = link.id.clone();
    let event_data = json!({
        "link_id": link_id,
        "slug": link.slug,
        "destination_url": destination_url,
        "query": sanitized_query,
    });
    let serialized_event_data = event_data.to_string();
    if serialized_event_data.len() > MAX_PUBLIC_EVENT_DATA_BYTES {
        return Err(AppError::PayloadTooLarge);
    }
    let event = Event {
        id: uuid::Uuid::new_v4().to_string(),
        website_id: link.website_id,
        tenant_id: None,
        session_id: AppState::pending_session_marker().to_string(),
        visitor_id,
        event_type: "event".to_string(),
        url: destination_url.clone(),
        referrer_url: referrer_url.clone(),
        referrer_domain: referrer_url.as_deref().and_then(extract_referrer_domain),
        event_name: Some("link_click".to_string()),
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
        utm_source: link.utm_source,
        utm_medium: link.utm_medium,
        utm_campaign: link.utm_campaign,
        utm_term: link.utm_term,
        utm_content: link.utm_content,
        link_id: Some(link.id),
        pixel_id: None,
        created_at: Utc::now(),
    };
    state.enqueue_ingest_events(vec![event]).await?;

    let mut response = StatusCode::FOUND.into_response();
    let location = HeaderValue::from_str(&destination_url)
        .map_err(|_| AppError::BadRequest("invalid destination_url".to_string()))?;
    response.headers_mut().insert(header::LOCATION, location);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_link() -> sparklytics_core::analytics::CampaignLink {
        sparklytics_core::analytics::CampaignLink {
            id: "lnk_1".to_string(),
            website_id: "site_1".to_string(),
            name: "Sample".to_string(),
            slug: "abc123".to_string(),
            destination_url: "https://example.com/pricing".to_string(),
            utm_source: Some("newsletter".to_string()),
            utm_medium: Some("email".to_string()),
            utm_campaign: Some("spring launch".to_string()),
            utm_term: None,
            utm_content: None,
            is_active: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            clicks: None,
            unique_visitors: None,
            conversions: None,
            revenue: None,
        }
    }

    #[test]
    fn parse_destination_url_rejects_non_http_scheme() {
        let result = parse_destination_url("javascript:alert(1)");
        assert!(result.is_err());
    }

    #[test]
    fn destination_matches_subdomain() {
        let parsed = parse_destination_url("https://app.example.com/path").expect("url");
        assert!(destination_matches_website(&parsed, "example.com"));
    }

    #[test]
    fn build_tracking_destination_encodes_spaces() {
        let destination = build_tracking_destination(&sample_link());
        assert!(destination.contains("utm_source=newsletter"));
        assert!(destination.contains("utm_medium=email"));
        assert!(destination.contains("utm_campaign=spring%20launch"));
    }
}
