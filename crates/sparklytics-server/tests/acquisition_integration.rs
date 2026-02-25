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
    db.seed_website("site_test", "example.com")
        .await
        .expect("seed website");
    let state = Arc::new(AppState::new(db, test_config()));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", "198.51.100.10")
        .header("user-agent", "Mozilla/5.0 Chrome/120")
        .body(Body::from(body.to_string()))
        .expect("request")
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-forwarded-for", "198.51.100.10")
        .header("user-agent", "Mozilla/5.0 Chrome/120")
        .body(Body::empty())
        .expect("request")
}

async fn json_body(response: axum::http::Response<Body>) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn create_link_and_track_redirect_records_event() {
    let (state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Newsletter March",
                "destination_url": "https://example.com/pricing",
                "utm_source": "newsletter",
                "utm_medium": "email",
                "utm_campaign": "march_launch"
            }),
        ))
        .await
        .expect("create link");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let slug = created["data"]["slug"]
        .as_str()
        .expect("slug present")
        .to_string();

    let redirect_response = app
        .clone()
        .oneshot(get_request(&format!("/l/{slug}")))
        .await
        .expect("redirect");
    assert_eq!(redirect_response.status(), StatusCode::FOUND);
    let location = redirect_response
        .headers()
        .get(axum::http::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location");
    assert!(location.contains("utm_source=newsletter"));

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1 AND event_name = 'link_click'")
        .expect("prepare");
    let count: i64 = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1);

    let mut event_stmt = conn
        .prepare(
            "SELECT link_id, utm_source, event_name FROM events WHERE website_id = ?1 AND event_name = 'link_click' LIMIT 1",
        )
        .expect("prepare event");
    let (stored_link_id, stored_utm_source, stored_event_name): (Option<String>, Option<String>, Option<String>) =
        event_stmt
            .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .expect("event row");
    assert!(stored_link_id.is_some());
    assert_eq!(stored_utm_source.as_deref(), Some("newsletter"));
    assert_eq!(stored_event_name.as_deref(), Some("link_click"));
}

#[tokio::test]
async fn create_link_rejects_external_domain() {
    let (_state, app) = setup().await;
    let response = app
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "External",
                "destination_url": "https://evil.example/phish"
            }),
        ))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_link_rejects_non_http_scheme() {
    let (_state, app) = setup().await;
    let response = app
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Bad URL",
                "destination_url": "javascript:alert(1)"
            }),
        ))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_link_rejects_external_domain() {
    let (_state, app) = setup().await;
    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Original",
                "destination_url": "https://example.com/pricing"
            }),
        ))
        .await
        .expect("create link");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let link_id = created["data"]["id"].as_str().expect("link id");

    let update_response = app
        .oneshot(json_request(
            "PUT",
            &format!("/api/websites/site_test/links/{link_id}"),
            json!({
                "destination_url": "https://evil.example/redirect"
            }),
        ))
        .await
        .expect("update");
    assert_eq!(update_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pixel_endpoint_returns_gif_and_records_event() {
    let (state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/pixels",
            json!({
                "name": "Email Pixel",
                "default_url": "https://example.com/docs"
            }),
        ))
        .await
        .expect("create pixel");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let pixel_key = created["data"]["pixel_key"]
        .as_str()
        .expect("pixel key");

    let pixel_response = app
        .clone()
        .oneshot(get_request(&format!("/p/{pixel_key}.gif")))
        .await
        .expect("pixel");
    assert_eq!(pixel_response.status(), StatusCode::OK);
    assert_eq!(
        pixel_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/gif")
    );
    assert_eq!(
        pixel_response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store, no-cache, must-revalidate")
    );
    assert_eq!(
        pixel_response
            .headers()
            .get(axum::http::header::PRAGMA)
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1 AND event_name = 'pixel_view'")
        .expect("prepare");
    let count: i64 = stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1);

    let mut event_stmt = conn
        .prepare(
            "SELECT pixel_id, event_name FROM events WHERE website_id = ?1 AND event_name = 'pixel_view' LIMIT 1",
        )
        .expect("prepare event");
    let (stored_pixel_id, stored_event_name): (Option<String>, Option<String>) = event_stmt
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("event row");
    assert!(stored_pixel_id.is_some());
    assert_eq!(stored_event_name.as_deref(), Some("pixel_view"));
}

