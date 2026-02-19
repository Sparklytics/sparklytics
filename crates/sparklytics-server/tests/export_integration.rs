/// BDD integration tests for the CSV export endpoint.
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
    }
}

async fn setup() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let config = test_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

async fn json_body(response: axum::http::Response<Body>) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("parse JSON")
}

async fn text_body(response: axum::http::Response<Body>) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

async fn create_website(app: &axum::Router) -> String {
    let body = json!({ "name": "Test", "domain": "test.example.com" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_body(response).await;
    json["data"]["id"].as_str().expect("id").to_string()
}

// ============================================================
// BDD: Export returns text/csv with correct headers
// ============================================================
#[tokio::test]
async fn test_export_events_as_csv() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/export?start_date={today}&end_date={today}&format=csv"
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/csv"),
        "content-type should be text/csv, got: {content_type}"
    );

    let content_disposition = response
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_disposition.contains("attachment"),
        "content-disposition should contain 'attachment'"
    );

    let body = text_body(response).await;
    // First line should be the CSV header row.
    let first_line = body.lines().next().unwrap_or("");
    assert!(
        first_line.contains("id"),
        "first CSV line should be headers containing 'id', got: {first_line}"
    );
    assert!(
        first_line.contains("event_type"),
        "first CSV line should contain 'event_type', got: {first_line}"
    );
    assert!(
        first_line.contains("created_at"),
        "first CSV line should contain 'created_at', got: {first_line}"
    );
}

// ============================================================
// BDD: Date range > 90 days returns 400 validation_error
// ============================================================
#[tokio::test]
async fn test_export_date_range_too_large_returns_400() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    // 100-day range.
    let start = "2026-01-01";
    let end = "2026-04-11"; // 100 days later

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/export?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json = json_body(response).await;
    assert_eq!(
        json["error"]["code"], "validation_error",
        "expected validation_error, got: {:?}",
        json
    );
}

// ============================================================
// BDD: Export unknown website returns 404
// ============================================================
#[tokio::test]
async fn test_export_unknown_website_returns_404() {
    let (_state, app) = setup().await;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/site_nonexistent/export?start_date={today}&end_date={today}"
        ))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
