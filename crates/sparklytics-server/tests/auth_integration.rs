use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use sparklytics_core::billing::{BillingGate, NullBillingGate};
use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::app::build_app;
use sparklytics_server::state::AppState;

const TEST_PASSWORD: &str = "strong_password_123";

/// Build a test Config with AuthMode::Local and low argon2 memory for fast tests.
fn auth_config() -> Config {
    Config {
        port: 0,
        data_dir: "/tmp/sparklytics-test".to_string(),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::Local,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 4096, // Low memory for fast tests.
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

/// Build a test Config with AuthMode::Local in Cloud mode.
fn cloud_auth_config() -> Config {
    let mut config = auth_config();
    config.mode = AppMode::Cloud;
    config
}

/// Build a test Config with AuthMode::None.
fn none_config() -> Config {
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
        argon2_memory_kb: 4096,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

/// Build a test Config with AuthMode::Password.
fn password_config() -> Config {
    Config {
        port: 0,
        data_dir: "/tmp/sparklytics-test".to_string(),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::Password(TEST_PASSWORD.to_string()),
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 4096,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

/// Create a fresh in-memory backend + state + app with AuthMode::Local.
async fn setup_auth() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let config = auth_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

/// Create a fresh in-memory backend + state + app with AuthMode::Local in Cloud mode.
async fn setup_auth_cloud() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let config = cloud_auth_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

/// Create a fresh in-memory backend + state + app with AuthMode::None.
async fn setup_none() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let config = none_config();
    let state = Arc::new(AppState::new(db, config));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

/// Create a fresh in-memory backend + state + app with AuthMode::Password.
async fn setup_password() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let config = password_config();
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

/// Helper: POST /api/auth/setup with a password.
fn setup_request(password: &str) -> Request<Body> {
    let body = json!({ "password": password });
    Request::builder()
        .method("POST")
        .uri("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

/// Helper: POST /api/auth/login with a password.
fn login_request(password: &str) -> Request<Body> {
    let body = json!({ "password": password });
    Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.1")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

/// Helper: run setup + login and return the session cookie string.
async fn setup_and_login(app: &axum::Router) -> String {
    // Step 1: Setup admin.
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("setup request");
    assert_eq!(response.status(), StatusCode::CREATED);

    // Step 2: Login.
    let response = app
        .clone()
        .oneshot(login_request(TEST_PASSWORD))
        .await
        .expect("login request");
    assert_eq!(response.status(), StatusCode::OK);

    // Extract Set-Cookie header.
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header must be present")
        .to_str()
        .expect("valid header string")
        .to_string();

    // Extract just the cookie value: "spk_session=<token>; ..."
    let cookie = set_cookie
        .split(';')
        .next()
        .expect("cookie value")
        .to_string();

    cookie
}

// ============================================================
// BDD: Setup creates admin
// ============================================================
#[tokio::test]
async fn test_setup_creates_admin() {
    let (_state, app) = setup_auth().await;

    let response = app
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    assert_eq!(json["data"]["ok"], true);
}

// ============================================================
// BDD: Setup rejects whitespace-only password
// ============================================================
#[tokio::test]
async fn test_setup_rejects_whitespace_only_password() {
    let (_state, app) = setup_auth().await;

    let body = json!({ "password": "            " });
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "validation_error");
}

// ============================================================
// BDD: Setup returns 410 after first use
// ============================================================
#[tokio::test]
async fn test_setup_returns_410_after_use() {
    let (_state, app) = setup_auth().await;

    // First setup succeeds.
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    // Second setup returns 410 Gone.
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::GONE);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "gone");
}

// ============================================================
// BDD: Login sets HttpOnly cookie
// ============================================================
#[tokio::test]
async fn test_login_sets_httponly_cookie() {
    let (_state, app) = setup_auth().await;

    // Setup admin first.
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("setup");
    assert_eq!(response.status(), StatusCode::CREATED);

    // Login.
    let response = app
        .clone()
        .oneshot(login_request(TEST_PASSWORD))
        .await
        .expect("login");

    assert_eq!(response.status(), StatusCode::OK);

    // Verify Set-Cookie header.
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header must be present")
        .to_str()
        .expect("valid header string");

    assert!(
        set_cookie.starts_with("spk_session="),
        "cookie must be named spk_session"
    );
    assert!(
        set_cookie.contains("HttpOnly"),
        "cookie must have HttpOnly flag"
    );
    assert!(
        set_cookie.contains("SameSite=Strict"),
        "cookie must have SameSite=Strict"
    );

    // HTTPS is false in test config, so Secure flag should NOT be present.
    assert!(
        !set_cookie.contains("Secure"),
        "cookie should not have Secure flag when https=false"
    );

    // Response body must NOT expose the raw token (JWT is HttpOnly-cookie only).
    // It should contain expires_at so the client can schedule re-auth.
    let json = json_body(response).await;
    assert!(
        json["data"]["token"].is_null(),
        "token must not be present in response body (security: HttpOnly cookie only)"
    );
    assert!(
        json["data"]["expires_at"].is_string(),
        "response should contain expires_at"
    );
}

// ============================================================
// BDD: Login with wrong password returns 401
// ============================================================
#[tokio::test]
async fn test_login_wrong_password() {
    let (_state, app) = setup_auth().await;

    // Setup admin.
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("setup");
    assert_eq!(response.status(), StatusCode::CREATED);

    // Attempt login with wrong password.
    let response = app
        .clone()
        .oneshot(login_request("wrong_password_123"))
        .await
        .expect("login");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "unauthorized");
}

// ============================================================
// BDD: Login rate limit returns Retry-After header
// ============================================================
#[tokio::test]
async fn test_login_rate_limit_returns_retry_after_header() {
    let (_state, app) = setup_auth().await;

    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("setup");
    assert_eq!(response.status(), StatusCode::CREATED);

    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(login_request("wrong_password_123"))
            .await
            .expect("login");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let response = app
        .clone()
        .oneshot(login_request("wrong_password_123"))
        .await
        .expect("rate-limited login");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let retry_after = response
        .headers()
        .get("retry-after")
        .expect("retry-after header")
        .to_str()
        .expect("retry-after string");
    assert_eq!(retry_after, "900");
}

// ============================================================
// BDD: Valid session cookie allows access to protected routes
// ============================================================
#[tokio::test]
async fn test_valid_session_allows_access() {
    let (_state, app) = setup_auth().await;

    let cookie = setup_and_login(&app).await;

    // Access protected route with cookie.
    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .header("cookie", &cookie)
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;
    assert!(json["data"].is_array(), "should return websites array");
}

// ============================================================
// BDD: No auth returns 401 on protected routes
// ============================================================
#[tokio::test]
async fn test_no_auth_returns_401() {
    let (_state, app) = setup_auth().await;

    // Setup admin so the middleware does not return "setup_required".
    let response = app
        .clone()
        .oneshot(setup_request(TEST_PASSWORD))
        .await
        .expect("setup");
    assert_eq!(response.status(), StatusCode::CREATED);

    // Try to access protected route without cookie.
    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "unauthorized");
}

// ============================================================
// BDD: API key grants access to analytics endpoints
// ============================================================
#[tokio::test]
async fn test_api_key_grants_analytics_access() {
    let (_state, app) = setup_auth().await;

    let cookie = setup_and_login(&app).await;

    // Create an API key (requires cookie auth on POST /api/auth/keys).
    let create_key_body = json!({ "name": "test-key" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/keys")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .body(Body::from(create_key_body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let raw_key = json["data"]["key"]
        .as_str()
        .expect("API key should be returned");
    assert!(
        raw_key.starts_with("spk_selfhosted_"),
        "API key must start with spk_selfhosted_ prefix"
    );

    // Use the API key to access a protected analytics endpoint.
    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .header("authorization", format!("Bearer {}", raw_key))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================
// BDD: Cloud mode API key uses spk_live_ prefix
// ============================================================
#[tokio::test]
async fn test_cloud_api_key_prefix_and_access() {
    let (_state, app) = setup_auth_cloud().await;

    let cookie = setup_and_login(&app).await;

    let create_key_body = json!({ "name": "cloud-key" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/keys")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .body(Body::from(create_key_body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let raw_key = json["data"]["key"]
        .as_str()
        .expect("API key should be returned");
    assert!(
        raw_key.starts_with("spk_live_"),
        "API key must start with spk_live_ prefix in cloud mode"
    );

    let request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .header("authorization", format!("Bearer {}", raw_key))
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================
// BDD: API key is forbidden for auth management endpoints
// ============================================================
#[tokio::test]
async fn test_api_key_forbidden_for_auth_mgmt() {
    let (_state, app) = setup_auth().await;

    let cookie = setup_and_login(&app).await;

    // Create an API key.
    let create_key_body = json!({ "name": "test-key-forbidden" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/keys")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .body(Body::from(create_key_body.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = json_body(response).await;
    let raw_key = json["data"]["key"]
        .as_str()
        .expect("API key should be returned");

    // Try to use the API key on a cookie-only auth management endpoint.
    // POST /api/auth/keys requires cookie auth — API key should get 403.
    let create_another_key = json!({ "name": "sneaky-key" });
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/keys")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", raw_key))
        .body(Body::from(create_another_key.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let json = json_body(response).await;
    assert_eq!(json["error"]["code"], "forbidden");
}

// ============================================================
// BDD: Auth endpoints return 404 in None mode
// ============================================================
#[tokio::test]
async fn test_auth_endpoints_404_in_none_mode() {
    let (_state, app) = setup_none().await;

    // GET /api/auth/status should 404 in None mode (endpoint not registered).
    let request = Request::builder()
        .method("GET")
        .uri("/api/auth/status")
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "auth/status should return 404 in None mode"
    );
}

// ============================================================
// BDD: Auth status returns mode in Local mode
// ============================================================
#[tokio::test]
async fn test_auth_status_returns_mode() {
    let (_state, app) = setup_auth().await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/auth/status")
        .body(Body::empty())
        .expect("build request");

    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let json = json_body(response).await;

    // Flat response (no {"data":...} wrapper) per CLAUDE.md critical fact #15.
    assert_eq!(json["mode"], "local");
    assert_eq!(
        json["setup_required"], true,
        "setup_required should be true before setup"
    );
    assert_eq!(
        json["authenticated"], false,
        "authenticated should be false without cookie"
    );
}

// ============================================================
// BDD: Password mode first-run login flow works without setup
// ============================================================
#[tokio::test]
async fn test_password_mode_first_run_login_flow() {
    let (_state, app) = setup_password().await;

    let status_request = Request::builder()
        .method("GET")
        .uri("/api/auth/status")
        .body(Body::empty())
        .expect("build request");
    let status_response = app
        .clone()
        .oneshot(status_request)
        .await
        .expect("status request");
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_json = json_body(status_response).await;
    assert_eq!(status_json["mode"], "password");
    assert_eq!(status_json["setup_required"], false);
    assert_eq!(status_json["authenticated"], false);

    let login_response = app
        .clone()
        .oneshot(login_request(TEST_PASSWORD))
        .await
        .expect("login request");
    assert_eq!(login_response.status(), StatusCode::OK);
    let cookie = login_response
        .headers()
        .get("set-cookie")
        .expect("set-cookie")
        .to_str()
        .expect("valid header")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string();

    let websites_request = Request::builder()
        .method("GET")
        .uri("/api/websites")
        .header("cookie", cookie)
        .body(Body::empty())
        .expect("build request");
    let websites_response = app
        .clone()
        .oneshot(websites_request)
        .await
        .expect("websites request");
    assert_eq!(websites_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_null_billing_gate_test_in_server_still_passes() {
    let gate = NullBillingGate;
    let admission = gate.admit_events("org_any", 7).await;
    assert_eq!(admission.allowed_events, 7);
    assert!(admission.reason.is_none());
}
