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

async fn seed_goal_stats_data(state: &AppState, website_id: &str, start_date: chrono::NaiveDate) {
    let conn = state.db.conn_for_test().await;

    let end_date = start_date + chrono::Duration::days(1);
    let prev_end = start_date - chrono::Duration::days(1);
    let prev_start = prev_end - chrono::Duration::days(1);

    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_cur_1', ?1, NULL, 'v1', ?2, ?3, 2, 'https://example.com/checkout')",
        sparklytics_duckdb::duckdb::params![
            website_id,
            format!("{} 10:00:00", start_date),
            format!("{} 10:10:00", start_date),
        ],
    )
    .expect("insert current session 1");
    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_cur_2', ?1, NULL, 'v2', ?2, ?3, 1, 'https://example.com/pricing')",
        sparklytics_duckdb::duckdb::params![
            website_id,
            format!("{} 11:00:00", end_date),
            format!("{} 11:05:00", end_date),
        ],
    )
    .expect("insert current session 2");
    conn.execute(
        "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
         VALUES ('sess_prev_1', ?1, NULL, 'v3', ?2, ?3, 1, 'https://example.com/checkout')",
        sparklytics_duckdb::duckdb::params![
            website_id,
            format!("{} 09:00:00", prev_start),
            format!("{} 09:05:00", prev_end),
        ],
    )
    .expect("insert previous session");

    let insert_event = |id: &str,
                        session_id: &str,
                        event_type: &str,
                        event_name: Option<&str>,
                        created_at: String| {
        conn.execute(
            r#"
            INSERT INTO events (
                id, website_id, tenant_id, session_id, visitor_id, event_type, url,
                referrer_url, referrer_domain, event_name, event_data, country, region, city,
                browser, browser_version, os, os_version, device_type, screen, language,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
            ) VALUES (
                ?1, ?2, NULL, ?3, ?4, ?5, 'https://example.com/checkout',
                NULL, NULL, ?6, NULL, 'PL', 'Mazowieckie', 'Warsaw',
                'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
                NULL, NULL, NULL, NULL, NULL, ?7
            )
            "#,
            sparklytics_duckdb::duckdb::params![
                id,
                website_id,
                session_id,
                format!("visitor_{session_id}"),
                event_type,
                event_name,
                created_at
            ],
        )
        .expect("insert event");
    };

    insert_event(
        "evt_cur_1",
        "sess_cur_1",
        "event",
        Some("purchase"),
        format!("{} 10:01:00", start_date),
    );
    insert_event(
        "evt_cur_2",
        "sess_cur_1",
        "event",
        Some("purchase"),
        format!("{} 10:02:00", start_date),
    );
    insert_event(
        "evt_cur_3",
        "sess_cur_2",
        "pageview",
        None,
        format!("{} 11:00:00", end_date),
    );
    insert_event(
        "evt_prev_1",
        "sess_prev_1",
        "event",
        Some("purchase"),
        format!("{} 09:00:00", prev_start),
    );
}

#[tokio::test]
async fn test_goals_crud_and_idempotent_delete() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/goals"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Purchase Completed",
                "goal_type": "event",
                "match_value": "purchase",
                "match_operator": "equals"
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let goal_id = create_json["data"]["id"]
        .as_str()
        .expect("goal id")
        .to_string();

    let list_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/goals"))
        .body(Body::empty())
        .expect("build request");
    let list_res = app.clone().oneshot(list_req).await.expect("request");
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_json = json_body(list_res).await;
    assert_eq!(list_json["data"].as_array().expect("array").len(), 1);

    let update_req = Request::builder()
        .method("PUT")
        .uri(format!("/api/websites/{website_id}/goals/{goal_id}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Purchase Success",
                "match_value": "purchase",
                "match_operator": "contains"
            })
            .to_string(),
        ))
        .expect("build request");
    let update_res = app.clone().oneshot(update_req).await.expect("request");
    assert_eq!(update_res.status(), StatusCode::OK);
    let update_json = json_body(update_res).await;
    assert_eq!(update_json["data"]["name"], "Purchase Success");
    assert_eq!(update_json["data"]["match_operator"], "contains");

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/goals/{goal_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_res = app.clone().oneshot(delete_req).await.expect("request");
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let delete_again_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/goals/{goal_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_again_res = app
        .clone()
        .oneshot(delete_again_req)
        .await
        .expect("request");
    assert_eq!(delete_again_res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_goals_duplicate_name_returns_422() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let create = |name: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/api/websites/{website_id}/goals"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": name,
                    "goal_type": "event",
                    "match_value": "purchase",
                    "match_operator": "equals"
                })
                .to_string(),
            ))
            .expect("build request")
    };

    let first = app
        .clone()
        .oneshot(create("Purchase"))
        .await
        .expect("request");
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .clone()
        .oneshot(create("Purchase"))
        .await
        .expect("request");
    assert_eq!(second.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json_body(second).await;
    assert_eq!(body["error"]["code"], "duplicate_name");
}

