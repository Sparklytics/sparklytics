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

async fn seed_custom_events(state: &AppState, app: &axum::Router, website_id: &str) {
    let events = json!([
        {
            "website_id": website_id,
            "type": "event",
            "url": "/checkout",
            "event_name": "purchase",
            "event_data": { "plan": "pro", "currency": "USD" },
            "language": "en-US"
        },
        {
            "website_id": website_id,
            "type": "event",
            "url": "/checkout",
            "event_name": "purchase",
            "event_data": { "plan": "free", "currency": "USD" },
            "language": "en-US"
        },
        {
            "website_id": website_id,
            "type": "event",
            "url": "/signup",
            "event_name": "signup",
            "event_data": { "method": "google" },
            "language": "en-US"
        }
    ]);

    let request = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.0.0.1")
        .header("user-agent", "Mozilla/5.0 Chrome/120")
        .body(Body::from(events.to_string()))
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    state.flush_buffer().await;
}

#[tokio::test]
async fn test_events_endpoints_return_custom_event_data() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    seed_custom_events(&state, &app, &website_id).await;

    let today = chrono::Utc::now().date_naive();
    let start = (today - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let end = today.format("%Y-%m-%d").to_string();

    let names_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");
    let names_response = app.clone().oneshot(names_request).await.expect("request");
    assert_eq!(names_response.status(), StatusCode::OK);
    let names_json = json_body(names_response).await;
    assert!(names_json["data"]["rows"]
        .as_array()
        .expect("rows array")
        .iter()
        .any(|row| row["event_name"] == "purchase"));

    let properties_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events/properties?event_name=purchase&start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");
    let properties_response = app
        .clone()
        .oneshot(properties_request)
        .await
        .expect("request");
    assert_eq!(properties_response.status(), StatusCode::OK);
    let properties_json = json_body(properties_response).await;
    assert_eq!(properties_json["data"]["event_name"], "purchase");
    assert!(properties_json["data"]["properties"]
        .as_array()
        .expect("properties array")
        .iter()
        .any(|row| row["property_key"] == "plan"));

    let timeseries_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events/timeseries?event_name=purchase&start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");
    let timeseries_response = app
        .clone()
        .oneshot(timeseries_request)
        .await
        .expect("request");
    assert_eq!(timeseries_response.status(), StatusCode::OK);
    let timeseries_json = json_body(timeseries_response).await;
    assert!(timeseries_json["data"]["series"]
        .as_array()
        .expect("series array")
        .iter()
        .any(|point| point["pageviews"].as_i64().unwrap_or_default() >= 1));
}

#[tokio::test]
async fn test_events_endpoints_require_event_name_on_detail_routes() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let properties_request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/events/properties"))
        .body(Body::empty())
        .expect("build request");
    let properties_response = app
        .clone()
        .oneshot(properties_request)
        .await
        .expect("request");
    assert_eq!(properties_response.status(), StatusCode::BAD_REQUEST);

    let timeseries_request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/events/timeseries"))
        .body(Body::empty())
        .expect("build request");
    let timeseries_response = app
        .clone()
        .oneshot(timeseries_request)
        .await
        .expect("request");
    assert_eq!(timeseries_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_events_endpoints_return_404_for_unknown_website() {
    let (_state, app) = setup_none().await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/websites/site_missing/events")
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_events_endpoints_require_auth_in_local_mode() {
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
        .uri("/api/websites/site_any/events")
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_events_endpoints_validate_date_ranges_and_timezone() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let reversed_range = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events?start_date=2026-01-10&end_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let reversed_response = app.clone().oneshot(reversed_range).await.expect("request");
    assert_eq!(reversed_response.status(), StatusCode::BAD_REQUEST);

    let too_large_range = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events?start_date=2025-01-01&end_date=2025-12-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let too_large_response = app.clone().oneshot(too_large_range).await.expect("request");
    assert_eq!(too_large_response.status(), StatusCode::BAD_REQUEST);

    let empty_timezone = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/events?timezone=%20%20%20"
        ))
        .body(Body::empty())
        .expect("build request");
    let empty_timezone_response = app.clone().oneshot(empty_timezone).await.expect("request");
    assert_eq!(empty_timezone_response.status(), StatusCode::BAD_REQUEST);
}
