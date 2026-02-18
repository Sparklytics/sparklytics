use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde_json::json;

use sparklytics_core::{
    event::{CollectOrBatch, CollectPayload, Event},
    visitor::{compute_visitor_id, extract_referrer_domain},
};

use crate::{error::AppError, state::AppState};

/// `POST /api/collect` — ingest a single event or a batch of up to 50 events.
///
/// ## Auth
/// None required. Events for unknown `website_id` values are rejected with 404.
///
/// ## Rate limiting
/// 60 req/min per IP — enforced by Tower middleware (Sprint 1).
/// The response headers `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
/// `X-RateLimit-Reset` are added by the rate-limit middleware (Sprint 1).
///
/// ## Batch rules (CLAUDE.md critical facts)
/// - Maximum **50** events per batch (returns 400 `batch_too_large` otherwise).
/// - `tenant_id` is always `NULL` in self-hosted mode (critical fact #2).
///
/// ## Enrichment (Sprint 0 deliverables)
/// - `visitor_id`: `sha256(salt_epoch + ip + user_agent)[0..8]` → 16 hex chars.
/// - `referrer_domain`: parsed from the `referrer` URL field.
/// - `country`, `region`, `city`: GeoIP via `maxminddb` (stubbed if .mmdb absent).
/// - `browser`, `browser_version`, `os`, `os_version`, `device_type`: UA parsing
///   via `woothee`.
///
/// ## Response
/// `202 Accepted` with `{ "ok": true }`.
#[tracing::instrument(skip(state, headers, payload))]
pub async fn collect(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CollectOrBatch>,
) -> Result<impl IntoResponse, AppError> {
    // Normalise single event / batch into a uniform Vec.
    let payloads: Vec<CollectPayload> = match payload {
        CollectOrBatch::Single(p) => vec![p],
        CollectOrBatch::Batch(v) => v,
    };

    // --- Validation: batch size (max 50) ---
    if payloads.len() > 50 {
        return Err(AppError::BadRequest(
            "batch_too_large: maximum 50 events per batch".to_string(),
        ));
    }

    if payloads.is_empty() {
        return Err(AppError::BadRequest("empty batch".to_string()));
    }

    // --- Validation: all website_ids must be known ---
    for p in &payloads {
        if !state.is_valid_website(&p.website_id).await {
            return Err(AppError::NotFound(format!(
                "Unknown website_id: {}",
                p.website_id
            )));
        }
    }

    // --- Extract client IP (X-Forwarded-For or a placeholder) ---
    // The real remote-addr fallback is wired in Sprint 1 via ConnectInfo middleware.
    let client_ip = extract_client_ip(&headers);

    // --- Extract User-Agent header ---
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // --- GeoIP lookup ---
    // Load the GeoIP database from the path configured at startup.
    // If the file is absent, geo fields are left as None (non-fatal, per Sprint 0).
    let geo = lookup_geo(&state.config.geoip_path, &client_ip);

    // --- UA parsing ---
    let ua_info = parse_user_agent(&user_agent);

    // --- Build enriched Event structs ---
    let events: Vec<Event> = payloads
        .into_iter()
        .map(|p| {
            let visitor_id = compute_visitor_id(&client_ip, &user_agent);
            let referrer_domain = p
                .referrer
                .as_deref()
                .and_then(extract_referrer_domain);

            Event {
                id: uuid::Uuid::new_v4().to_string(),
                website_id: p.website_id,
                // tenant_id is always NULL in self-hosted mode (CLAUDE.md critical fact #2).
                tenant_id: None,
                // session_id: derived in Sprint 1 session-management layer.
                session_id: String::new(),
                visitor_id,
                event_type: p.event_type,
                url: p.url,
                referrer_url: p.referrer.clone(),
                referrer_domain,
                event_name: p.event_name,
                // event_data is serialised to a JSON string for DuckDB VARCHAR storage.
                event_data: p.event_data.map(|v| v.to_string()),
                country: geo.as_ref().and_then(|g| g.country.clone()),
                region: geo.as_ref().and_then(|g| g.region.clone()),
                city: geo.as_ref().and_then(|g| g.city.clone()),
                browser: ua_info.as_ref().map(|u| u.browser.clone()),
                browser_version: ua_info.as_ref().and_then(|u| u.browser_version.clone()),
                os: ua_info.as_ref().map(|u| u.os.clone()),
                os_version: ua_info.as_ref().and_then(|u| u.os_version.clone()),
                device_type: ua_info.as_ref().map(|u| u.device_type.clone()),
                screen: p.screen,
                language: p.language,
                utm_source: p.utm_source,
                utm_medium: p.utm_medium,
                utm_campaign: p.utm_campaign,
                utm_term: p.utm_term,
                utm_content: p.utm_content,
                created_at: Utc::now(),
            }
        })
        .collect();

    state.push_events(events).await;

    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "ok": true })),
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the real client IP from `X-Forwarded-For` (first entry).
///
/// Falls back to `"unknown"` when the header is absent. Sprint 1 will wire
/// in `ConnectInfo<SocketAddr>` as a proper TCP-addr fallback once the
/// Tower `into_make_service_with_connect_info` plumbing is added.
fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// GeoIP result from a MaxMind lookup.
struct GeoInfo {
    country: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

/// Attempt a GeoIP lookup for `ip` using the MaxMind database at `path`.
///
/// Returns `None` if the database file is missing or the IP cannot be parsed.
/// This is the Sprint 0 non-fatal behaviour: events are stored with NULL geo
/// fields rather than panicking.
fn lookup_geo(path: &str, ip: &str) -> Option<GeoInfo> {
    use std::net::IpAddr;
    use std::str::FromStr;

    if !std::path::Path::new(path).exists() {
        // Database absent — non-fatal. Warning already logged at startup.
        return None;
    }

    let reader = maxminddb::Reader::open_readfile(path).ok()?;
    let ip_addr = IpAddr::from_str(ip).ok()?;

    let record: maxminddb::geoip2::City = reader.lookup(ip_addr).ok()?;

    let country = record
        .country
        .as_ref()
        .and_then(|c| c.iso_code)
        .map(|s| s.to_string());

    let region = record
        .subdivisions
        .as_ref()
        .and_then(|subs| subs.first())
        .and_then(|sub| sub.names.as_ref())
        .and_then(|names| names.get("en"))
        .map(|s| s.to_string());

    let city = record
        .city
        .as_ref()
        .and_then(|c| c.names.as_ref())
        .and_then(|names| names.get("en"))
        .map(|s| s.to_string());

    Some(GeoInfo {
        country,
        region,
        city,
    })
}

/// Parsed User-Agent fields.
struct UaInfo {
    browser: String,
    browser_version: Option<String>,
    os: String,
    os_version: Option<String>,
    device_type: String,
}

/// Parse a `User-Agent` string via the `woothee` crate.
///
/// Returns `None` if the UA string is empty or `woothee` cannot classify it.
fn parse_user_agent(user_agent: &str) -> Option<UaInfo> {
    if user_agent.is_empty() {
        return None;
    }

    let result = woothee::parser::Parser::new().parse(user_agent)?;

    // woothee `category` maps to our device_type convention:
    //   "pc"         → "desktop"
    //   "smartphone" → "mobile"
    //   "mobilephone"→ "mobile"
    //   "tablet"     → "tablet"
    //   everything else → "desktop" (conservative default)
    let device_type = match result.category {
        "smartphone" | "mobilephone" => "mobile",
        "tablet" => "tablet",
        _ => "desktop",
    }
    .to_string();

    // woothee returns empty string for unknown values; normalise to None.
    let browser_version = if result.version.is_empty() {
        None
    } else {
        Some(result.version.to_string())
    };

    let os_version = if result.os_version.is_empty() {
        None
    } else {
        Some(result.os_version.to_string())
    };

    Some(UaInfo {
        browser: result.name.to_string(),
        browser_version,
        os: result.os.to_string(),
        os_version,
        device_type,
    })
}