#[tokio::test]
async fn test_goal_stats_semantics_and_not_found() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let start = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    let end = chrono::Utc::now().date_naive();
    seed_goal_stats_data(&state, &website_id, start).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/goals"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Purchase",
                "goal_type": "event",
                "match_value": "purchase",
                "match_operator": "equals"
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let goal_id = create_json["data"]["id"].as_str().expect("goal id");

    let stats_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/goals/{goal_id}/stats?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");
    let stats_res = app.clone().oneshot(stats_req).await.expect("request");
    assert_eq!(stats_res.status(), StatusCode::OK);
    let stats_json = json_body(stats_res).await;
    assert_eq!(stats_json["data"]["conversions"], 2);
    assert_eq!(stats_json["data"]["converting_sessions"], 1);
    assert_eq!(stats_json["data"]["total_sessions"], 2);
    let rate = stats_json["data"]["conversion_rate"]
        .as_f64()
        .expect("conversion_rate");
    assert!((rate - 0.5).abs() < 0.0001);
    assert_eq!(stats_json["data"]["prev_conversions"], 1);

    let missing_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/goals/goal_does_not_exist/stats?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("build request");
    let missing_res = app.clone().oneshot(missing_req).await.expect("request");
    assert_eq!(missing_res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_goals_deleted_with_website() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/goals"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Purchase",
                "goal_type": "event",
                "match_value": "purchase",
                "match_operator": "equals"
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);

    let delete_site_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_site_res = app.clone().oneshot(delete_site_req).await.expect("request");
    assert_eq!(delete_site_res.status(), StatusCode::NO_CONTENT);

    let conn = state.db.conn_for_test().await;
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1")
        .expect("prepare")
        .query_row(sparklytics_duckdb::duckdb::params![&website_id], |row| {
            row.get(0)
        })
        .expect("query");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_goal_stats_returns_null_previous_fields_when_no_previous_data() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let today = chrono::Utc::now().date_naive();
    {
        let conn = state.db.conn_for_test().await;
        conn.execute(
            "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page)
             VALUES ('sess_only_current', ?1, NULL, 'v_only', ?2, ?3, 1, 'https://example.com/checkout')",
            sparklytics_duckdb::duckdb::params![
                &website_id,
                format!("{today} 10:00:00"),
                format!("{today} 10:05:00"),
            ],
        )
        .expect("insert current session");
        conn.execute(
            r#"
            INSERT INTO events (
                id, website_id, tenant_id, session_id, visitor_id, event_type, url,
                referrer_url, referrer_domain, event_name, event_data, country, region, city,
                browser, browser_version, os, os_version, device_type, screen, language,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
            ) VALUES (
                'evt_only_current', ?1, NULL, 'sess_only_current', 'v_only', 'event', 'https://example.com/checkout',
                NULL, NULL, 'purchase', NULL, 'PL', 'Mazowieckie', 'Warsaw',
                'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
                NULL, NULL, NULL, NULL, NULL, ?2
            )
            "#,
            sparklytics_duckdb::duckdb::params![&website_id, format!("{today} 10:01:00")],
        )
        .expect("insert current event");
    }

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/goals"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Purchase",
                "goal_type": "event",
                "match_value": "purchase",
                "match_operator": "equals"
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let goal_id = create_json["data"]["id"].as_str().expect("goal id");

    let stats_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/goals/{goal_id}/stats?start_date={today}&end_date={today}"
        ))
        .body(Body::empty())
        .expect("build request");
    let stats_res = app.clone().oneshot(stats_req).await.expect("request");
    assert_eq!(stats_res.status(), StatusCode::OK);
    let stats_json = json_body(stats_res).await;

    assert!(stats_json["data"]["prev_conversions"].is_null());
    assert!(stats_json["data"]["prev_conversion_rate"].is_null());
    assert!(stats_json["data"]["trend_pct"].is_null());
}

#[tokio::test]
async fn test_goals_endpoints_require_auth_in_local_mode() {
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
        .uri("/api/websites/site_any/goals")
        .body(Body::empty())
        .expect("build request");
    let response = app.clone().oneshot(request).await.expect("request");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
