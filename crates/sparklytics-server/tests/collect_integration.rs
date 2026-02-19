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

/// Build a test Config with sensible defaults for integration tests.
fn test_config() -> Config {
    Config {
        port: 0,
        data_dir: "/tmp/sparklytics-test".to_string(),
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
    }
}

/// Create a fresh in-memory backend + state + app for each test.
async fn setup() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_test", "example.com")
        .await
        .expect("seed website");
    let config = test_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

/// Helper: send a POST /api/collect with the given JSON body and optional headers.
fn collect_request(body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "1.2.3.4")
        .header("user-agent", "Mozilla/5.0 Chrome/120")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

/// Helper: extract JSON body from response.
async fn json_body(response: axum::http::Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.expect("read body").to_bytes();
    serde_json::from_slice(&bytes).expect("parse JSON")
}

/// Helper: query event count from DuckDB for a given website_id.
async fn event_count(state: &AppState, website_id: &str) -> i64 {
    // Flush the buffer first to ensure events are written.
    state.flush_buffer().await;
    let db = &state.db;
    let conn = db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1")
        .expect("prepare count query");
    stmt.query_row(sparklytics_duckdb::duckdb::params![website_id], |row| row.get(0))
        .expect("count events")
}

// ============================================================
// BDD: Collect a valid pageview
// ============================================================
#[tokio::test]
async fn test_collect_valid_pageview() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/home",
        "referrer": "https://google.com",
        "language": "en-US"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let json = json_body(response).await;
    assert_eq!(json, json!({ "ok": true }));

    // Verify event is persisted after flush.
    let count = event_count(&state, "site_test").await;
    assert_eq!(count, 1);
}

// ============================================================
// BDD: Collect a batch of events
// ============================================================
#[tokio::test]
async fn test_collect_batch_of_three_events() {
    let (state, app) = setup().await;

    let body = json!([
        { "website_id": "site_test", "type": "pageview", "url": "/page1" },
        { "website_id": "site_test", "type": "pageview", "url": "/page2" },
        { "website_id": "site_test", "type": "pageview", "url": "/page3" }
    ]);

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let count = event_count(&state, "site_test").await;
    assert_eq!(count, 3);
}

// ============================================================
// BDD: Batch of 51 events is rejected
// ============================================================
#[tokio::test]
async fn test_collect_batch_too_large() {
    let (_state, app) = setup().await;

    let events: Vec<Value> = (0..51)
        .map(|i| {
            json!({
                "website_id": "site_test",
                "type": "pageview",
                "url": format!("/page{}", i)
            })
        })
        .collect();
    let body = serde_json::to_string(&events).expect("serialize");

    let response = app
        .oneshot(collect_request(&body))
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "batch_too_large");
}

// ============================================================
// BDD: Reject unknown website_id
// ============================================================
#[tokio::test]
async fn test_collect_unknown_website_id() {
    let (_state, app) = setup().await;

    let body = json!({
        "website_id": "site_unknown",
        "type": "pageview",
        "url": "/home"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "not_found");
}

// ============================================================
// BDD: Reject malformed payload
// ============================================================
#[tokio::test]
async fn test_collect_malformed_payload() {
    let (_state, app) = setup().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "1.2.3.4")
        .body(Body::from("not json"))
        .expect("build request");

    let response = app.oneshot(request).await.expect("request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================
// BDD: Rate limit enforcement
// ============================================================
#[tokio::test]
async fn test_rate_limit_enforcement() {
    let (_state, app) = setup().await;

    // We need to reuse the same app state for all 61 requests.
    // Since oneshot consumes the router, we need to clone it.
    // Axum Router implements Clone.
    let mut last_status = StatusCode::OK;
    for i in 0..61 {
        let body = json!({
            "website_id": "site_test",
            "type": "pageview",
            "url": format!("/page{}", i)
        });

        let response = app
            .clone()
            .oneshot(collect_request(&body.to_string()))
            .await
            .expect("request");

        last_status = response.status();

        // First 60 should be 202; the 61st should be 429.
        if i < 60 {
            assert_eq!(
                last_status,
                StatusCode::ACCEPTED,
                "request {} should be accepted",
                i + 1
            );
        }
    }

    assert_eq!(last_status, StatusCode::TOO_MANY_REQUESTS);
}

// ============================================================
// BDD: Visitor ID is deterministic within same day
// ============================================================
#[tokio::test]
async fn test_visitor_id_deterministic_within_day() {
    let (state, app) = setup().await;

    // Send two events from the same IP and UA.
    let body = json!([
        { "website_id": "site_test", "type": "pageview", "url": "/page1" },
        { "website_id": "site_test", "type": "pageview", "url": "/page2" }
    ]);

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    // Read both visitor_ids from DuckDB.
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT visitor_id FROM events WHERE website_id = ?1 ORDER BY url")
        .expect("prepare");
    let visitor_ids: Vec<String> = stmt
        .query_map(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    assert_eq!(visitor_ids.len(), 2);
    assert_eq!(visitor_ids[0], visitor_ids[1], "same IP+UA on same day must produce same visitor_id");

    // Verify 16 hex chars.
    let vid = &visitor_ids[0];
    assert_eq!(vid.len(), 16, "visitor_id must be 16 hex chars");
    assert!(
        vid.chars().all(|c| c.is_ascii_hexdigit()),
        "visitor_id must be hex only"
    );
}

// ============================================================
// BDD: UTM params extracted at ingestion
// ============================================================
#[tokio::test]
async fn test_utm_params_extracted_from_url() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/pricing?utm_source=twitter&utm_medium=social"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT utm_source, utm_medium FROM events WHERE website_id = ?1")
        .expect("prepare");
    let (source, medium): (Option<String>, Option<String>) = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("query");

    assert_eq!(source.as_deref(), Some("twitter"));
    assert_eq!(medium.as_deref(), Some("social"));
}

// ============================================================
// BDD: Referrer domain extracted correctly
// ============================================================
#[tokio::test]
async fn test_referrer_domain_extracted() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/home",
        "referrer": "https://news.ycombinator.com/item?id=12345"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT referrer_domain FROM events WHERE website_id = ?1")
        .expect("prepare");
    let domain: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("query");

    assert_eq!(domain.as_deref(), Some("news.ycombinator.com"));
}

