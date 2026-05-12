use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

// BDD integration tests for the public share link feature.
//
// All tests use `AuthMode::None` (no auth on protected routes) to exercise the
// self-hosted share management endpoints and the public share analytics endpoints.
mod common;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::app::build_app;
use sparklytics_server::state::AppState;

static TRUSTED_PROXY_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn test_config() -> Config {
    Config {
        port: 0,
        data_dir: common::unique_data_dir("share"),
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

fn share_stats_request_with_connect_info(
    share_id: &str,
    spoofed_forwarded_for: &str,
    remote_addr: SocketAddr,
) -> Request<Body> {
    let mut request = Request::builder()
        .method("GET")
        .uri(format!("/api/share/{share_id}/stats"))
        .header("x-forwarded-for", spoofed_forwarded_for)
        .body(Body::empty())
        .expect("build request");
    request.extensions_mut().insert(ConnectInfo(remote_addr));
    request
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
// BDD: Shared stats reject reversed date range
// ============================================================
#[tokio::test]
async fn test_shared_stats_reject_reversed_date_range() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let enable_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let enable_res = app.clone().oneshot(enable_req).await.expect("request");
    assert_eq!(enable_res.status(), StatusCode::CREATED);
    let enable_json = json_body(enable_res).await;
    let share_id = enable_json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/share/{share_id}/stats?start_date=2026-01-10&end_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ============================================================
// BDD: Shared pageviews reject reversed date range
// ============================================================
#[tokio::test]
async fn test_shared_pageviews_reject_reversed_date_range() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let enable_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let enable_res = app.clone().oneshot(enable_req).await.expect("request");
    assert_eq!(enable_res.status(), StatusCode::CREATED);
    let enable_json = json_body(enable_res).await;
    let share_id = enable_json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/share/{share_id}/pageviews?start_date=2026-01-10&end_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ============================================================
// BDD: Shared stats reject oversized date range
// ============================================================
#[tokio::test]
async fn test_shared_stats_reject_oversized_date_range() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let enable_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let enable_res = app.clone().oneshot(enable_req).await.expect("request");
    assert_eq!(enable_res.status(), StatusCode::CREATED);
    let enable_json = json_body(enable_res).await;
    let share_id = enable_json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/share/{share_id}/stats?start_date=2025-01-01&end_date=2025-12-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = json_body(resp).await;
    assert_eq!(json["error"]["code"], "validation_error");
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

#[tokio::test]
async fn test_share_rate_limit_uses_socket_ip_when_proxy_not_trusted() {
    let _guard = TRUSTED_PROXY_ENV_LOCK.lock().await;
    let _env = EnvGuard::unset("SPARKLYTICS_TRUSTED_PROXIES");
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("enable share");
    let json = json_body(resp).await;
    let share_id = json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    let remote_addr = SocketAddr::from((Ipv4Addr::new(198, 51, 100, 10), 12345));

    for idx in 1..=30 {
        let req = share_stats_request_with_connect_info(
            &share_id,
            &format!("203.0.113.{idx}"),
            remote_addr,
        );
        let resp = app.clone().oneshot(req).await.expect("share stats");
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "request {idx} should be accepted"
        );
    }

    let req = share_stats_request_with_connect_info(&share_id, "203.0.113.250", remote_addr);
    let resp = app
        .clone()
        .oneshot(req)
        .await
        .expect("rate-limited share stats");

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_share_rate_limit_uses_forwarded_ip_when_proxy_is_trusted() {
    let _guard = TRUSTED_PROXY_ENV_LOCK.lock().await;
    let _env = EnvGuard::set("SPARKLYTICS_TRUSTED_PROXIES", "10.0.0.0/8");
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/share"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.clone().oneshot(req).await.expect("enable share");
    let json = json_body(resp).await;
    let share_id = json["data"]["share_id"]
        .as_str()
        .expect("share_id")
        .to_string();

    let trusted_proxy_addr = SocketAddr::from((Ipv4Addr::new(10, 0, 0, 10), 12345));

    for idx in 1..=35 {
        let req = share_stats_request_with_connect_info(
            &share_id,
            &format!("203.0.113.{idx}"),
            trusted_proxy_addr,
        );
        let resp = app.clone().oneshot(req).await.expect("share stats");
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "request {idx} should be accepted because each forwarded IP is distinct"
        );
    }
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
