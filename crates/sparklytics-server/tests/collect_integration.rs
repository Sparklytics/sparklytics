use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration, Instant};
use tower::ServiceExt;

use async_trait::async_trait;
use sparklytics_core::billing::{BillingAdmission, BillingGate, BillingLimitReason};
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
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
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

struct MockBillingGate {
    admission: BillingAdmission,
    seen_tenants: Arc<StdMutex<Vec<String>>>,
}

#[async_trait]
impl BillingGate for MockBillingGate {
    async fn admit_events(&self, tenant_id: &str, requested_events: usize) -> BillingAdmission {
        let mut seen = self.seen_tenants.lock().expect("lock seen tenants");
        seen.push(tenant_id.to_string());

        let mut admission = self.admission.clone();
        if admission.reason.is_none() {
            admission.allowed_events = requested_events;
        } else {
            admission.allowed_events = admission.allowed_events.min(requested_events);
        }
        admission
    }
}

async fn setup_cloud(
    website_tenant_id: Option<&str>,
    billing_admission: BillingAdmission,
) -> (Arc<AppState>, axum::Router, Arc<StdMutex<Vec<String>>>) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_test", "example.com")
        .await
        .expect("seed website");
    if let Some(tenant_id) = website_tenant_id {
        let conn = db.conn_for_test().await;
        conn.execute(
            "UPDATE websites SET tenant_id = ?1 WHERE id = ?2",
            sparklytics_duckdb::duckdb::params![tenant_id, "site_test"],
        )
        .expect("assign tenant to website");
    }
    let mut config = test_config();
    config.mode = AppMode::Cloud;

    let seen_tenants = Arc::new(StdMutex::new(Vec::new()));
    let billing_gate = Arc::new(MockBillingGate {
        admission: billing_admission,
        seen_tenants: Arc::clone(&seen_tenants),
    });
    let mut state = AppState::new(db, config);
    state.billing_gate = billing_gate;
    let state = Arc::new(state);
    let app = build_app(Arc::clone(&state));
    (state, app, seen_tenants)
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
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
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
    stmt.query_row(sparklytics_duckdb::duckdb::params![website_id], |row| {
        row.get(0)
    })
    .expect("count events")
}

fn header_usize(response: &axum::http::Response<Body>, name: &str) -> usize {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .expect("numeric ingest header")
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

#[tokio::test]
async fn test_collect_sets_ingest_ack_header() {
    let (_state, app) = setup().await;

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
    assert_eq!(
        response
            .headers()
            .get("x-sparklytics-ingest-ack")
            .and_then(|v| v.to_str().ok()),
        Some("queued")
    );
    let queued = response
        .headers()
        .get("x-sparklytics-ingest-queue-events")
        .and_then(|v| v.to_str().ok())
        .expect("queue events header");
    let _queued: usize = queued.parse().expect("queue events should be number");
    let capacity = response
        .headers()
        .get("x-sparklytics-ingest-queue-capacity")
        .and_then(|v| v.to_str().ok())
        .expect("queue capacity header");
    let parsed_capacity: usize = capacity.parse().expect("queue capacity should be number");
    assert!(parsed_capacity > 0, "queue capacity should be positive");
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

#[tokio::test]
async fn test_collect_batch_timestamps_are_strictly_increasing() {
    let (state, app) = setup().await;

    let body = json!([
        { "website_id": "site_test", "type": "pageview", "url": "/a" },
        { "website_id": "site_test", "type": "event", "url": "/a", "event_name": "step_2" },
        { "website_id": "site_test", "type": "event", "url": "/a", "event_name": "step_3" }
    ]);

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare(
            "SELECT epoch_us(created_at) FROM events WHERE website_id = ?1 ORDER BY created_at",
        )
        .expect("prepare query");
    let timestamps: Vec<i64> = stmt
        .query_map(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    assert_eq!(timestamps.len(), 3);
    assert!(timestamps[0] < timestamps[1]);
    assert!(timestamps[1] < timestamps[2]);
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

    let response = app.oneshot(collect_request(&body)).await.expect("request");

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
        .query_map(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");

    assert_eq!(visitor_ids.len(), 2);
    assert_eq!(
        visitor_ids[0], visitor_ids[1],
        "same IP+UA on same day must produce same visitor_id"
    );

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
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
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

    // Threshold flush now runs in a background task; wait briefly for persistence.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let conn = state.db.conn_for_test().await;
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1")
            .expect("prepare");
        let count: i64 = stmt
            .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
                row.get(0)
            })
            .expect("count");
        drop(stmt);
        drop(conn);

        if count == 100 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "all 100 events should be flushed to DuckDB (count={count})"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

// ============================================================
// BDD: Schema initialization on first run
// ============================================================
#[tokio::test]
async fn test_schema_initialization_on_first_run() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");

    // Settings should be populated with default values.
    let salt = db.get_daily_salt().await.expect("get salt");
    assert_eq!(
        salt.len(),
        64,
        "daily_salt should be 32-byte hex (64 chars)"
    );
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
    assert_eq!(
        install_id.len(),
        16,
        "install_id should be 8-byte hex (16 chars)"
    );
}

// ============================================================
// BDD: seed_website helper
// ============================================================
#[tokio::test]
async fn test_seed_website_helper() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_foo", "foo.com").await.expect("seed");

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
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query");

    assert!(
        !session_id.is_empty(),
        "session_id should be populated by session management"
    );
    assert_eq!(session_id.len(), 16, "session_id should be 16 hex chars");
}

// ============================================================
// BDD: batch collect keeps sessions.pageview_count in sync
// ============================================================
#[tokio::test]
async fn test_batch_collect_updates_session_pageview_count() {
    let (state, app) = setup().await;

    let body = json!([
        { "website_id": "site_test", "type": "pageview", "url": "/a" },
        { "website_id": "site_test", "type": "pageview", "url": "/b" },
        { "website_id": "site_test", "type": "pageview", "url": "/c" }
    ]);

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    state.flush_buffer().await;

    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT pageview_count FROM sessions WHERE website_id = ?1 LIMIT 1")
        .expect("prepare");
    let pageview_count: i64 = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query");

    assert_eq!(pageview_count, 3);
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
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
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
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query");

    assert!(
        tenant_id.is_none(),
        "tenant_id must be NULL in self-hosted mode"
    );
}

// ============================================================
// BDD: cloud collect resolves tenant from website and checks billing gate
// ============================================================
#[tokio::test]
async fn test_cloud_collect_assigns_tenant_and_checks_billing() {
    let (state, app, seen_tenants) =
        setup_cloud(Some("org_acme"), BillingAdmission::allow_all(1)).await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/cloud"
    });

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let seen = seen_tenants.lock().expect("lock seen tenants").clone();
    assert_eq!(seen, vec!["org_acme".to_string()]);

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT tenant_id FROM events WHERE website_id = ?1")
        .expect("prepare query");
    let tenant_id: Option<String> = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            row.get(0)
        })
        .expect("query tenant_id");
    assert_eq!(tenant_id.as_deref(), Some("org_acme"));
}

