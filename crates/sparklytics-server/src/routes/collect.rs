use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

use axum::{
    extract::{ConnectInfo, FromRequestParts, State},
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
    billing::BillingLimitReason,
    config::AppMode,
    event::{CollectOrBatch, CollectPayload, Event},
    visitor::{compute_visitor_id, extract_referrer_domain},
};
use sparklytics_metadata::Website;

use crate::{
    bot_detection::{classify_event, BotOverrideDecision, BotPolicyInput},
    error::AppError,
    state::AppState,
};

#[derive(Debug)]
pub struct MaybeConnectInfo(pub Option<SocketAddr>);

impl<S> FromRequestParts<S> for MaybeConnectInfo
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}

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
    maybe_connect_info: MaybeConnectInfo,
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

    // --- Validation: all website_ids must be known ---
    // Validate unique IDs only to avoid repeated DB lookups for batches.
    let unique_website_ids: HashSet<String> =
        payloads.iter().map(|p| p.website_id.clone()).collect();
    let mut websites_by_id: HashMap<String, Website> = HashMap::new();
    for website_id in unique_website_ids {
        let website = state
            .get_website_metadata_cached(&website_id)
            .await
            .map_err(AppError::Internal)?;
        let Some(website) = website else {
            return Err(AppError::NotFound(format!(
                "Unknown website_id: {}",
                website_id
            )));
        };
        websites_by_id.insert(website_id, website);
    }

    // --- Resolve cloud tenant context from website ownership ---
    let cloud_batch_tenant_id = if state.config.mode == AppMode::Cloud {
        let mut resolved_tenant_id: Option<&str> = None;
        for website in websites_by_id.values() {
            let Some(tenant_id) = website.tenant_id.as_deref() else {
                return Err(AppError::OrganizationContextRequired);
            };
            match resolved_tenant_id {
                Some(existing) if existing != tenant_id => {
                    return Err(AppError::BadRequest(
                        "batch must contain events for a single tenant".to_string(),
                    ));
                }
                None => resolved_tenant_id = Some(tenant_id),
                _ => {}
            }
        }
        Some(
            resolved_tenant_id
                .ok_or(AppError::OrganizationContextRequired)?
                .to_string(),
        )
    } else {
        None
    };

    // --- Extract client IP ---
    let client_ip = extract_client_ip(&headers, maybe_connect_info.0);

    // --- Rate limiting: 60 req/min per IP ---
    // SPARKLYTICS_RATE_LIMIT_DISABLE bypasses this for load testing only.
    if !state.config.rate_limit_disable && !state.check_rate_limit(&client_ip).await {
        return Err(AppError::RateLimited);
    }

    // --- Apply cloud plan admission limits ---
    let mut payloads = payloads;
    let mut dropped_monthly_limit = 0usize;
    let mut dropped_peak_rate = 0usize;
    if let Some(ref tenant_id) = cloud_batch_tenant_id {
        let admission = state
            .billing_gate
            .admit_events(tenant_id, payloads.len())
            .await;
        if admission.allowed_events < payloads.len() {
            let dropped = payloads.len().saturating_sub(admission.allowed_events);
            payloads.truncate(admission.allowed_events);
            match admission.reason {
                Some(BillingLimitReason::MonthlyLimit) => {
                    dropped_monthly_limit = dropped_monthly_limit.saturating_add(dropped);
                }
                Some(BillingLimitReason::PeakRate) | None => {
                    dropped_peak_rate = dropped_peak_rate.saturating_add(dropped);
                }
            }
        }
    }

    // --- Apply self-hosted per-website peak rate admission ---
    if state.config.mode == AppMode::SelfHosted {
        let mut requested_by_website: HashMap<String, usize> = HashMap::new();
        for payload in &payloads {
            let next = requested_by_website
                .get(&payload.website_id)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            requested_by_website.insert(payload.website_id.clone(), next);
        }

        let mut allowed_by_website: HashMap<String, usize> = HashMap::new();
        for (website_id, requested) in requested_by_website {
            let peak_eps = websites_by_id
                .get(&website_id)
                .and_then(|website| website.ingest_peak_eps)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(state.website_ingest_peak_eps_default());
            let max_per_min = peak_eps.saturating_mul(60);
            let admitted = state
                .admit_events_rate_limit_with_max(
                    &format!("website:{website_id}"),
                    requested,
                    max_per_min,
                )
                .await;
            allowed_by_website.insert(website_id, admitted);
        }

        let mut admitted_payloads = Vec::with_capacity(payloads.len());
        for payload in payloads {
            let remaining = allowed_by_website
                .entry(payload.website_id.clone())
                .or_insert(0);
            if *remaining > 0 {
                *remaining = remaining.saturating_sub(1);
                admitted_payloads.push(payload);
            } else {
                dropped_peak_rate = dropped_peak_rate.saturating_add(1);
            }
        }
        payloads = admitted_payloads;
    }

    if payloads.is_empty() {
        let mut reasons = Vec::new();
        if dropped_monthly_limit > 0 {
            reasons.push("monthly_limit");
        }
        if dropped_peak_rate > 0 {
            reasons.push("peak_rate");
        }
        return Ok(build_collect_response(
            &state,
            0,
            dropped_monthly_limit.saturating_add(dropped_peak_rate),
            &reasons,
        ));
    }

    // --- Extract User-Agent header ---
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let has_accept_header = headers.get(axum::http::header::ACCEPT).is_some();
    let has_accept_language_header = headers.get(axum::http::header::ACCEPT_LANGUAGE).is_some();

    // --- GeoIP lookup ---
    // Load the GeoIP database from the path configured at startup.
    // If the file is absent, geo fields are left as None (non-fatal, per Sprint 0).
    let geo = lookup_geo(&state.config.geoip_path, &client_ip);

    // --- UA parsing ---
    let ua_info = parse_user_agent_cached(&user_agent);

    // --- Build enriched Event structs ---
    let mut events: Vec<Event> = Vec::with_capacity(payloads.len());
    let base_now = Utc::now();
    let server_visitor_id = compute_visitor_id_cached(&client_ip, &user_agent);
    let mut website_bot_policies: HashMap<String, BotPolicyInput> = HashMap::new();
    let mut website_bot_overrides: HashMap<String, Option<BotOverrideDecision>> = HashMap::new();

    for (idx, p) in payloads.into_iter().enumerate() {
        let website_id = p.website_id.clone();
        let referrer_domain = p.referrer.as_deref().and_then(extract_referrer_domain);

        // Use client-supplied visitor_id when present (max 64 chars),
        // otherwise fall back to the server-computed hash.
        let visitor_id = p
            .visitor_id
            .filter(|id| !id.is_empty() && id.len() <= 64)
            .unwrap_or_else(|| server_visitor_id.clone());

        let bot_policy = if let Some(policy) = website_bot_policies.get(&website_id) {
            policy.clone()
        } else {
            let policy = state
                .get_bot_policy_cached(&website_id)
                .await
                .map_err(AppError::Internal)?;
            website_bot_policies.insert(website_id.clone(), policy.clone());
            policy
        };
        let override_decision = if let Some(decision) = website_bot_overrides.get(&website_id) {
            decision.clone()
        } else {
            let decision = state
                .classify_override_for_request_cached(&website_id, &client_ip, &user_agent)
                .await
                .map_err(AppError::Internal)?;
            website_bot_overrides.insert(website_id.clone(), decision.clone());
            decision
        };
        let bot_classification = classify_event(
            &website_id,
            &visitor_id,
            &p.url,
            &user_agent,
            has_accept_header,
            has_accept_language_header,
            &bot_policy,
            override_decision,
        );

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
            tenant_id: websites_by_id
                .get(&p.website_id)
                .and_then(|website| website.tenant_id.clone()),
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
            link_id: None,
            pixel_id: None,
            source_ip: Some(client_ip.clone()),
            user_agent: Some(user_agent.clone()),
            is_bot: bot_classification.is_bot,
            bot_score: bot_classification.bot_score,
            bot_reason: bot_classification.bot_reason,
            // Preserve deterministic ordering for batched events so strict
            // funnel step sequencing (`created_at > prev.matched_at`) works.
            created_at: base_now + chrono::Duration::microseconds(idx as i64),
        });
    }

    let website_queue_caps = if state.config.mode == AppMode::SelfHosted {
        let mut caps = HashMap::new();
        for (website_id, website) in &websites_by_id {
            let cap = website
                .ingest_queue_max_events
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(state.website_ingest_queue_max_events_default());
            caps.insert(website_id.clone(), cap);
        }
        Some(caps)
    } else {
        None
    };

    let enqueue_outcome = state
        .enqueue_ingest_events_with_limits(events, website_queue_caps.as_ref())
        .await?;

    let mut reasons = Vec::new();
    if dropped_monthly_limit > 0 {
        reasons.push("monthly_limit");
    }
    if dropped_peak_rate > 0 {
        reasons.push("peak_rate");
    }
    if enqueue_outcome.dropped_events > 0 {
        reasons.push("queue_overflow");
    }

    Ok(build_collect_response(
        &state,
        enqueue_outcome.accepted_events,
        dropped_monthly_limit
            .saturating_add(dropped_peak_rate)
            .saturating_add(enqueue_outcome.dropped_events),
        &reasons,
    ))
}

