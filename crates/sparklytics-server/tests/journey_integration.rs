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

#[allow(clippy::too_many_arguments)]
async fn insert_event_row(
    state: &AppState,
    website_id: &str,
    event_id: &str,
    session_id: &str,
    event_type: &str,
    url: &str,
    event_name: Option<&str>,
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
            ?1, ?2, NULL, ?3, ?4, ?5, ?6,
            NULL, NULL, ?7, NULL, 'US', NULL, NULL,
            'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
            NULL, NULL, NULL, NULL, NULL, ?8
        )
        "#,
        sparklytics_duckdb::duckdb::params![
            event_id,
            website_id,
            session_id,
            format!("visitor_{session_id}"),
            event_type,
            url,
            event_name,
            created_at
        ],
    )
    .expect("insert event");
}

async fn seed_journey_events(state: &AppState, website_id: &str, day: chrono::NaiveDate) {
    // Session 1: /pricing -> /signup -> signup_clicked
    insert_event_row(
        state,
        website_id,
        "evt_1",
        "sess_1",
        "pageview",
        "/home",
        None,
        &format!("{} 10:00:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_2",
        "sess_1",
        "pageview",
        "/pricing?utm_source=ad#hero",
        None,
        &format!("{} 10:01:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_3",
        "sess_1",
        "pageview",
        "/signup",
        None,
        &format!("{} 10:02:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_4",
        "sess_1",
        "event",
        "/signup",
        Some("signup_clicked"),
        &format!("{} 10:03:00", day),
    )
    .await;

    // Session 2: /pricing -> /features
    insert_event_row(
        state,
        website_id,
        "evt_5",
        "sess_2",
        "pageview",
        "/landing",
        None,
        &format!("{} 11:00:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_6",
        "sess_2",
        "pageview",
        "/pricing/",
        None,
        &format!("{} 11:01:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_7",
        "sess_2",
        "pageview",
        "/features",
        None,
        &format!("{} 11:02:00", day),
    )
    .await;

    // Session 3: /pricing with no next step
    insert_event_row(
        state,
        website_id,
        "evt_8",
        "sess_3",
        "pageview",
        "/pricing",
        None,
        &format!("{} 12:00:00", day),
    )
    .await;

    // Session 4: /home -> signup_clicked (used for previous direction)
    insert_event_row(
        state,
        website_id,
        "evt_9",
        "sess_4",
        "pageview",
        "/home",
        None,
        &format!("{} 13:00:00", day),
    )
    .await;
    insert_event_row(
        state,
        website_id,
        "evt_10",
        "sess_4",
        "event",
        "/home",
        Some("signup_clicked"),
        &format!("{} 13:01:00", day),
    )
    .await;
}

#[tokio::test]
async fn test_journey_endpoint_returns_next_and_previous_branches() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let day = chrono::Utc::now().date_naive();
    let date = day.format("%Y-%m-%d").to_string();

    seed_journey_events(&state, &website_id, day).await;

    let next_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/journey?anchor_type=page&anchor_value=%2FPricing%2F%3Fa%3D1%23x&direction=next&max_depth=2&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let next_response = app.clone().oneshot(next_request).await.expect("request");
    assert_eq!(next_response.status(), StatusCode::OK);

    let next_json = json_body(next_response).await;
    assert_eq!(next_json["data"]["anchor"]["type"], "page");
    assert_eq!(next_json["data"]["anchor"]["value"], "/pricing");
    assert_eq!(next_json["data"]["total_anchor_sessions"], 3);

    let next_branches = next_json["data"]["branches"].as_array().expect("branches");
    assert!(next_branches
        .iter()
        .any(|branch| branch["nodes"] == json!([])));
    assert!(next_branches
        .iter()
        .any(|branch| branch["nodes"] == json!(["/features"])));
    assert!(next_branches
        .iter()
        .any(|branch| branch["nodes"] == json!(["/signup", "signup_clicked"])));

    let previous_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/journey?anchor_type=event&anchor_value=signup_clicked&direction=previous&max_depth=1&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let previous_response = app
        .clone()
        .oneshot(previous_request)
        .await
        .expect("request");
    assert_eq!(previous_response.status(), StatusCode::OK);

    let previous_json = json_body(previous_response).await;
    assert_eq!(previous_json["data"]["anchor"]["type"], "event");
    assert_eq!(previous_json["data"]["total_anchor_sessions"], 2);

    let previous_branches = previous_json["data"]["branches"]
        .as_array()
        .expect("branches");
    assert!(previous_branches
        .iter()
        .any(|branch| branch["nodes"] == json!(["/home"])));
    assert!(previous_branches
        .iter()
        .any(|branch| branch["nodes"] == json!(["/signup"])));
}

#[tokio::test]
async fn test_journey_endpoint_validates_query_params() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let date = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    let missing_anchor_type = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/journey?anchor_value=%2Fpricing&direction=next&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app
        .clone()
        .oneshot(missing_anchor_type)
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let invalid_depth = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/journey?anchor_type=page&anchor_value=%2Fpricing&direction=next&max_depth=6&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(invalid_depth).await.expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let invalid_timezone = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/journey?anchor_type=page&anchor_value=%2Fpricing&direction=next&timezone=Invalid%2FTimezone&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app
        .clone()
        .oneshot(invalid_timezone)
        .await
        .expect("request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_journey_endpoint_returns_404_for_unknown_website() {
    let (_state, app) = setup_none().await;

    let date = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/site_missing/journey?anchor_type=page&anchor_value=%2Fpricing&direction=next&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_journey_endpoint_requires_auth_in_local_mode() {
    let (_state, app) = setup_auth().await;

    let setup_request = Request::builder()
        .method("POST")
        .uri("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "password": TEST_PASSWORD }).to_string()))
        .expect("build request");
    let setup_response = app.clone().oneshot(setup_request).await.expect("request");
    assert_eq!(setup_response.status(), StatusCode::CREATED);

    let date = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/site_any/journey?anchor_type=page&anchor_value=%2Fpricing&direction=next&start_date={date}&end_date={date}"
        ))
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
