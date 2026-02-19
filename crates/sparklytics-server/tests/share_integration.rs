/// BDD integration tests for the public share link feature.
///
/// All tests use `AuthMode::None` (no auth on protected routes) to exercise the
/// self-hosted share management endpoints and the public share analytics endpoints.
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
        duckdb_memory_limit: "1GB".to_string(),
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
// BDD: Enable sharing generates a unique share_id
// ============================================================
#[tokio::test]
async fn test_enable_sharing_generates_unique_share_id() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let share_id = json["data"]["share_id"].as_str().expect("share_id");
    // UUID v4 is 36 characters.
    assert_eq!(share_id.len(), 36, "share_id should be a 36-char UUID v4");
    // share_url should contain the share_id.
    let share_url = json["data"]["share_url"].as_str().expect("share_url");
    assert!(
        share_url.contains(share_id),
        "share_url must contain share_id"
    );
}

// ============================================================
// BDD: Enable sharing is idempotent (returns same share_id)
// ============================================================
#[tokio::test]
async fn test_enable_sharing_idempotent() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    // First POST.
    let req1 = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp1 = app.clone().oneshot(req1).await.expect("request");
    assert_eq!(resp1.status(), StatusCode::CREATED);
    let json1 = json_body(resp1).await;
    let share_id_1 = json1["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    // Second POST — must return the SAME share_id.
    let req2 = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp2 = app.clone().oneshot(req2).await.expect("request");
    assert_eq!(resp2.status(), StatusCode::CREATED);
    let json2 = json_body(resp2).await;
    let share_id_2 = json2["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    assert_eq!(share_id_1, share_id_2, "enable_sharing must be idempotent");
}

// ============================================================
// BDD: Shared stats accessible without auth
// ============================================================
#[tokio::test]
async fn test_shared_stats_accessible_without_auth() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    // Enable sharing.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    let json = json_body(resp).await;
    let share_id = json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    // GET /api/share/:share_id/stats without any auth.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/share/{share_id}/stats"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "shared stats should be accessible without auth"
    );

    let json = json_body(resp).await;
    assert!(
        json["data"]["pageviews"].is_number(),
        "response should have pageviews field"
    );
}

// ============================================================
// BDD: Unknown share_id returns 404
// ============================================================
#[tokio::test]
async fn test_unknown_share_id_returns_404() {
    let (_state, app) = setup().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/share/nonexistent-share-id/stats")
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let json = json_body(resp).await;
    assert_eq!(json["error"]["code"], "not_found");
}

// ============================================================
// BDD: Disable sharing invalidates the link
// ============================================================
#[tokio::test]
async fn test_disable_sharing_invalidates_link() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    // Enable sharing.
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    let json = json_body(resp).await;
    let share_id = json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    // Disable sharing.
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Old share URL should now return 404.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/share/{share_id}/stats"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "disabled share link should return 404"
    );
}