// ============================================================
// BDD: cloud collect rejects websites without tenant context
// ============================================================
#[tokio::test]
async fn test_cloud_collect_rejects_missing_website_tenant() {
    let (_state, app, seen_tenants) = setup_cloud(None, BillingAdmission::allow_all(1)).await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/cloud"
    });
    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "organization_context_required");
    assert!(
        seen_tenants.lock().expect("lock seen tenants").is_empty(),
        "billing gate must not run when tenant context is missing"
    );
}

// ============================================================
// BDD: cloud collect enforces billing plan limit
// ============================================================
#[tokio::test]
async fn test_cloud_collect_plan_limit_exceeded() {
    let (_state, app, seen_tenants) = setup_cloud(
        Some("org_acme"),
        BillingAdmission::limited(0, BillingLimitReason::MonthlyLimit),
    )
    .await;

    let body = json!({
        "website_id": "site_test",
        "type": "pageview",
        "url": "/cloud"
    });
    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-sparklytics-ingest-accepted-events")
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );
    assert_eq!(
        response
            .headers()
            .get("x-sparklytics-ingest-dropped-events")
            .and_then(|value| value.to_str().ok()),
        Some("1")
    );
    assert_eq!(
        response
            .headers()
            .get("x-sparklytics-ingest-drop-reason")
            .and_then(|value| value.to_str().ok()),
        Some("monthly_limit")
    );
    let json = json_body(response).await;
    assert_eq!(json, json!({ "ok": true }));
    let seen = seen_tenants.lock().expect("lock seen tenants").clone();
    assert_eq!(seen, vec!["org_acme".to_string()]);
}

// ============================================================
// BDD: cloud collect rejects mixed-tenant batches
// ============================================================
#[tokio::test]
async fn test_cloud_collect_rejects_mixed_tenant_batch() {
    let (state, app, seen_tenants) =
        setup_cloud(Some("org_acme"), BillingAdmission::allow_all(1)).await;
    state
        .db
        .seed_website("site_other", "other.example.com")
        .await
        .expect("seed second website");
    {
        let conn = state.db.conn_for_test().await;
        conn.execute(
            "UPDATE websites SET tenant_id = ?1 WHERE id = ?2",
            sparklytics_duckdb::duckdb::params!["org_other", "site_other"],
        )
        .expect("set second tenant");
    }

    let body = json!([
        {
            "website_id": "site_test",
            "type": "pageview",
            "url": "/a"
        },
        {
            "website_id": "site_other",
            "type": "pageview",
            "url": "/b"
        }
    ]);
    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "validation_error");
    assert_eq!(
        json["error"]["message"],
        "batch must contain events for a single tenant"
    );
    assert!(
        seen_tenants.lock().expect("lock seen tenants").is_empty(),
        "billing gate must not run for invalid mixed-tenant batches"
    );
}

