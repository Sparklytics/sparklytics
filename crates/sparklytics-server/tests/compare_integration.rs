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
            json!({ "name": "Compare Test", "domain": "example.com" }).to_string(),
        ))
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("create website");
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    body["data"]["id"].as_str().expect("website id").to_string()
}

async fn seed_compare_data(state: &AppState, website_id: &str) {
    let conn = state.db.conn_for_test().await;

    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_primary_a', ?1, NULL, 'visitor_a', '2026-02-10 10:00:00', '2026-02-10 10:03:00', 2, 'https://example.com/pricing')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("insert primary session a");

    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_primary_b', ?1, NULL, 'visitor_b', '2026-02-11 11:00:00', '2026-02-11 11:01:00', 1, 'https://example.com/contact')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("insert primary session b");

    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_compare_a', ?1, NULL, 'visitor_c', '2026-02-08 09:00:00', '2026-02-08 09:01:00', 1, 'https://example.com/contact')",
        sparklytics_duckdb::duckdb::params![website_id],
    )
    .expect("insert compare session");

    let insert_event = |id: &str,
                        session_id: &str,
                        visitor_id: &str,
                        url: &str,
                        utm_source: Option<&str>,
                        created_at: &str| {
        conn.execute(
            r#"
            INSERT INTO events (
                id, website_id, tenant_id, session_id, visitor_id, event_type, url,
                referrer_url, referrer_domain, event_name, event_data, country, region, city,
                browser, browser_version, os, os_version, device_type, screen, language,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
            ) VALUES (
                ?1, ?2, NULL, ?3, ?4, 'pageview', ?5,
                NULL, NULL, NULL, NULL, 'US', 'CA', 'San Francisco',
                'Chrome', NULL, 'macOS', NULL, 'desktop', '1440x900', 'en-US',
                ?6, NULL, NULL, NULL, NULL, ?7
            )
            "#,
            sparklytics_duckdb::duckdb::params![
                id,
                website_id,
                session_id,
                visitor_id,
                url,
                utm_source,
                created_at
            ],
        )
        .expect("insert event");
    };

    insert_event(
        "evt_primary_1",
        "sess_primary_a",
        "visitor_a",
        "https://example.com/pricing",
        Some("google"),
        "2026-02-10 10:00:10",
    );
    insert_event(
        "evt_primary_2",
        "sess_primary_a",
        "visitor_a",
        "https://example.com/pricing",
        Some("google"),
        "2026-02-10 10:02:00",
    );
    insert_event(
        "evt_primary_3",
        "sess_primary_b",
        "visitor_b",
        "https://example.com/contact",
        Some("twitter"),
        "2026-02-11 11:00:30",
    );
    insert_event(
        "evt_compare_1",
        "sess_compare_a",
        "visitor_c",
        "https://example.com/contact",
        Some("twitter"),
        "2026-02-08 09:00:40",
    );
}

#[tokio::test]
async fn compare_mode_stats_and_pageviews_include_metadata() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;
    seed_compare_data(&state, &website_id).await;

    let stats_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/stats?start_date=2026-02-10&end_date=2026-02-11&compare_mode=previous_period"
        ))
        .body(Body::empty())
        .expect("stats request");
    let stats_res = app.clone().oneshot(stats_req).await.expect("stats response");
    assert_eq!(stats_res.status(), StatusCode::OK);
    let stats_json = json_body(stats_res).await;
    assert_eq!(stats_json["compare"]["mode"], "previous_period");
    assert_eq!(stats_json["compare"]["comparison_range"][0], "2026-02-08");
    assert!(stats_json["data"]["prev_pageviews"].is_number());

    let pageviews_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/pageviews?start_date=2026-02-10&end_date=2026-02-11&compare_mode=previous_period"
        ))
        .body(Body::empty())
        .expect("pageviews request");
    let pageviews_res = app
        .clone()
        .oneshot(pageviews_req)
        .await
        .expect("pageviews response");
    assert_eq!(pageviews_res.status(), StatusCode::OK);
    let pageviews_json = json_body(pageviews_res).await;
    assert_eq!(pageviews_json["compare"]["mode"], "previous_period");
    let primary = pageviews_json["data"]["series"].as_array().expect("series");
    let compare = pageviews_json["data"]["compare_series"]
        .as_array()
        .expect("compare series");
    assert_eq!(primary.len(), compare.len());
}

#[tokio::test]
async fn compare_mode_metrics_missing_dimension_uses_zero_previous() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;
    seed_compare_data(&state, &website_id).await;

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/metrics?type=page&start_date=2026-02-10&end_date=2026-02-11&compare_mode=previous_period"
        ))
        .body(Body::empty())
        .expect("metrics request");
    let res = app.clone().oneshot(req).await.expect("metrics response");
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    let rows = body["data"]["rows"].as_array().expect("rows");
    let pricing = rows
        .iter()
        .find(|row| row["value"] == "https://example.com/pricing")
        .expect("pricing row");

    assert_eq!(pricing["prev_visitors"], 0);
    assert_eq!(pricing["prev_pageviews"], 0);
    assert_eq!(body["compare"]["mode"], "previous_period");
}

#[tokio::test]
async fn compare_mode_custom_requires_both_dates() {
    let (_state, app) = setup().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/stats?start_date=2026-02-10&end_date=2026-02-11&compare_mode=custom&compare_start_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("request");
    let res = app.clone().oneshot(req).await.expect("response");
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
