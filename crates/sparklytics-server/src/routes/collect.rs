use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

/// Maximum allowed body size for POST /api/collect (100 KB).
pub const COLLECT_BODY_LIMIT: usize = 102_400;
/// Maximum allowed size for a single event's `event_data` JSON string (4 KB).
const EVENT_DATA_MAX_BYTES: usize = 4_096;
use chrono::Utc;
use serde_json::json;

use sparklytics_core::{
    billing::BillingOutcome,
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
) -> Result<Response, AppError> {
    // Normalise single event / batch into a uniform Vec.
    let payloads: Vec<CollectPayload> = match payload {
        CollectOrBatch::Single(p) => vec![*p],
        CollectOrBatch::Batch(v) => v,
    };

    // --- Validation: batch size (max 50) ---
    if payloads.len() > 50 {
        return Err(AppError::BatchTooLarge(payloads.len()));
    }

    if payloads.is_empty() {
        return Err(AppError::BadRequest("empty batch".to_string()));
    }

    // --- Validation: per-event event_data size (max 4KB serialised) ---
    for p in &payloads {
        if let Some(data) = &p.event_data {
            if data.to_string().len() > EVENT_DATA_MAX_BYTES {
                return Err(AppError::PayloadTooLarge);
            }
        }
    }

    let cloud_tenant_id: Option<String> = None;

    // --- BillingGate check (cloud mode) ---
    if let Some(ref tenant_id) = cloud_tenant_id {
        let outcome = state.billing_gate.check(tenant_id).await;
        if outcome == BillingOutcome::LimitExceeded {
            return Err(AppError::PlanLimitExceeded);
        }
    }

    // --- Validation: all website_ids must be known ---
    // Validate unique IDs only to avoid repeated cache/DB lookups for batches.
    let unique_website_ids: HashSet<&str> =
        payloads.iter().map(|p| p.website_id.as_str()).collect();
    for website_id in unique_website_ids {
        if !state.is_valid_website(website_id).await {
            return Err(AppError::NotFound(format!(
                "Unknown website_id: {}",
                website_id
            )));
        }
    }

    // --- Extract client IP (X-Forwarded-For or a placeholder) ---
    // The real remote-addr fallback is wired in Sprint 1 via ConnectInfo middleware.
    let client_ip = extract_client_ip(&headers);

    // --- Rate limiting: 60 req/min per IP ---
    // SPARKLYTICS_RATE_LIMIT_DISABLE bypasses this for load testing only.
    if !state.config.rate_limit_disable && !state.check_rate_limit(&client_ip).await {
        return Err(AppError::RateLimited);
    }

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
    let mut events: Vec<Event> = Vec::with_capacity(payloads.len());
    let base_now = Utc::now();
    let server_visitor_id = compute_visitor_id(&client_ip, &user_agent);

    for (idx, p) in payloads.into_iter().enumerate() {
        let website_id = p.website_id.clone();
        let referrer_domain = p.referrer.as_deref().and_then(extract_referrer_domain);

        // Use client-supplied visitor_id when present (max 64 chars),
        // otherwise fall back to the server-computed hash.
        let visitor_id = p
            .visitor_id
            .filter(|id| !id.is_empty() && id.len() <= 64)
            .unwrap_or_else(|| server_visitor_id.clone());

        // Extract UTM params from the URL query string as fallback.
        let url_utm = extract_utm_from_url(&p.url);

        // Build screen string: prefer combined "WxH" payload field,
        // fall back to screen_width + screen_height.
        let screen = p
            .screen
            .or_else(|| match (p.screen_width, p.screen_height) {
                (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                _ => None,
            });

        events.push(Event {
            id: uuid::Uuid::new_v4().to_string(),
            website_id,
            // tenant_id is always NULL in self-hosted mode (CLAUDE.md critical fact #2).
            tenant_id: None,
            // Session is resolved in the ingest worker right before persistence.
            session_id: AppState::pending_session_marker().to_string(),
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
            screen,
            language: p.language,
            // Explicit payload fields take precedence over URL-extracted params.
            utm_source: p.utm_source.or_else(|| url_utm.get("utm_source").cloned()),
            utm_medium: p.utm_medium.or_else(|| url_utm.get("utm_medium").cloned()),
            utm_campaign: p
                .utm_campaign
                .or_else(|| url_utm.get("utm_campaign").cloned()),
            utm_term: p.utm_term.or_else(|| url_utm.get("utm_term").cloned()),
            utm_content: p
                .utm_content
                .or_else(|| url_utm.get("utm_content").cloned()),
            // Preserve deterministic ordering for batched events so strict
            // funnel step sequencing (`created_at > prev.matched_at`) works.
            created_at: base_now + chrono::Duration::microseconds(idx as i64),
        });
    }

    state.enqueue_ingest_events(events).await?;

    let mut response = (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "ok": true })),
    )
        .into_response();
    response.headers_mut().insert(
        "x-sparklytics-ingest-ack",
        HeaderValue::from_static("queued"),
    );
    if let Ok(value) = HeaderValue::from_str(&state.queued_ingest_events().to_string()) {
        response
            .headers_mut()
            .insert("x-sparklytics-ingest-queue-events", value);
    }
    if let Ok(value) = HeaderValue::from_str(&state.ingest_queue_capacity().to_string()) {
        response
            .headers_mut()
            .insert("x-sparklytics-ingest-queue-capacity", value);
    }
    Ok(response)
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

    let lookup = reader.lookup(ip_addr).ok()?;
    let record = lookup.decode::<maxminddb::geoip2::City>().ok().flatten()?;

    let country = record.country.iso_code.map(|s| s.to_string());

    let region = record
        .subdivisions
        .first()
        .and_then(|sub| sub.names.english)
        .map(|s| s.to_string());

    let city = record.city.names.english.map(|s| s.to_string());

    Some(GeoInfo {
        country,
        region,
        city,
    })
}

/// Extract UTM parameters from the URL query string.
///
/// Returns a map of utm_source / utm_medium / utm_campaign / utm_term /
/// utm_content → value. Used as a fallback when the caller does not supply
/// explicit top-level utm_* fields in the payload.
fn extract_utm_from_url(url: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let query = match url.find('?') {
        Some(pos) => &url[pos + 1..],
        None => return map,
    };
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if !value.is_empty()
            && matches!(
                key,
                "utm_source" | "utm_medium" | "utm_campaign" | "utm_term" | "utm_content"
            )
        {
            // Minimal percent-decoding for '+' (common in form-encoded query strings).
            let decoded = value.replace('+', " ");
            map.insert(key.to_string(), decoded);
        }
    }
    map
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
