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

async fn seed_sessions_data(state: &AppState, website_id: &str) {
    let conn = state.db.conn_for_test().await;

    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)",
        sparklytics_duckdb::duckdb::params![
            "sess_a",
            website_id,
            "visitor_a",
            "2026-02-20 10:00:00",
            "2026-02-20 10:10:00",
            2,
            "https://docs.example.com/start"
        ],
    )
    .expect("insert session a");
    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)",
        sparklytics_duckdb::duckdb::params![
            "sess_b",
            website_id,
            "visitor_b",
            "2026-02-20 09:00:00",
            "2026-02-20 09:15:00",
            2,
            "https://app.example.com/home"
        ],
    )
    .expect("insert session b");
    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)",
        sparklytics_duckdb::duckdb::params![
            "sess_c",
            website_id,
            "visitor_c",
            "2026-02-20 08:00:00",
            "2026-02-20 08:20:00",
            1,
            "https://blog.example.com/post"
        ],
    )
    .expect("insert session c");

    let insert_event = |id: &str,
                        session_id: &str,
                        event_type: &str,
                        url: &str,
                        event_name: Option<&str>,
                        event_data: Option<&str>,
                        created_at: &str| {
        conn.execute(
            r#"
            INSERT INTO events (
                id, website_id, tenant_id, session_id, visitor_id, event_type, url,
                referrer_url, referrer_domain, event_name, event_data, country, region, city,
                browser, browser_version, os, os_version, device_type, screen, language,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
            ) VALUES (
                ?1, ?2, NULL, ?3, ?4, ?5, ?6,
                NULL, NULL, ?7, ?8, 'PL', 'Mazowieckie', 'Warsaw',
                'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
                NULL, NULL, NULL, NULL, NULL, ?9
            )
            "#,
            sparklytics_duckdb::duckdb::params![
                id,
                website_id,
                session_id,
                format!("visitor_{}", &session_id[5..]),
                event_type,
                url,
                event_name,
                event_data,
                created_at
            ],
        )
        .expect("insert event");
    };

    insert_event(
        "evt_a1",
        "sess_a",
        "pageview",
        "https://docs.example.com/start",
        None,
        None,
        "2026-02-20 10:00:00",
    );
    insert_event(
        "evt_a2",
        "sess_a",
        "event",
        "https://docs.example.com/checkout",
        Some("purchase"),
        Some(r#"{"plan":"pro"}"#),
        "2026-02-20 10:05:00",
    );
    insert_event(
        "evt_a3",
        "sess_a",
        "pageview",
        "https://docs.example.com/thank-you",
        None,
        None,
        "2026-02-20 10:10:00",
    );
    insert_event(
        "evt_b1",
        "sess_b",
        "pageview",
        "https://app.example.com/home",
        None,
        None,
        "2026-02-20 09:00:00",
    );
    insert_event(
        "evt_b2",
        "sess_b",
        "event",
        "https://app.example.com/upgrade",
        Some("upgrade_click"),
        Some(r#"{"cta":"header"}"#),
        "2026-02-20 09:15:00",
    );
    insert_event(
        "evt_c1",
        "sess_c",
        "pageview",
        "https://blog.example.com/post",
        None,
        None,
        "2026-02-20 08:20:00",
    );
}

#[tokio::test]
async fn test_list_sessions_cursor_pagination_and_hostname_filter() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    seed_sessions_data(&state, &website_id).await;

    let first_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/sessions?start_date=2026-02-20&end_date=2026-02-20&limit=2"
        ))
        .body(Body::empty())
        .expect("build request");
    let first_response = app.clone().oneshot(first_request).await.expect("request");
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_json = json_body(first_response).await;
    let first_rows = first_json["data"].as_array().expect("rows array");
    assert_eq!(first_rows.len(), 2);
    assert_eq!(first_rows[0]["session_id"], "sess_a");
    assert_eq!(first_rows[1]["session_id"], "sess_b");
    assert_eq!(first_json["pagination"]["has_more"], true);

    let cursor = first_json["pagination"]["next_cursor"]
        .as_str()
        .expect("next cursor");
    let second_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/sessions?start_date=2026-02-20&end_date=2026-02-20&limit=2&cursor={cursor}"
        ))
        .body(Body::empty())
        .expect("build request");
    let second_response = app.clone().oneshot(second_request).await.expect("request");
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_json = json_body(second_response).await;
    let second_rows = second_json["data"].as_array().expect("rows array");
    assert_eq!(second_rows.len(), 1);
    assert_eq!(second_rows[0]["session_id"], "sess_c");
    assert_eq!(second_json["pagination"]["has_more"], false);

    let hostname_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/sessions?start_date=2026-02-20&end_date=2026-02-20&filter_hostname=docs.example.com"
        ))
        .body(Body::empty())
        .expect("build request");
    let hostname_response = app
        .clone()
        .oneshot(hostname_request)
        .await
        .expect("request");
    assert_eq!(hostname_response.status(), StatusCode::OK);
    let hostname_json = json_body(hostname_response).await;
    let hostname_rows = hostname_json["data"].as_array().expect("rows array");
    assert_eq!(hostname_rows.len(), 1);
    assert_eq!(hostname_rows[0]["session_id"], "sess_a");

    let filtered_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/sessions?start_date=2026-02-20&end_date=2026-02-20&filter_page=checkout"
        ))
        .body(Body::empty())
        .expect("build request");
    let filtered_response = app
        .clone()
        .oneshot(filtered_request)
        .await
        .expect("request");
    assert_eq!(filtered_response.status(), StatusCode::OK);
    let filtered_json = json_body(filtered_response).await;
    let filtered_rows = filtered_json["data"].as_array().expect("rows array");
    assert_eq!(filtered_rows.len(), 1);
    assert_eq!(filtered_rows[0]["session_id"], "sess_a");
    assert_eq!(filtered_rows[0]["event_count"], 3);
    assert_eq!(
        filtered_rows[0]["exit_page"],
        "https://docs.example.com/thank-you"
    );
}

