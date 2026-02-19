/// Security integration tests — Sprint 4.
///
/// Covers: SQL injection, oversized payload, malformed UTF-8, CORS behaviour,
/// website cache invalidation on delete, and rate limiting.
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::app::build_app;
use sparklytics_server::state::AppState;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

fn base_config() -> Config {
    Config {
        port: 0,
        data_dir: "/tmp/sparklytics-security-test".to_string(),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 65536,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: true, // disable for tests that send many requests
    }
}

fn config_with_cors(origins: Vec<String>) -> Config {
    Config {
        cors_origins: origins,
        ..base_config()
    }
}

async fn setup_with_config(config: Config) -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_sec", "example.com")
        .await
        .expect("seed website");
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

async fn setup() -> (Arc<AppState>, axum::Router) {
    setup_with_config(base_config()).await
}

fn collect_req(body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.1")
        .header("user-agent", "TestAgent/1.0")
        .body(Body::from(body.to_string()))
        .expect("build collect request")
}

async fn json_body(resp: axum::http::Response<Body>) -> Value {
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// Query the stored URL for the most recent event for a website.
async fn last_stored_url(state: &AppState, website_id: &str) -> Option<String> {
    state.flush_buffer().await;
    let db = &state.db;
    let conn = db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT url FROM events WHERE website_id = ?1 ORDER BY created_at DESC LIMIT 1")
        .ok()?;
    stmt.query_row(sparklytics_duckdb::duckdb::params![website_id], |row| {
        row.get::<_, String>(0)
    })
    .ok()
}

// ─────────────────────────────────────────────────────────────
// Feature: SQL injection via string fields
// ─────────────────────────────────────────────────────────────

/// Scenario: SQL injection via URL field — stored literally, table still exists.
#[tokio::test]
async fn test_sql_injection_url_stored_literally() {
    let (state, app) = setup().await;
    let malicious = "'; DROP TABLE events; --";
    let body = json!({
        "website_id": "site_sec",
        "type": "pageview",
        "url": malicious
    });
    let resp = app
        .oneshot(collect_req(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Table still exists and the URL is stored as the literal string.
    let stored = last_stored_url(&state, "site_sec").await;
    assert_eq!(stored.as_deref(), Some(malicious));
}

/// Scenario: SQL injection via event_name field — stored literally.
#[tokio::test]
async fn test_sql_injection_event_name_stored_literally() {
    let (state, app) = setup().await;
    let malicious = "' OR '1'='1"; // classic SQL injection probe
    let body = json!({
        "website_id": "site_sec",
        "type": "event",
        "url": "/checkout",
        "event_name": malicious
    });
    let resp = app
        .oneshot(collect_req(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify event_name was stored as the literal string.
    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare(
            "SELECT event_name FROM events WHERE website_id = ?1 AND event_name IS NOT NULL LIMIT 1",
        )
        .expect("prepare");
    let stored: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_sec"], |row| {
            row.get(0)
        })
        .ok();
    assert_eq!(stored.as_deref(), Some(malicious));
}

/// Scenario: SQL injection via referrer field — stored literally.
#[tokio::test]
async fn test_sql_injection_referrer_stored_literally() {
    let (state, app) = setup().await;
    let malicious = "https://evil.com/?q='; DELETE FROM sessions; --";
    let body = json!({
        "website_id": "site_sec",
        "type": "pageview",
        "url": "/page",
        "referrer": malicious
    });
    let resp = app
        .oneshot(collect_req(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare(
            "SELECT referrer_url FROM events WHERE website_id = ?1 AND referrer_url IS NOT NULL LIMIT 1",
        )
        .expect("prepare");
    let stored: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_sec"], |row| {
            row.get(0)
        })
        .ok();
    assert_eq!(stored.as_deref(), Some(malicious));
}

// ─────────────────────────────────────────────────────────────
// Feature: Payload size limits
// ─────────────────────────────────────────────────────────────

/// Scenario: Oversized event_data (>4KB) rejected with 400.
#[tokio::test]
async fn test_oversized_event_data_rejected() {
    let (_state, app) = setup().await;
    // Build event_data that exceeds 4KB when JSON-serialised.
    let big_value = "x".repeat(5000);
    let body = json!({
        "website_id": "site_sec",
        "type": "event",
        "url": "/checkout",
        "event_name": "purchase",
        "event_data": { "payload": big_value }
    });
    let resp = app
        .oneshot(collect_req(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let j = json_body(resp).await;
    assert_eq!(j["error"]["code"], "payload_too_large");
}

/// Scenario: Oversized batch (>100KB total body) rejected.
///
/// Axum's DefaultBodyLimit returns 413 Payload Too Large when the raw body
/// exceeds the configured limit. The server must not crash or store the data.
#[tokio::test]
async fn test_oversized_batch_rejected() {
    let (_state, app) = setup().await;
    // Build a single-event batch where the raw JSON body exceeds 100KB.
    let huge_url = "a".repeat(110_000);
    let body = json!([{
        "website_id": "site_sec",
        "type": "pageview",
        "url": huge_url
    }]);
    let req = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.2")
        .body(Body::from(body.to_string()))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("request");
    // DefaultBodyLimit returns 413; application-level checks return 400.
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::PAYLOAD_TOO_LARGE,
        "expected 400 or 413, got {}",
        resp.status()
    );
}

// ─────────────────────────────────────────────────────────────
// Feature: Malformed input handling
// ─────────────────────────────────────────────────────────────

/// Scenario: Malformed UTF-8 in the request body returns 400 (not a 500 crash).
#[tokio::test]
async fn test_malformed_utf8_body_returns_400() {
    let (_state, app) = setup().await;
    // Build a body with invalid UTF-8 bytes embedded in a JSON-like string.
    let mut body = b"{\"website_id\":\"site_sec\",\"type\":\"pageview\",\"url\":\"/p".to_vec();
    body.extend_from_slice(&[0xFF, 0xFE]); // invalid UTF-8
    body.extend_from_slice(b"\"}");

    let req = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.3")
        .body(Body::from(body))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("request");
    // Axum's JSON extractor rejects non-UTF-8 with 400 or 422; server must not panic.
    assert!(
        resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "expected 400/422, got {}",
        resp.status()
    );
}

// ─────────────────────────────────────────────────────────────
// Feature: CORS behaviour
// ─────────────────────────────────────────────────────────────

/// Scenario: /api/collect allows any origin (Access-Control-Allow-Origin: *).
#[tokio::test]
async fn test_cors_collect_allows_any_origin() {
    let (_state, app) =
        setup_with_config(config_with_cors(vec!["https://myapp.com".to_string()])).await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("origin", "https://any-website.com")
        .header("x-forwarded-for", "10.0.0.4")
        .body(Body::from(
            json!({
                "website_id": "site_sec",
                "type": "pageview",
                "url": "/page"
            })
            .to_string(),
        ))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(acao, "*", "collect must return ACAO: *");
}

/// Scenario: Analytics query endpoint (GET /api/websites/:id/stats) blocks
/// requests from origins not in SPARKLYTICS_CORS_ORIGINS.
#[tokio::test]
async fn test_cors_query_endpoint_blocks_unlisted_origin() {
    let (_state, app) =
        setup_with_config(config_with_cors(vec!["https://myapp.com".to_string()])).await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/websites/site_sec/stats")
        .header("origin", "https://evil.com")
        .body(Body::empty())
        .expect("build request");
    let resp = app.oneshot(req).await.expect("request");
    // The response must NOT include an ACAO header for this origin.
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_ne!(
        acao, "https://evil.com",
        "evil.com must not be in ACAO for query endpoint"
    );
    assert_ne!(acao, "*", "query endpoint must not return ACAO: *");
}

/// Scenario: Analytics query endpoint allows a listed origin.
#[tokio::test]
async fn test_cors_query_endpoint_allows_listed_origin() {
    let (_state, app) =
        setup_with_config(config_with_cors(vec!["https://myapp.com".to_string()])).await;
    let req = Request::builder()
        .method("GET")
        .uri("/api/websites/site_sec/stats")
        .header("origin", "https://myapp.com")
        .body(Body::empty())
        .expect("build request");
    let resp = app.oneshot(req).await.expect("request");
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        acao, "https://myapp.com",
        "listed origin must be reflected in ACAO"
    );
}

// ─────────────────────────────────────────────────────────────
// Feature: Website cache invalidated after delete
// ─────────────────────────────────────────────────────────────

/// Scenario: After DELETE /api/websites/:id, events for that site return 404.
#[tokio::test]
async fn test_website_cache_invalidated_after_delete() {
    let (_state, app) = setup().await;

    // First confirm the site is valid — event should be accepted.
    let resp = app
        .clone()
        .oneshot(collect_req(
            &json!({
                "website_id": "site_sec",
                "type": "pageview",
                "url": "/before-delete"
            })
            .to_string(),
        ))
        .await
        .expect("pre-delete collect");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Delete the website.
    let del_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/websites/site_sec")
                .body(Body::empty())
                .expect("delete request"),
        )
        .await
        .expect("delete response");
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    // Now collecting for the deleted site must return 404.
    let post_delete_resp = app
        .oneshot(collect_req(
            &json!({
                "website_id": "site_sec",
                "type": "pageview",
                "url": "/after-delete"
            })
            .to_string(),
        ))
        .await
        .expect("post-delete collect");
    assert_eq!(post_delete_resp.status(), StatusCode::NOT_FOUND);
}

// ─────────────────────────────────────────────────────────────
// Feature: Rate limiting
// ─────────────────────────────────────────────────────────────

/// Scenario: 60 requests within a minute succeed; the 61st is rejected with 429.
#[tokio::test]
async fn test_rate_limit_60_per_minute() {
    // Use rate_limit_disable: false for this specific test.
    let config = Config {
        rate_limit_disable: false,
        ..base_config()
    };
    let (_state, app) = setup_with_config(config).await;

    let body = json!({
        "website_id": "site_sec",
        "type": "pageview",
        "url": "/rl-test"
    })
    .to_string();

    // Requests 1–60 must all succeed.
    for i in 1..=60 {
        let req = Request::builder()
            .method("POST")
            .uri("/api/collect")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "99.0.0.1") // dedicated IP for this test
            .body(Body::from(body.clone()))
            .expect("build request");
        let resp = app.clone().oneshot(req).await.expect("request");
        assert_eq!(
            resp.status(),
            StatusCode::ACCEPTED,
            "request {} should be accepted",
            i
        );
    }

    // Request 61 must be rate-limited.
    let req = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "99.0.0.1")
        .body(Body::from(body))
        .expect("build request");
    let resp = app.oneshot(req).await.expect("61st request");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let j = json_body(resp).await;
    assert_eq!(j["error"]["code"], "rate_limited");
}