fn build_collect_response(
    state: &Arc<AppState>,
    accepted_events: usize,
    dropped_events: usize,
    drop_reasons: &[&str],
) -> Response {
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
    if let Ok(value) = HeaderValue::from_str(&accepted_events.to_string()) {
        response
            .headers_mut()
            .insert("x-sparklytics-ingest-accepted-events", value);
    }
    if let Ok(value) = HeaderValue::from_str(&dropped_events.to_string()) {
        response
            .headers_mut()
            .insert("x-sparklytics-ingest-dropped-events", value);
    }
    if !drop_reasons.is_empty() {
        let reason_value = drop_reasons.join(",");
        if let Ok(value) = HeaderValue::from_str(&reason_value) {
            response
                .headers_mut()
                .insert("x-sparklytics-ingest-drop-reason", value);
        }
    }

    response
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract client IP.
///
/// Prefer the direct socket address when available. `X-Forwarded-For` is only
/// used as fallback when socket metadata is unavailable.
pub(crate) fn extract_client_ip(headers: &HeaderMap, remote_addr: Option<SocketAddr>) -> String {
    let forwarded_ip = parse_forwarded_ip(headers);
    if let Some(addr) = remote_addr {
        let remote_ip = addr.ip();
        if trusted_proxy_cidrs()
            .iter()
            .any(|cidr| cidr.contains(&remote_ip))
        {
            return forwarded_ip.unwrap_or(remote_ip).to_string();
        }
        return remote_ip.to_string();
    }

    forwarded_ip
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn parse_forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
}

fn trusted_proxy_cidrs() -> &'static Vec<ipnet::IpNet> {
    static TRUSTED: OnceLock<Vec<ipnet::IpNet>> = OnceLock::new();
    TRUSTED.get_or_init(|| {
        std::env::var("SPARKLYTICS_TRUSTED_PROXIES")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .filter_map(|entry| entry.trim().parse::<ipnet::IpNet>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
}

/// GeoIP result from a MaxMind lookup.
pub(crate) struct GeoInfo {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

/// Attempt a GeoIP lookup for `ip` using the MaxMind database at `path`.
///
/// Returns `None` if the database file is missing or the IP cannot be parsed.
/// This is the Sprint 0 non-fatal behaviour: events are stored with NULL geo
/// fields rather than panicking.
pub(crate) fn lookup_geo(path: &str, ip: &str) -> Option<GeoInfo> {
    use std::net::IpAddr;
    use std::str::FromStr;

    type GeoReader = maxminddb::Reader<Vec<u8>>;
    static GEOIP_READER: OnceLock<Option<GeoReader>> = OnceLock::new();
    let reader = GEOIP_READER.get_or_init(|| {
        if !std::path::Path::new(path).exists() {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        maxminddb::Reader::from_source(bytes).ok()
    });
    let reader = reader.as_ref()?;
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
pub(crate) fn extract_utm_from_url(url: &str) -> HashMap<String, String> {
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
#[derive(Debug, Clone)]
pub(crate) struct UaInfo {
    pub browser: String,
    pub browser_version: Option<String>,
    pub os: String,
    pub os_version: Option<String>,
    pub device_type: String,
}

#[derive(Debug, Clone)]
struct CachedUaInfo {
    value: Option<UaInfo>,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedVisitorId {
    value: String,
    salt_epoch: i64,
}

/// Parse a `User-Agent` string via the `woothee` crate.
///
/// Returns `None` if the UA string is empty or `woothee` cannot classify it.
pub(crate) fn parse_user_agent(user_agent: &str) -> Option<UaInfo> {
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

/// Parse User-Agent with a small process-local cache to reduce repeated woothee work.
fn parse_user_agent_cached(user_agent: &str) -> Option<UaInfo> {
    const UA_CACHE_MAX_ENTRIES: usize = 2048;
    const UA_CACHE_TTL_SECONDS: u64 = 900;

    if user_agent.is_empty() {
        return None;
    }

    static UA_CACHE: OnceLock<RwLock<HashMap<String, CachedUaInfo>>> = OnceLock::new();
    let cache = UA_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let now = Instant::now();

    if let Ok(map) = cache.read() {
        if let Some(entry) = map.get(user_agent) {
            if entry.expires_at > now {
                return entry.value.clone();
            }
        }
    }
    if let Ok(mut map) = cache.write() {
        if let Some(entry) = map.get(user_agent) {
            if entry.expires_at > now {
                return entry.value.clone();
            }
            map.remove(user_agent);
        }
    }

    let parsed = parse_user_agent(user_agent);

    if let Ok(mut map) = cache.write() {
        if map.len() >= UA_CACHE_MAX_ENTRIES {
            map.retain(|_, entry| entry.expires_at > now);
            if map.len() >= UA_CACHE_MAX_ENTRIES {
                map.clear();
            }
        }
        map.insert(
            user_agent.to_string(),
            CachedUaInfo {
                value: parsed.clone(),
                expires_at: now + Duration::from_secs(UA_CACHE_TTL_SECONDS),
            },
        );
    }

    parsed
}

/// Compute visitor ID with a small process-local cache keyed by ip+ua and salt epoch.
fn compute_visitor_id_cached(ip: &str, user_agent: &str) -> String {
    const VISITOR_ID_CACHE_MAX_IPS: usize = 4096;
    const VISITOR_ID_CACHE_MAX_UA_PER_IP: usize = 8;

    static VISITOR_ID_CACHE: OnceLock<RwLock<HashMap<String, HashMap<String, CachedVisitorId>>>> =
        OnceLock::new();
    let cache = VISITOR_ID_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let salt_epoch = chrono::Utc::now().timestamp() / 86400;

    if let Ok(map) = cache.read() {
        if let Some(entry) = map.get(ip).and_then(|ua_map| ua_map.get(user_agent)) {
            if entry.salt_epoch == salt_epoch {
                return entry.value.clone();
            }
        }
    }

    let visitor_id = compute_visitor_id(ip, user_agent);

    if let Ok(mut map) = cache.write() {
        if map.len() >= VISITOR_ID_CACHE_MAX_IPS {
            map.retain(|_, ua_map| ua_map.values().any(|entry| entry.salt_epoch == salt_epoch));
            if map.len() >= VISITOR_ID_CACHE_MAX_IPS {
                map.clear();
            }
        }

        let ua_map = map.entry(ip.to_string()).or_default();
        if ua_map.len() >= VISITOR_ID_CACHE_MAX_UA_PER_IP {
            ua_map.retain(|_, entry| entry.salt_epoch == salt_epoch);
            if ua_map.len() >= VISITOR_ID_CACHE_MAX_UA_PER_IP {
                ua_map.clear();
            }
        }

        ua_map.insert(
            user_agent.to_string(),
            CachedVisitorId {
                value: visitor_id.clone(),
                salt_epoch,
            },
        );
    }

    visitor_id
}