#[tokio::test]
async fn test_sessions_list_validates_limit_and_cursor() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let too_big_limit = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/sessions?limit=999"))
        .body(Body::empty())
        .expect("build request");
    let too_big_limit_response = app.clone().oneshot(too_big_limit).await.expect("request");
    assert_eq!(too_big_limit_response.status(), StatusCode::BAD_REQUEST);

    let invalid_cursor = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/sessions?cursor=not!!base64"
        ))
        .body(Body::empty())
        .expect("build request");
    let invalid_cursor_response = app.clone().oneshot(invalid_cursor).await.expect("request");
    assert_eq!(invalid_cursor_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_session_detail_returns_ordered_timeline_and_404() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    seed_sessions_data(&state, &website_id).await;

    let detail_request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/sessions/sess_a"))
        .body(Body::empty())
        .expect("build request");
    let detail_response = app.clone().oneshot(detail_request).await.expect("request");
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_json = json_body(detail_response).await;
    let events = detail_json["data"]["events"]
        .as_array()
        .expect("events array");
    assert_eq!(events.len(), 3);
    let ts0 = events[0]["created_at"].as_str().expect("ts0");
    let ts1 = events[1]["created_at"].as_str().expect("ts1");
    let ts2 = events[2]["created_at"].as_str().expect("ts2");
    assert!(ts0 <= ts1);
    assert!(ts1 <= ts2);
    assert!(events.iter().any(|row| row["event_name"] == "purchase"));

    let missing_request = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/sessions/sess_missing"))
        .body(Body::empty())
        .expect("build request");
    let missing_response = app.clone().oneshot(missing_request).await.expect("request");
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_sessions_endpoints_require_auth_in_local_mode() {
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
        .uri("/api/websites/site_any/sessions")
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