// ============================================================
// BDD: Buffer flushes on threshold (100 events)
// ============================================================
#[tokio::test]
async fn test_buffer_flush_on_threshold() {
    // Use a config with buffer_max_size = 100.
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_test", "example.com")
        .await
        .expect("seed website");
    let config = test_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));

    // Send 10 batches of 10 events = 100 total, which should trigger auto-flush.
    for batch_num in 0..10 {
        let events: Vec<Value> = (0..10)
            .map(|i| {
                json!({
                    "website_id": "site_test",
                    "type": "pageview",
                    "url": format!("/page{}_{}", batch_num, i)
                })
            })
            .collect();
        let body = serde_json::to_string(&events).expect("serialize");

        let response = app
            .clone()
            .oneshot(collect_request(&body))
            .await
            .expect("request");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // The buffer should have auto-flushed at 100 events (without calling flush_buffer).
    // Verify all 100 events are in DuckDB.
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1")
        .expect("prepare");
    let count: i64 = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("count");

    assert_eq!(count, 100, "all 100 events should be flushed to DuckDB");
}

// ============================================================
// BDD: Schema initialization on first run
// ============================================================
#[tokio::test]
async fn test_schema_initialization_on_first_run() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");

    // Settings should be populated with default values.
    let salt = db.get_daily_salt().await.expect("get salt");
    assert_eq!(salt.len(), 64, "daily_salt should be 32-byte hex (64 chars)");
    assert!(
        salt.chars().all(|c| c.is_ascii_hexdigit()),
        "daily_salt should be hex"
    );

    // Verify the version key exists.
    let conn = db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = 'version'")
        .expect("prepare");
    let version: String = stmt.query_row([], |row| row.get(0)).expect("query");
    assert_eq!(version, "1");

    // Verify install_id exists and is hex.
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = 'install_id'")
        .expect("prepare");
    let install_id: String = stmt.query_row([], |row| row.get(0)).expect("query");
    assert_eq!(install_id.len(), 16, "install_id should be 8-byte hex (16 chars)");
}

// ============================================================
// BDD: seed_website helper
// ============================================================
#[tokio::test]
async fn test_seed_website_helper() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_foo", "foo.com")
        .await
        .expect("seed");

    let exists = db.website_exists("site_foo").await.expect("check");
    assert!(exists);

    let not_exists = db.website_exists("site_bar").await.expect("check");
    assert!(!not_exists);
}

// ============================================================
// BDD: session_id is populated by session management (Sprint 1)
// ============================================================
#[tokio::test]
async fn test_session_id_is_populated() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/home"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT session_id FROM events WHERE website_id = ?1")
        .expect("prepare");
    let session_id: String = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("query");

    assert!(!session_id.is_empty(), "session_id should be populated by session management");
    assert_eq!(session_id.len(), 16, "session_id should be 16 hex chars");
}

// ============================================================
// BDD: event_data serialized to JSON string
// ============================================================
#[tokio::test]
async fn test_event_data_serialized_to_json_string() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "event",
        "url": "/checkout",
        "event_name": "purchase",
        "event_data": { "plan": "pro", "value": 49.99 }
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT event_data FROM events WHERE website_id = ?1")
        .expect("prepare");
    let data: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("query");

    let data = data.expect("event_data should not be NULL");
    let parsed: Value = serde_json::from_str(&data).expect("event_data should be valid JSON");
    assert_eq!(parsed["plan"], "pro");
}

// ============================================================
// BDD: tenant_id is always NULL in self-hosted mode
// ============================================================
#[tokio::test]
async fn test_tenant_id_null_in_self_hosted() {
    let (state, app) = setup().await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/home"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT tenant_id FROM events WHERE website_id = ?1")
        .expect("prepare");
    let tenant_id: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("query");

    assert!(tenant_id.is_none(), "tenant_id must be NULL in self-hosted mode");
}