#[tokio::test]
async fn inactive_link_returns_not_found_without_recording_event() {
    let (state, app) = setup().await;
    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Inactive Link",
                "destination_url": "https://example.com/pricing"
            }),
        ))
        .await
        .expect("create link");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let link_id = created["data"]["id"].as_str().expect("link id");
    let slug = created["data"]["slug"].as_str().expect("slug");

    let disable_response = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/websites/site_test/links/{link_id}"),
            json!({ "is_active": false }),
        ))
        .await
        .expect("disable link");
    assert_eq!(disable_response.status(), StatusCode::OK);

    let response = app
        .oneshot(get_request(&format!("/l/{slug}")))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1 AND event_name = 'link_click'")
        .expect("prepare")
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn inactive_pixel_returns_not_found_without_recording_event() {
    let (state, app) = setup().await;
    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/pixels",
            json!({
                "name": "Inactive Pixel",
                "default_url": "https://example.com/docs"
            }),
        ))
        .await
        .expect("create pixel");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let pixel_id = created["data"]["id"].as_str().expect("pixel id");
    let pixel_key = created["data"]["pixel_key"].as_str().expect("pixel key");

    let disable_response = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/websites/site_test/pixels/{pixel_id}"),
            json!({ "is_active": false }),
        ))
        .await
        .expect("disable pixel");
    assert_eq!(disable_response.status(), StatusCode::OK);

    let response = app
        .oneshot(get_request(&format!("/p/{pixel_key}")))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM events WHERE website_id = ?1 AND event_name = 'pixel_view'")
        .expect("prepare")
        .query_row(sparklytics_duckdb::duckdb::params!["site_test"], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn link_tracker_rejects_excess_query_params() {
    let (_state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Query cap",
                "destination_url": "https://example.com/pricing"
            }),
        ))
        .await
        .expect("create link");
    let created = json_body(create_response).await;
    let slug = created["data"]["slug"].as_str().expect("slug");

    let query = (0..33)
        .map(|i| format!("k{i}=v{i}"))
        .collect::<Vec<_>>()
        .join("&");
    let response = app
        .oneshot(get_request(&format!("/l/{slug}?{query}")))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pixel_tracker_rejects_excess_query_params() {
    let (_state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/pixels",
            json!({
                "name": "Query cap",
                "default_url": "https://example.com/docs"
            }),
        ))
        .await
        .expect("create pixel");
    let created = json_body(create_response).await;
    let pixel_key = created["data"]["pixel_key"].as_str().expect("pixel key");

    let query = (0..33)
        .map(|i| format!("k{i}=v{i}"))
        .collect::<Vec<_>>()
        .join("&");
    let response = app
        .oneshot(get_request(&format!("/p/{pixel_key}?{query}")))
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn acquisition_management_crud_endpoints_work() {
    let (_state, app) = setup().await;

    let create_link = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "CRUD Link",
                "destination_url": "https://example.com/pricing"
            }),
        ))
        .await
        .expect("create link");
    assert_eq!(create_link.status(), StatusCode::CREATED);
    let link_json = json_body(create_link).await;
    let link_id = link_json["data"]["id"].as_str().expect("link id");

    let list_links = app
        .clone()
        .oneshot(get_request("/api/websites/site_test/links"))
        .await
        .expect("list links");
    assert_eq!(list_links.status(), StatusCode::OK);

    let update_link = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/websites/site_test/links/{link_id}"),
            json!({ "name": "CRUD Link Updated" }),
        ))
        .await
        .expect("update link");
    assert_eq!(update_link.status(), StatusCode::OK);

    let link_stats = app
        .clone()
        .oneshot(get_request(&format!(
            "/api/websites/site_test/links/{link_id}/stats"
        )))
        .await
        .expect("link stats");
    assert_eq!(link_stats.status(), StatusCode::OK);

    let create_pixel = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/pixels",
            json!({
                "name": "CRUD Pixel",
                "default_url": "https://example.com/docs"
            }),
        ))
        .await
        .expect("create pixel");
    assert_eq!(create_pixel.status(), StatusCode::CREATED);
    let pixel_json = json_body(create_pixel).await;
    let pixel_id = pixel_json["data"]["id"].as_str().expect("pixel id");

    let list_pixels = app
        .clone()
        .oneshot(get_request("/api/websites/site_test/pixels"))
        .await
        .expect("list pixels");
    assert_eq!(list_pixels.status(), StatusCode::OK);

    let update_pixel = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/websites/site_test/pixels/{pixel_id}"),
            json!({ "name": "CRUD Pixel Updated" }),
        ))
        .await
        .expect("update pixel");
    assert_eq!(update_pixel.status(), StatusCode::OK);

    let pixel_stats = app
        .clone()
        .oneshot(get_request(&format!(
            "/api/websites/site_test/pixels/{pixel_id}/stats"
        )))
        .await
        .expect("pixel stats");
    assert_eq!(pixel_stats.status(), StatusCode::OK);

    let delete_link = app
        .clone()
        .oneshot(Request::builder()
            .method("DELETE")
            .uri(format!("/api/websites/site_test/links/{link_id}"))
            .header("x-forwarded-for", "198.51.100.10")
            .header("user-agent", "Mozilla/5.0 Chrome/120")
            .body(Body::empty())
            .expect("delete link request"))
        .await
        .expect("delete link");
    assert_eq!(delete_link.status(), StatusCode::NO_CONTENT);

    let delete_pixel = app
        .oneshot(Request::builder()
            .method("DELETE")
            .uri(format!("/api/websites/site_test/pixels/{pixel_id}"))
            .header("x-forwarded-for", "198.51.100.10")
            .header("user-agent", "Mozilla/5.0 Chrome/120")
            .body(Body::empty())
            .expect("delete pixel request"))
        .await
        .expect("delete pixel");
    assert_eq!(delete_pixel.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn campaign_link_rate_limit_enforced() {
    let (_state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/links",
            json!({
                "name": "Rate test",
                "destination_url": "https://example.com/pricing"
            }),
        ))
        .await
        .expect("create link");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let slug = created["data"]["slug"].as_str().expect("slug");

    for _ in 0..120 {
        let response = app
            .clone()
            .oneshot(get_request(&format!("/l/{slug}")))
            .await
            .expect("request");
        assert_eq!(response.status(), StatusCode::FOUND);
    }

    let limited = app
        .oneshot(get_request(&format!("/l/{slug}")))
        .await
        .expect("rate limited request");
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn tracking_pixel_rate_limit_enforced() {
    let (_state, app) = setup().await;

    let create_response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/websites/site_test/pixels",
            json!({
                "name": "Pixel rate test",
                "default_url": "https://example.com/docs"
            }),
        ))
        .await
        .expect("create pixel");
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = json_body(create_response).await;
    let pixel_key = created["data"]["pixel_key"].as_str().expect("pixel key");

    for _ in 0..240 {
        let response = app
            .clone()
            .oneshot(get_request(&format!("/p/{pixel_key}")))
            .await
            .expect("request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let limited = app
        .oneshot(get_request(&format!("/p/{pixel_key}")))
        .await
        .expect("rate limited request");
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
}
