use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::app::build_app;
use sparklytics_server::state::AppState;

const TEST_PASSWORD: &str = "strong_password_123";

fn config(auth_mode: AuthMode) -> Config {
    Config {
        port: 0,
        data_dir: "/tmp/sparklytics-test".to_string(),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode,
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

async fn setup_none() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let state = Arc::new(AppState::new(db, config(AuthMode::None)));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

async fn setup_auth() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let state = Arc::new(AppState::new(db, config(AuthMode::Local)));
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

async fn insert_session_row(
    state: &AppState,
    website_id: &str,
    session_id: &str,
    visitor_id: &str,
    first_seen: &str,
) {
    let conn = state.db.conn_for_test().await;
    conn.execute(
        r#"
        INSERT INTO sessions (
            session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, ?4, 1, '/'
        )
        "#,
        sparklytics_duckdb::duckdb::params![session_id, website_id, visitor_id, first_seen],
    )
    .expect("insert session");
}

async fn insert_event_row(
    state: &AppState,
    website_id: &str,
    event_id: &str,
    session_id: &str,
    visitor_id: &str,
    country: &str,
    created_at: &str,
) {
    let conn = state.db.conn_for_test().await;
    conn.execute(
        r#"
        INSERT INTO events (
            id, website_id, tenant_id, session_id, visitor_id, event_type, url,
            referrer_url, referrer_domain, event_name, event_data, country, region, city,
            browser, browser_version, os, os_version, device_type, screen, language,
            utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, 'pageview', '/pricing',
            NULL, NULL, NULL, NULL, ?5, NULL, NULL,
            'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
            NULL, NULL, NULL, NULL, NULL, ?6
        )
        "#,
        sparklytics_duckdb::duckdb::params![
            event_id, website_id, session_id, visitor_id, country, created_at
        ],
    )
    .expect("insert event");
}

async fn seed_retention_fixture(state: &AppState, website_id: &str) {
    insert_session_row(
        state,
        website_id,
        "sess_a0",
        "visitor_a",
        "2026-01-01 10:00:00",
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_a0",
        "sess_a0",
        "visitor_a",
        "PL",
        "2026-01-01 10:00:00",
    )
    .await;

    insert_session_row(
        state,
        website_id,
        "sess_a1",
        "visitor_a",
        "2026-01-08 10:00:00",
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_a1",
        "sess_a1",
        "visitor_a",
        "PL",
        "2026-01-08 10:00:00",
    )
    .await;

    insert_session_row(
        state,
        website_id,
        "sess_b0",
        "visitor_b",
        "2026-01-01 12:00:00",
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_b0",
        "sess_b0",
        "visitor_b",
        "US",
        "2026-01-01 12:00:00",
    )
    .await;
}

#[tokio::test]
async fn retention_endpoint_returns_matrix_payload() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    seed_retention_fixture(&state, &website_id).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&max_periods=4&start_date=2026-01-01&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::OK);

    let payload = json_body(response).await;
    assert_eq!(payload["data"]["granularity"], "week");
    assert_eq!(payload["data"]["max_periods"], 4);
    assert_eq!(payload["data"]["rows"][0]["cohort_size"], 2);
    assert_eq!(payload["data"]["rows"][0]["periods"][0]["rate"], 1.0);
}

#[tokio::test]
async fn retention_endpoint_validates_granularity_and_periods() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let invalid_granularity = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=hourly&start_date=2026-01-01&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app
        .clone()
        .oneshot(invalid_granularity)
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let invalid_periods = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&max_periods=13&start_date=2026-01-01&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(invalid_periods).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn retention_endpoint_returns_400_for_invalid_timezone() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&max_periods=8&timezone=Invalid%2FTimezone&start_date=2026-01-01&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn retention_endpoint_requires_start_date() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn retention_endpoint_requires_end_date() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&start_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn retention_endpoint_rejects_end_before_start() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&start_date=2026-01-31&end_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn retention_endpoint_returns_429_when_query_slot_busy() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let _permit_1 = state
        .retention_semaphore
        .acquire()
        .await
        .expect("acquire semaphore");
    let _permit_2 = state
        .retention_semaphore
        .acquire()
        .await
        .expect("acquire semaphore");

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/retention?cohort_granularity=week&max_periods=4&start_date=2026-01-01&end_date=2026-01-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = tokio::time::timeout(Duration::from_secs(8), app.clone().oneshot(request))
        .await
        .expect("retention request should not hang")
        .expect("request");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn retention_endpoint_returns_404_for_unknown_website() {
    let (_state, app) = setup_none().await;

    let request = Request::builder()
        .method("GET")
        .uri(
            "/api/websites/site_missing/retention?cohort_granularity=week&start_date=2026-01-01&end_date=2026-01-31",
        )
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn retention_endpoint_requires_auth_in_local_mode() {
    let (_state, app) = setup_auth().await;

    let setup_request = Request::builder()
        .method("POST")
        .uri("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "password": TEST_PASSWORD }).to_string()))
        .expect("build request");
    let setup_response = app.clone().oneshot(setup_request).await.expect("request");
    assert_eq!(setup_response.status(), StatusCode::CREATED);

    let request = Request::builder()
        .method("GET")
        .uri(
            "/api/websites/site_1/retention?cohort_granularity=week&start_date=2026-01-01&end_date=2026-01-31",
        )
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
