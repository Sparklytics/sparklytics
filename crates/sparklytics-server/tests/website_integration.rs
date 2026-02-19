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
    let config = test_config();
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
    assert!(id.starts_with("site_"), "website id must start with 'site_'");

    // tenant_id must be null in self-hosted mode (critical fact #2).
    assert!(data["tenant_id"].is_null(), "tenant_id must be null in self-hosted mode");

    assert_eq!(data["name"], "My Blog");
    assert_eq!(data["domain"], "blog.example.com");
    assert_eq!(data["timezone"], "UTC");

    // tracking_snippet should contain the website ID.
    let snippet = data["tracking_snippet"].as_str().expect("tracking_snippet");
    assert!(snippet.contains(id), "tracking snippet must contain website ID");
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

// ============================================================
// BDD: Delete a website cascades and returns 204
// ============================================================
#[tokio::test]
async fn test_delete_website_cascade() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;

    // Seed some events so we can verify cascade delete.
    seed_events(&state, &app, &website_id).await;

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
}

// ============================================================
// BDD: Stats endpoint returns data
// ============================================================
#[tokio::test]
async fn test_stats_returns_data() {
    let (state, app) = setup().await;

    let website_id = create_test_website(&app).await;
    seed_events(&state, &app, &website_id).await;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/stats?start_date={}&end_date={}",
            website_id, today, today
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = &json["data"];

    // Stats should contain standard fields.
    assert!(data["pageviews"].is_number(), "stats should contain pageviews");
    assert!(data["visitors"].is_number(), "stats should contain visitors");
    assert!(data["sessions"].is_number(), "stats should contain sessions");

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

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/pageviews?start_date={}&end_date={}",
            website_id, today, today
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    let data = &json["data"];

    // Should contain series array and granularity.
    assert!(data["series"].is_array(), "data should contain series array");
    assert!(data["granularity"].is_string(), "data should contain granularity");

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

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{}/metrics?type=page&start_date={}&end_date={}",
            website_id, today, today
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
    assert!(json["pagination"].is_object(), "pagination should be present");
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
