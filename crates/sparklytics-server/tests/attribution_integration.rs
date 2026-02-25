use std::collections::HashMap;
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

fn config() -> Config {
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

async fn setup() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let state = Arc::new(AppState::new(db, config()));
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
    let req = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": "Attribution Test", "domain": "example.com" }).to_string(),
        ))
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("create website");
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    body["data"]["id"].as_str().expect("website id").to_string()
}

async fn create_event_goal(
    app: &axum::Router,
    website_id: &str,
    payload: Value,
) -> String {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/goals"))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("create goal");
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    body["data"]["id"].as_str().expect("goal id").to_string()
}

fn insert_session(
    conn: &sparklytics_duckdb::duckdb::Connection,
    website_id: &str,
    session_id: &str,
    visitor_id: &str,
) {
    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES (?1, ?2, NULL, ?3, '2026-02-20 10:00:00', '2026-02-20 10:10:00', 2, 'https://example.com')",
        sparklytics_duckdb::duckdb::params![session_id, website_id, visitor_id],
    )
    .expect("insert session");
}

fn insert_event(
    conn: &sparklytics_duckdb::duckdb::Connection,
    website_id: &str,
    id: &str,
    session_id: &str,
    visitor_id: &str,
    event_type: &str,
    event_name: Option<&str>,
    event_data: Option<&str>,
    utm_source: Option<&str>,
    utm_medium: Option<&str>,
    referrer_domain: Option<&str>,
    created_at: &str,
) {
    conn.execute(
        r#"
        INSERT INTO events (
            id, website_id, tenant_id, session_id, visitor_id, event_type, url,
            referrer_url, referrer_domain, event_name, event_data, country, region, city,
            browser, browser_version, os, os_version, device_type, screen, language,
            utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, ?5, 'https://example.com/checkout',
            NULL, ?6, ?7, ?8, 'US', 'CA', 'San Francisco',
            'Chrome', NULL, 'macOS', NULL, 'desktop', '1440x900', 'en-US',
            ?9, ?10, NULL, NULL, NULL, ?11
        )
        "#,
        sparklytics_duckdb::duckdb::params![
            id,
            website_id,
            session_id,
            visitor_id,
            event_type,
            referrer_domain,
            event_name,
            event_data,
            utm_source,
            utm_medium,
            created_at
        ],
    )
    .expect("insert event");
}