#[tokio::test]
async fn test_collect_self_hosted_peak_rate_soft_drop_headers() {
    let (state, app) = setup().await;
    {
        let conn = state.db.conn_for_test().await;
        conn.execute(
            "UPDATE websites SET ingest_peak_eps = ?1 WHERE id = ?2",
            sparklytics_duckdb::duckdb::params![1i64, "site_test"],
        )
        .expect("set custom peak eps");
    }

    let first_events: Vec<Value> = (0..50)
        .map(|i| {
            json!({
                "website_id": "site_test",
                "type": "pageview",
                "url": format!("/first/{i}")
            })
        })
        .collect();
    let first = app
        .clone()
        .oneshot(collect_request(
            &serde_json::to_string(&first_events).expect("serialize first"),
        ))
        .await
        .expect("first request");
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    assert_eq!(
        first
            .headers()
            .get("x-sparklytics-ingest-drop-reason")
            .and_then(|v| v.to_str().ok()),
        None
    );

    let second_events: Vec<Value> = (0..50)
        .map(|i| {
            json!({
                "website_id": "site_test",
                "type": "pageview",
                "url": format!("/second/{i}")
            })
        })
        .collect();
    let second = app
        .oneshot(collect_request(
            &serde_json::to_string(&second_events).expect("serialize second"),
        ))
        .await
        .expect("second request");
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    assert_eq!(
        second
            .headers()
            .get("x-sparklytics-ingest-drop-reason")
            .and_then(|v| v.to_str().ok()),
        Some("peak_rate")
    );
    assert_eq!(
        header_usize(&second, "x-sparklytics-ingest-accepted-events"),
        10
    );
    assert_eq!(
        header_usize(&second, "x-sparklytics-ingest-dropped-events"),
        40
    );

    let total = event_count(&state, "site_test").await;
    assert_eq!(total, 60);
}

#[tokio::test]
async fn test_collect_self_hosted_queue_overflow_headers() {
    let (state, app) = setup().await;
    {
        let conn = state.db.conn_for_test().await;
        conn.execute(
            "UPDATE websites SET ingest_queue_max_events = ?1 WHERE id = ?2",
            sparklytics_duckdb::duckdb::params![1i64, "site_test"],
        )
        .expect("set custom queue cap");
    }

    let body = json!([
        { "website_id": "site_test", "type": "pageview", "url": "/q1" },
        { "website_id": "site_test", "type": "pageview", "url": "/q2" },
        { "website_id": "site_test", "type": "pageview", "url": "/q3" }
    ]);

    let response = app
        .oneshot(collect_request(&body.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        response
            .headers()
            .get("x-sparklytics-ingest-drop-reason")
            .and_then(|v| v.to_str().ok()),
        Some("queue_overflow")
    );
    assert_eq!(
        header_usize(&response, "x-sparklytics-ingest-accepted-events"),
        1
    );
    assert_eq!(
        header_usize(&response, "x-sparklytics-ingest-dropped-events"),
        2
    );

    let total = event_count(&state, "site_test").await;
    assert_eq!(total, 1);
}

#[tokio::test]
async fn test_ingest_limits_get_put_clear_cycle() {
    let (_state, app) = setup().await;

    let initial = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/websites/site_test/ingest-limits")
                .body(Body::empty())
                .expect("build get request"),
        )
        .await
        .expect("get ingest limits");
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_json = json_body(initial).await;
    assert_eq!(
        initial_json["data"]["source"]["peak_events_per_sec"],
        "default"
    );
    assert_eq!(
        initial_json["data"]["source"]["queue_max_events"],
        "default"
    );

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/websites/site_test/ingest-limits")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "peak_events_per_sec": 321,
                        "queue_max_events": 654
                    })
                    .to_string(),
                ))
                .expect("build put request"),
        )
        .await
        .expect("put ingest limits");
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_json = json_body(updated).await;
    assert_eq!(
        updated_json["data"]["source"]["peak_events_per_sec"],
        "custom"
    );
    assert_eq!(updated_json["data"]["source"]["queue_max_events"], "custom");
    assert_eq!(updated_json["data"]["peak_events_per_sec"], 321);
    assert_eq!(updated_json["data"]["queue_max_events"], 654);

    let cleared = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/websites/site_test/ingest-limits")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "peak_events_per_sec": null,
                        "queue_max_events": null
                    })
                    .to_string(),
                ))
                .expect("build clear request"),
        )
        .await
        .expect("clear ingest limits");
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_json = json_body(cleared).await;
    assert_eq!(
        cleared_json["data"]["source"]["peak_events_per_sec"],
        "default"
    );
    assert_eq!(
        cleared_json["data"]["source"]["queue_max_events"],
        "default"
    );
}

#[tokio::test]
async fn test_ingest_limits_reject_non_positive_values() {
    let (_state, app) = setup().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/websites/site_test/ingest-limits")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "peak_events_per_sec": 0
                    })
                    .to_string(),
                ))
                .expect("build invalid put request"),
        )
        .await
        .expect("invalid request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
