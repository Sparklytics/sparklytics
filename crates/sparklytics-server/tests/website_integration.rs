mod common;

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

/// Build a test Config with AuthMode::None (no auth required).
fn test_config() -> Config {
    Config {
        port: 0,
        data_dir: common::unique_data_dir("website"),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        bootstrap_password: None,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 65536,
        public_url: "http://localhost:3000".to_string(),
        tracking_public_base: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

/// Create a fresh in-memory backend + state + app for each test.
async fn setup() -> (Arc<AppState>, axum::Router) {
    setup_with_config(test_config()).await
}

async fn setup_with_config(config: Config) -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
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

/// Helper: send a POST /api/collect with the given JSON body.
fn collect_request(body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.1")
        .header("user-agent", "Mozilla/5.0 Chrome/120")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

/// Helper: create a website and return its ID.
async fn create_test_website(app: &axum::Router) -> String {
    let body = json!({
        "name": "Test Site",
        "domain": "test.example.com",
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_body(response).await;
    json["data"]["id"].as_str().expect("website id").to_string()
}

/// Helper: send events for a website and flush the buffer.
async fn seed_events(state: &AppState, app: &axum::Router, website_id: &str) {
    let events = json!([
        {
            "website_id": website_id,
            "type": "pageview",
            "url": "/home",
            "referrer": "https://google.com",
            "language": "en-US"
        },
        {
            "website_id": website_id,
            "type": "pageview",
            "url": "/about",
            "language": "en-US"
        },
        {
            "website_id": website_id,
            "type": "pageview",
            "url": "/pricing",
            "referrer": "https://twitter.com",
            "language": "de-DE"
        }
    ]);

    let response = app
        .clone()
        .oneshot(collect_request(&events.to_string()))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Flush buffer to ensure events are written to DuckDB.
    state.flush_buffer().await;
}

async fn seed_delete_cascade_children(state: &AppState, website_id: &str) {
    let conn = state.db.conn_for_test().await;

    conn.execute(
        "INSERT INTO goals (id, website_id, name, goal_type, match_value, match_operator)
         VALUES ('goal_cascade', ?1, 'Cascade Goal', 'page_view', '/pricing', 'equals')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed goal");
    conn.execute(
        "INSERT INTO saved_reports (id, website_id, name, description, config_json)
         VALUES ('report_cascade', ?1, 'Cascade Report', 'delete cascade test', '{}')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed saved report");
    conn.execute(
        "INSERT INTO attribution_cache (website_id, goal_id, model, range_start, range_end, payload_json)
         VALUES (?1, 'goal_cascade', 'first_touch', '2026-01-01 00:00:00', '2026-01-02 00:00:00', '{}')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed attribution cache");
    conn.execute(
        "INSERT INTO bot_policies (website_id, mode, threshold_score)
         VALUES (?1, 'balanced', 70)",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed bot policy");
    conn.execute(
        "INSERT INTO bot_allowlist (id, website_id, match_type, match_value, note)
         VALUES ('allow_cascade', ?1, 'ip_exact', '203.0.113.10', 'cascade test')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed bot allowlist");
    conn.execute(
        "INSERT INTO bot_blocklist (id, website_id, match_type, match_value, note)
         VALUES ('block_cascade', ?1, 'ua_contains', 'BadBot', 'cascade test')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed bot blocklist");
    conn.execute(
        "INSERT INTO bot_policy_audit (id, website_id, actor, action, payload)
         VALUES ('audit_cascade', ?1, 'test', 'policy_update', '{}')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed bot audit");
    conn.execute(
        "INSERT INTO bot_recompute_runs (id, website_id, start_date, end_date, status)
         VALUES ('recompute_cascade', ?1, '2026-01-01 00:00:00', '2026-01-02 00:00:00', 'queued')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed bot recompute");
    conn.execute(
        "INSERT INTO report_subscriptions (id, website_id, report_id, schedule, timezone, channel, target, next_run_at)
         VALUES ('sub_cascade', ?1, 'report_cascade', 'daily', 'UTC', 'webhook', 'https://example.com/hook', '2026-01-02 00:00:00')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed report subscription");
    conn.execute(
        "INSERT INTO alert_rules (id, website_id, name, metric, condition_type, threshold_value, channel, target)
         VALUES ('alert_cascade', ?1, 'Cascade Alert', 'pageviews', 'threshold_above', 100, 'webhook', 'https://example.com/hook')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed alert rule");
    conn.execute(
        "INSERT INTO notification_deliveries (id, source_type, source_id, idempotency_key, status)
         VALUES ('delivery_sub_cascade', 'subscription', 'sub_cascade', 'delivery_sub_cascade_key', 'sent')",
        [],
    )
    .expect("seed subscription delivery");
    conn.execute(
        "INSERT INTO notification_deliveries (id, source_type, source_id, idempotency_key, status)
         VALUES ('delivery_alert_cascade', 'alert', 'alert_cascade', 'delivery_alert_cascade_key', 'sent')",
        [],
    )
    .expect("seed alert delivery");
    conn.execute(
        "INSERT INTO campaign_links (id, website_id, name, slug, destination_url)
         VALUES ('link_cascade', ?1, 'Cascade Link', 'cascade-link', 'https://example.com')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed campaign link");
    conn.execute(
        "INSERT INTO tracking_pixels (id, website_id, name, pixel_key, default_url)
         VALUES ('pixel_cascade', ?1, 'Cascade Pixel', 'px_cascade', 'https://example.com/pixel')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed tracking pixel");
    conn.execute(
        "INSERT INTO funnels (id, website_id, name)
         VALUES ('funnel_cascade', ?1, 'Cascade Funnel')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("seed funnel");
    conn.execute(
        "INSERT INTO funnel_steps (id, funnel_id, step_order, step_type, match_value, label)
         VALUES ('step_cascade', 'funnel_cascade', 1, 'page_view', '/pricing', 'Pricing')",
        [],
    )
    .expect("seed funnel step");
}

fn count_rows_for_website(
    conn: &sparklytics_duckdb::duckdb::Connection,
    table: &str,
    website_id: &str,
) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE website_id = ?1");
    conn.prepare(&sql)
        .expect("prepare count")
        .query_row(sparklytics_duckdb::duckdb::params![website_id], |row| {
            row.get(0)
        })
        .expect("count rows")
}

// ============================================================
// BDD: Create a website in self-hosted mode
// ============================================================
#[tokio::test]
async fn test_create_website_selfhosted() {
    let (_state, app) = setup().await;

    let body = json!({
        "name": "My Blog",
        "domain": "blog.example.com",
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");

    let response = app.oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let data = &json["data"];

    // ID must start with "site_".
    let id = data["id"].as_str().expect("id should be a string");
    assert!(
        id.starts_with("site_"),
        "website id must start with 'site_'"
    );

    // tenant_id must be null in self-hosted mode (critical fact #2).
    assert!(
        data["tenant_id"].is_null(),
        "tenant_id must be null in self-hosted mode"
    );

    assert_eq!(data["name"], "My Blog");
    assert_eq!(data["domain"], "blog.example.com");
    assert_eq!(data["timezone"], "UTC");

    // tracking_snippet should contain the website ID.
    let snippet = data["tracking_snippet"].as_str().expect("tracking_snippet");
    assert!(
        snippet.contains(id),
        "tracking snippet must contain website ID"
    );
    assert!(
        snippet.contains(r#"src="http://localhost:3000/s.js""#),
        "tracking snippet must point at the configured analytics origin"
    );
}

#[tokio::test]
async fn test_create_website_uses_tracking_public_base_when_configured() {
    let mut config = test_config();
    config.tracking_public_base = "https://example.com/_sl".to_string();
    let (_state, app) = setup_with_config(config).await;

    let body = json!({
        "name": "My Blog",
        "domain": "blog.example.com",
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");

    let response = app.oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let snippet = json["data"]["tracking_snippet"]
        .as_str()
        .expect("tracking_snippet");

    assert!(
        snippet.contains(r#"src="https://example.com/_sl/s.js""#),
        "tracking snippet must use the configured public tracking base"
    );
}

#[tokio::test]
async fn test_tracking_script_is_served() {
    let (_state, app) = setup().await;

    for uri in ["/s.js", "/_sl/s.js"] {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .expect("build request");

        let response = app.clone().oneshot(request).await.expect("request");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/javascript")
        );
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("read body")
            .to_bytes();
        let script = std::str::from_utf8(&bytes).expect("script utf8");
        assert!(
            script.contains("Sparklytics tracking script"),
            "{uri} must serve the embedded tracker"
        );
    }
}

// ============================================================
// BDD: List websites
// ============================================================
#[tokio::test]
async fn test_list_websites() {
    let (_state, app) = setup().await;

    // Create two websites.
    let _ = create_test_website(&app).await;
    let _ = create_test_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = json["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 2, "should list 2 websites");

    // Pagination metadata should be present.
    assert!(json["pagination"]["total"].is_number());
    assert_eq!(json["pagination"]["total"], 2);
}

// ============================================================
// BDD: Update a website
// ============================================================
#[tokio::test]
async fn test_update_website() {
    let (_state, app) = setup().await;

    let website_id = create_test_website(&app).await;

    let body = json!({
        "name": "Updated Name",
        "domain": "updated.example.com",
    });
    let request = Request::builder()
        .method("PUT")
        .uri(format!("/api/websites/{}", website_id))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    assert_eq!(json["data"]["name"], "Updated Name");
    assert_eq!(json["data"]["domain"], "updated.example.com");
}

#[tokio::test]
async fn test_get_website_returns_tracking_snippet() {
    let (_state, app) = setup().await;
    let website_id = create_test_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}"))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let snippet = json["data"]["tracking_snippet"]
        .as_str()
        .expect("tracking_snippet");

    assert!(
        snippet.contains(&format!(r#"data-website-id="{website_id}""#)),
        "tracking snippet must contain the website ID"
    );
    assert!(
        snippet.contains(r#"src="http://localhost:3000/s.js""#),
        "tracking snippet must use the canonical tracking base"
    );
}

// ============================================================
// BDD: Delete a website cascades and returns 204
// ============================================================
#[tokio::test]
async fn test_delete_website_cascade() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;

    // Seed some events so we can verify cascade delete.
    seed_events(&state, &app, &website_id).await;
    seed_delete_cascade_children(&state, &website_id).await;

    // DELETE the website.
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{}", website_id))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify website is gone by listing websites.
    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = json["data"].as_array().expect("data should be an array");
    assert!(data.is_empty(), "website list should be empty after delete");

    // Verify events are also deleted (cascade).
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1")
        .expect("prepare");
    let count: i64 = stmt
        .query_row(sparklytics_duckdb::duckdb::params![&website_id], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(count, 0, "events should be cascade deleted");
    for table in [
        "sessions",
        "saved_reports",
        "goals",
        "attribution_cache",
        "bot_policies",
        "bot_allowlist",
        "bot_blocklist",
        "bot_policy_audit",
        "bot_recompute_runs",
        "report_subscriptions",
        "alert_rules",
        "campaign_links",
        "tracking_pixels",
        "funnels",
    ] {
        assert_eq!(
            count_rows_for_website(&conn, table, &website_id),
            0,
            "{table} should be cascade deleted"
        );
    }

    let notification_count: i64 = conn
        .prepare(
            "SELECT COUNT(*) FROM notification_deliveries
             WHERE source_id IN ('sub_cascade', 'alert_cascade')",
        )
        .expect("prepare notification count")
        .query_row([], |row| row.get(0))
        .expect("count notification deliveries");
    assert_eq!(
        notification_count, 0,
        "notification deliveries should be cascade deleted before their sources"
    );

    let step_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM funnel_steps WHERE funnel_id = 'funnel_cascade'")
        .expect("prepare funnel step count")
        .query_row([], |row| row.get(0))
        .expect("count funnel steps");
    assert_eq!(
        step_count, 0,
        "funnel steps should be cascade deleted before funnels"
    );
}

// ============================================================
// BDD: Stats endpoint returns data
// ============================================================
#[tokio::test]
async fn test_stats_returns_data() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;
    seed_events(&state, &app, &website_id).await;

    let (start, end) = common::surrounding_date_window();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/stats?start_date={}&end_date={}",
            website_id, start, end
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = &json["data"];

    // Stats should contain standard fields.
    assert!(
        data["pageviews"].is_number(),
        "stats should contain pageviews"
    );
    assert!(
        data["visitors"].is_number(),
        "stats should contain visitors"
    );
    assert!(
        data["sessions"].is_number(),
        "stats should contain sessions"
    );

    // We sent 3 pageviews.
    assert_eq!(data["pageviews"].as_i64().unwrap(), 3);
}

// ============================================================
// BDD: Pageviews endpoint returns time series
// ============================================================
#[tokio::test]
async fn test_pageviews_returns_series() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;
    seed_events(&state, &app, &website_id).await;

    let (start, end) = common::surrounding_date_window();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/pageviews?start_date={}&end_date={}",
            website_id, start, end
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = &json["data"];

    // Should contain series array and granularity.
    assert!(
        data["series"].is_array(),
        "data should contain series array"
    );
    assert!(
        data["granularity"].is_string(),
        "data should contain granularity"
    );

    let series = data["series"].as_array().expect("series is array");
    assert!(!series.is_empty(), "series should not be empty");
}

// ============================================================
// BDD: Metrics endpoint returns top pages
// ============================================================
#[tokio::test]
async fn test_metrics_top_pages() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;
    seed_events(&state, &app, &website_id).await;

    let (start, end) = common::surrounding_date_window();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/metrics?type=page&start_date={}&end_date={}",
            website_id, start, end
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = json["data"].as_object().expect("data should be an object");
    assert_eq!(data["type"], "page", "metric type should be page");

    let rows = data["rows"].as_array().expect("rows should be an array");
    // We sent pageviews to /home, /about, /pricing — expect 3 entries.
    assert_eq!(rows.len(), 3, "should have 3 page entries");

    // Pagination metadata should be present.
    assert!(
        json["pagination"].is_object(),
        "pagination should be present"
    );
}

// ============================================================
// BDD: Realtime endpoint returns active visitors
// ============================================================
#[tokio::test]
async fn test_realtime_active_visitors() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;
    seed_events(&state, &app, &website_id).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{}/realtime", website_id))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = &json["data"];

    // Realtime should contain active_visitors and recent_events (critical fact #3).
    assert!(
        data["active_visitors"].is_number(),
        "realtime should contain active_visitors"
    );
    assert!(
        data["recent_events"].is_array(),
        "realtime should contain recent_events (not recent_pageviews)"
    );
    assert!(
        data.get("recent_pageviews").is_none(),
        "realtime must not expose the stale recent_pageviews field"
    );
}

// ============================================================
// BDD: Stats with invalid country filter returns 400
// ============================================================
#[tokio::test]
async fn test_stats_invalid_country_filter() {
    let (_state, app) = setup().await;

    let website_id = create_test_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/stats?filter_country=INVALID",
            website_id
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "validation_error");
}

// ============================================================
// BDD: Core analytics endpoints reject reversed date ranges
// ============================================================
#[tokio::test]
async fn test_core_analytics_endpoints_reject_reversed_date_ranges() {
    let (_state, app) = setup().await;
    let website_id = create_test_website(&app).await;

    let requests = [
        format!("/api/websites/{website_id}/stats?start_date=2026-01-10&end_date=2026-01-01"),
        format!("/api/websites/{website_id}/pageviews?start_date=2026-01-10&end_date=2026-01-01"),
        format!(
            "/api/websites/{website_id}/metrics?type=page&start_date=2026-01-10&end_date=2026-01-01"
        ),
    ];

    for uri in requests {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .expect("build request");
        let response = app.clone().oneshot(request).await.expect("request");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], "validation_error");
    }
}