async fn seed_fixed_value_fixture(state: &AppState, website_id: &str) {
    let conn = state.db.conn_for_test().await;

    insert_session(&conn, website_id, "sess_fixed_a", "visitor_fixed_a");
    insert_session(&conn, website_id, "sess_fixed_b", "visitor_fixed_b");

    insert_event(
        &conn,
        website_id,
        "evt_fixed_a1",
        "sess_fixed_a",
        "visitor_fixed_a",
        "pageview",
        None,
        None,
        Some("google"),
        Some("cpc"),
        None,
        "2026-02-20 10:00:10",
    );
    insert_event(
        &conn,
        website_id,
        "evt_fixed_a2",
        "sess_fixed_a",
        "visitor_fixed_a",
        "event",
        Some("purchase"),
        Some(r#"{"plan":"pro"}"#),
        Some("newsletter"),
        Some("email"),
        None,
        "2026-02-20 10:05:10",
    );

    insert_event(
        &conn,
        website_id,
        "evt_fixed_b1",
        "sess_fixed_b",
        "visitor_fixed_b",
        "pageview",
        None,
        None,
        Some("bing"),
        Some("cpc"),
        None,
        "2026-02-20 11:00:10",
    );
    insert_event(
        &conn,
        website_id,
        "evt_fixed_b2",
        "sess_fixed_b",
        "visitor_fixed_b",
        "pageview",
        None,
        None,
        Some("reddit"),
        Some("social"),
        None,
        "2026-02-20 11:02:10",
    );
    insert_event(
        &conn,
        website_id,
        "evt_fixed_b3",
        "sess_fixed_b",
        "visitor_fixed_b",
        "event",
        Some("purchase"),
        Some(r#"{"plan":"starter"}"#),
        Some("reddit"),
        Some("social"),
        None,
        "2026-02-20 11:04:10",
    );
}

async fn seed_event_property_fixture(state: &AppState, website_id: &str) {
    let conn = state.db.conn_for_test().await;

    insert_session(&conn, website_id, "sess_prop_a", "visitor_prop_a");
    insert_session(&conn, website_id, "sess_prop_b", "visitor_prop_b");

    insert_event(
        &conn,
        website_id,
        "evt_prop_a1",
        "sess_prop_a",
        "visitor_prop_a",
        "pageview",
        None,
        None,
        Some("google"),
        Some("cpc"),
        None,
        "2026-02-20 12:00:10",
    );
    insert_event(
        &conn,
        website_id,
        "evt_prop_a2",
        "sess_prop_a",
        "visitor_prop_a",
        "event",
        Some("purchase"),
        Some(r#"{"amount":"19.95"}"#),
        Some("google"),
        Some("cpc"),
        None,
        "2026-02-20 12:03:10",
    );

    insert_event(
        &conn,
        website_id,
        "evt_prop_b1",
        "sess_prop_b",
        "visitor_prop_b",
        "pageview",
        None,
        None,
        Some("google"),
        Some("cpc"),
        None,
        "2026-02-20 12:10:10",
    );
    insert_event(
        &conn,
        website_id,
        "evt_prop_b2",
        "sess_prop_b",
        "visitor_prop_b",
        "event",
        Some("purchase"),
        Some(r#"{"plan":"free"}"#),
        Some("google"),
        Some("cpc"),
        None,
        "2026-02-20 12:13:10",
    );
}

fn rows_to_map(rows: &[Value]) -> HashMap<String, (i64, f64)> {
    rows.iter()
        .map(|row| {
            (
                row["channel"].as_str().expect("channel").to_string(),
                (
                    row["conversions"].as_i64().expect("conversions"),
                    row["revenue"].as_f64().expect("revenue"),
                ),
            )
        })
        .collect()
}

#[tokio::test]
async fn attribution_models_and_fixed_value_revenue_are_correct() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;
    seed_fixed_value_fixture(&state, &website_id).await;

    let goal_id = create_event_goal(
        &app,
        &website_id,
        json!({
            "name": "Purchase fixed",
            "goal_type": "event",
            "match_value": "purchase",
            "match_operator": "equals",
            "value_mode": "fixed",
            "fixed_value": 50.0,
            "currency": "USD"
        }),
    )
    .await;

    let first_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/attribution?goal_id={goal_id}&model=first_touch&start_date=2026-02-20&end_date=2026-02-20"
        ))
        .body(Body::empty())
        .expect("request");
    let first_res = app.clone().oneshot(first_req).await.expect("response");
    assert_eq!(first_res.status(), StatusCode::OK);
    let first_body = json_body(first_res).await;
    let first_rows = first_body["data"]["rows"].as_array().expect("rows");
    let first_map = rows_to_map(first_rows);
    assert_eq!(first_map.get("google / cpc"), Some(&(1, 50.0)));
    assert_eq!(first_map.get("bing / cpc"), Some(&(1, 50.0)));
    assert_eq!(first_body["data"]["totals"]["conversions"], 2);
    assert_eq!(first_body["data"]["totals"]["revenue"], 100.0);

    let last_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/attribution?goal_id={goal_id}&model=last_touch&start_date=2026-02-20&end_date=2026-02-20"
        ))
        .body(Body::empty())
        .expect("request");
    let last_res = app.clone().oneshot(last_req).await.expect("response");
    assert_eq!(last_res.status(), StatusCode::OK);
    let last_body = json_body(last_res).await;
    let last_rows = last_body["data"]["rows"].as_array().expect("rows");
    let last_map = rows_to_map(last_rows);
    assert_eq!(last_map.get("newsletter / email"), Some(&(1, 50.0)));
    assert_eq!(last_map.get("reddit / social"), Some(&(1, 50.0)));
    assert_eq!(last_body["data"]["totals"]["conversions"], 2);
    assert_eq!(last_body["data"]["totals"]["revenue"], 100.0);

    let summary_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/revenue/summary?goal_id={goal_id}&model=last_touch&start_date=2026-02-20&end_date=2026-02-20"
        ))
        .body(Body::empty())
        .expect("request");
    let summary_res = app.clone().oneshot(summary_req).await.expect("response");
    assert_eq!(summary_res.status(), StatusCode::OK);
    let summary_body = json_body(summary_res).await;
    assert_eq!(summary_body["data"]["conversions"], 2);
    assert_eq!(summary_body["data"]["revenue"], 100.0);
}

#[tokio::test]
async fn attribution_event_property_missing_value_falls_back_to_zero_revenue() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;
    seed_event_property_fixture(&state, &website_id).await;

    let goal_id = create_event_goal(
        &app,
        &website_id,
        json!({
            "name": "Purchase property",
            "goal_type": "event",
            "match_value": "purchase",
            "match_operator": "equals",
            "value_mode": "event_property",
            "value_property_key": "amount",
            "currency": "USD"
        }),
    )
    .await;

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/attribution?goal_id={goal_id}&model=last_touch&start_date=2026-02-20&end_date=2026-02-20"
        ))
        .body(Body::empty())
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    assert_eq!(body["data"]["totals"]["conversions"], 2);
    assert_eq!(body["data"]["totals"]["revenue"], 19.95);
    let rows = body["data"]["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["channel"], "google / cpc");
    assert_eq!(rows[0]["conversions"], 2);
    assert_eq!(rows[0]["revenue"], 19.95);
}

#[tokio::test]
async fn attribution_rejects_invalid_model_and_missing_goal() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let invalid_model_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/attribution?goal_id=goal_x&model=middle_touch"
        ))
        .body(Body::empty())
        .expect("request");
    let invalid_model_res = app
        .clone()
        .oneshot(invalid_model_req)
        .await
        .expect("response");
    assert_eq!(invalid_model_res.status(), StatusCode::BAD_REQUEST);

    let missing_goal_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/attribution?goal_id=goal_missing&model=last_touch"
        ))
        .body(Body::empty())
        .expect("request");
    let missing_goal_res = app
        .clone()
        .oneshot(missing_goal_req)
        .await
        .expect("response");
    assert_eq!(missing_goal_res.status(), StatusCode::NOT_FOUND);
}
