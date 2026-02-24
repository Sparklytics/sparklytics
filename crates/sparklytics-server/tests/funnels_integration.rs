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

fn create_funnel_request(name: &str) -> Value {
    json!({
        "name": name,
        "steps": [
            {
                "step_type": "page_view",
                "match_value": "/pricing",
                "match_operator": "equals",
                "label": "Pricing"
            },
            {
                "step_type": "event",
                "match_value": "signup_completed",
                "match_operator": "equals",
                "label": "Signup Completed"
            }
        ]
    })
}

async fn seed_funnel_events(state: &AppState, website_id: &str, day: chrono::NaiveDate) {
    let conn = state.db.conn_for_test().await;

    let insert_event = |id: &str,
                        session_id: &str,
                        event_type: &str,
                        url: &str,
                        event_name: Option<&str>,
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
                NULL, NULL, ?7, NULL, 'US', NULL, NULL,
                'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
                NULL, NULL, NULL, NULL, NULL, ?8
            )
            "#,
            sparklytics_duckdb::duckdb::params![
                id,
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
    };

    // Session 1: reaches both steps in order.
    insert_event(
        "evt_1",
        "sess_1",
        "pageview",
        "/pricing",
        None,
        &format!("{} 10:00:00", day),
    );
    insert_event(
        "evt_2",
        "sess_1",
        "event",
        "/signup",
        Some("signup_completed"),
        &format!("{} 10:02:00", day),
    );

    // Session 2: out-of-order; should not reach step 2 after reaching step 1.
    insert_event(
        "evt_3",
        "sess_2",
        "event",
        "/signup",
        Some("signup_completed"),
        &format!("{} 11:00:00", day),
    );
    insert_event(
        "evt_4",
        "sess_2",
        "pageview",
        "/pricing",
        None,
        &format!("{} 11:02:00", day),
    );
}

async fn insert_event_row(
    state: &AppState,
    website_id: &str,
    event_id: &str,
    session_id: &str,
    event_type: &str,
    url: &str,
    event_name: Option<&str>,
    browser: &str,
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
            ?1, ?2, NULL, ?3, ?4, ?5, ?6,
            NULL, NULL, ?7, NULL, ?8, NULL, NULL,
            ?9, NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
            NULL, NULL, NULL, NULL, NULL, ?10
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
            country,
            browser,
            created_at
        ],
    )
    .expect("insert event");
}

#[tokio::test]
async fn test_funnels_crud_and_delete_not_found_after_removal() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            create_funnel_request("Signup Funnel").to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let list_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .body(Body::empty())
        .expect("build request");
    let list_res = app.clone().oneshot(list_req).await.expect("request");
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_json = json_body(list_res).await;
    assert_eq!(list_json["data"].as_array().expect("array").len(), 1);

    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/funnels/{funnel_id}"))
        .body(Body::empty())
        .expect("build request");
    let get_res = app.clone().oneshot(get_req).await.expect("request");
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_json = json_body(get_res).await;
    assert_eq!(get_json["data"]["name"], "Signup Funnel");

    let update_req = Request::builder()
        .method("PUT")
        .uri(format!("/api/websites/{website_id}/funnels/{funnel_id}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Signup Funnel Updated"
            })
            .to_string(),
        ))
        .expect("build request");
    let update_res = app.clone().oneshot(update_req).await.expect("request");
    assert_eq!(update_res.status(), StatusCode::OK);
    let update_json = json_body(update_res).await;
    assert_eq!(update_json["data"]["name"], "Signup Funnel Updated");

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/funnels/{funnel_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_res = app.clone().oneshot(delete_req).await.expect("request");
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    let delete_again_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/funnels/{funnel_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_again_res = app
        .clone()
        .oneshot(delete_again_req)
        .await
        .expect("request");
    assert_eq!(delete_again_res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_funnels_duplicate_name_and_step_validation() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let first_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            create_funnel_request("Checkout Funnel").to_string(),
        ))
        .expect("build request");
    let first_res = app.clone().oneshot(first_req).await.expect("request");
    assert_eq!(first_res.status(), StatusCode::CREATED);

    let dup_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            create_funnel_request("Checkout Funnel").to_string(),
        ))
        .expect("build request");
    let dup_res = app.clone().oneshot(dup_req).await.expect("request");
    assert_eq!(dup_res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let dup_json = json_body(dup_res).await;
    assert_eq!(dup_json["error"]["code"], "duplicate_name");

    let invalid_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Invalid",
                "steps": [
                    {
                        "step_type": "page_view",
                        "match_value": "/pricing",
                        "match_operator": "equals"
                    }
                ]
            })
            .to_string(),
        ))
        .expect("build request");
    let invalid_res = app.clone().oneshot(invalid_req).await.expect("request");
    assert_eq!(invalid_res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_funnel_results_respect_ordering() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let day = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    seed_funnel_events(&state, &website_id, day).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(create_funnel_request("Signup").to_string()))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let results_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={day}&end_date={day}"
        ))
        .body(Body::empty())
        .expect("build request");
    let results_res = app.clone().oneshot(results_req).await.expect("request");
    assert_eq!(results_res.status(), StatusCode::OK);
    let results_json = json_body(results_res).await;

    assert_eq!(results_json["data"]["total_sessions_entered"], 2);
    assert_eq!(results_json["data"]["steps"][0]["sessions_reached"], 2);
    assert_eq!(results_json["data"]["steps"][1]["sessions_reached"], 1);
    let final_rate = results_json["data"]["final_conversion_rate"]
        .as_f64()
        .expect("final conversion rate");
    assert!((final_rate - 0.5).abs() < 0.0001);

    let filtered_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={day}&end_date={day}&filter_page=signup"
        ))
        .body(Body::empty())
        .expect("build request");
    let filtered_res = app.clone().oneshot(filtered_req).await.expect("request");
    assert_eq!(filtered_res.status(), StatusCode::OK);
    let filtered_json = json_body(filtered_res).await;
    assert_eq!(filtered_json["data"]["total_sessions_entered"], 0);
    assert_eq!(filtered_json["data"]["steps"][0]["sessions_reached"], 0);
}

#[tokio::test]
async fn test_funnel_results_validate_range_and_timezone() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let old_day = chrono::Utc::now().date_naive() - chrono::Duration::days(20);
    seed_funnel_events(&state, &website_id, old_day).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(create_funnel_request("Signup").to_string()))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    // Default results range should include the previous 30 days.
    let default_range_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results"
        ))
        .body(Body::empty())
        .expect("build request");
    let default_range_res = app
        .clone()
        .oneshot(default_range_req)
        .await
        .expect("request");
    assert_eq!(default_range_res.status(), StatusCode::OK);
    let default_range_json = json_body(default_range_res).await;
    assert_eq!(default_range_json["data"]["total_sessions_entered"], 2);

    let reversed_range_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2026-01-10&end_date=2026-01-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let reversed_range_res = app
        .clone()
        .oneshot(reversed_range_req)
        .await
        .expect("request");
    assert_eq!(reversed_range_res.status(), StatusCode::BAD_REQUEST);

    let invalid_start_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2026-02-30&end_date=2026-03-01"
        ))
        .body(Body::empty())
        .expect("build request");
    let invalid_start_res = app
        .clone()
        .oneshot(invalid_start_req)
        .await
        .expect("request");
    assert_eq!(invalid_start_res.status(), StatusCode::BAD_REQUEST);

    let invalid_end_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2026-03-01&end_date=bad-date"
        ))
        .body(Body::empty())
        .expect("build request");
    let invalid_end_res = app.clone().oneshot(invalid_end_req).await.expect("request");
    assert_eq!(invalid_end_res.status(), StatusCode::BAD_REQUEST);

    let too_large_range_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2025-01-01&end_date=2025-12-31"
        ))
        .body(Body::empty())
        .expect("build request");
    let too_large_range_res = app
        .clone()
        .oneshot(too_large_range_req)
        .await
        .expect("request");
    assert_eq!(too_large_range_res.status(), StatusCode::BAD_REQUEST);

    let empty_timezone_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?timezone=%20%20"
        ))
        .body(Body::empty())
        .expect("build request");
    let empty_timezone_res = app
        .clone()
        .oneshot(empty_timezone_req)
        .await
        .expect("request");
    assert_eq!(empty_timezone_res.status(), StatusCode::BAD_REQUEST);

    let invalid_timezone_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?timezone=Not/AZone"
        ))
        .body(Body::empty())
        .expect("build request");
    let invalid_timezone_res = app
        .clone()
        .oneshot(invalid_timezone_req)
        .await
        .expect("request");
    assert_eq!(invalid_timezone_res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_funnel_results_timezone_shift_changes_window() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    {
        let conn = state.db.conn_for_test().await;
        let insert_event = |id: &str,
                            session_id: &str,
                            event_type: &str,
                            url: &str,
                            event_name: Option<&str>,
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
                    NULL, NULL, ?7, NULL, 'US', NULL, NULL,
                    'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
                    NULL, NULL, NULL, NULL, NULL, ?8
                )
                "#,
                sparklytics_duckdb::duckdb::params![
                    id,
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
        };

        // Included for America/Los_Angeles on 2026-01-10 (UTC-8):
        // local 2026-01-10 23:30 => UTC 2026-01-11 07:30.
        insert_event(
            "evt_la_in_1",
            "sess_la_in",
            "pageview",
            "/pricing",
            None,
            "2026-01-11 07:30:00",
        );
        insert_event(
            "evt_la_in_2",
            "sess_la_in",
            "event",
            "/signup",
            Some("signup_completed"),
            "2026-01-11 07:35:00",
        );

        // Excluded for America/Los_Angeles on 2026-01-10:
        // local 2026-01-11 00:30 => UTC 2026-01-11 08:30.
        insert_event(
            "evt_la_out_1",
            "sess_la_out",
            "pageview",
            "/pricing",
            None,
            "2026-01-11 08:30:00",
        );
        insert_event(
            "evt_la_out_2",
            "sess_la_out",
            "event",
            "/signup",
            Some("signup_completed"),
            "2026-01-11 08:35:00",
        );
    }

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(create_funnel_request("Signup").to_string()))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let utc_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2026-01-10&end_date=2026-01-10"
        ))
        .body(Body::empty())
        .expect("build request");
    let utc_res = app.clone().oneshot(utc_req).await.expect("request");
    assert_eq!(utc_res.status(), StatusCode::OK);
    let utc_json = json_body(utc_res).await;
    assert_eq!(utc_json["data"]["total_sessions_entered"], 0);

    let la_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date=2026-01-10&end_date=2026-01-10&timezone=America/Los_Angeles"
        ))
        .body(Body::empty())
        .expect("build request");
    let la_res = app.clone().oneshot(la_req).await.expect("request");
    assert_eq!(la_res.status(), StatusCode::OK);
    let la_json = json_body(la_res).await;
    assert_eq!(la_json["data"]["total_sessions_entered"], 1);
    assert_eq!(la_json["data"]["steps"][1]["sessions_reached"], 1);
}

#[tokio::test]
async fn test_funnel_results_filter_hostname_ignores_port() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let day = chrono::Utc::now().date_naive() - chrono::Duration::days(1);

    {
        let conn = state.db.conn_for_test().await;
        let insert_event =
            |id: &str, session_id: &str, url: &str, event_name: Option<&str>, created_at: &str| {
                let event_type = if event_name.is_some() {
                    "event"
                } else {
                    "pageview"
                };
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
                        id,
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
            };

        insert_event(
            "evt_host_1",
            "sess_host_1",
            "https://example.com:3000/pricing",
            None,
            &format!("{} 10:00:00", day),
        );
        insert_event(
            "evt_host_2",
            "sess_host_1",
            "https://example.com:3000/signup",
            Some("signup_completed"),
            &format!("{} 10:01:00", day),
        );

        insert_event(
            "evt_host_3",
            "sess_host_2",
            "https://other.com/pricing",
            None,
            &format!("{} 11:00:00", day),
        );
        insert_event(
            "evt_host_4",
            "sess_host_2",
            "https://other.com/signup",
            Some("signup_completed"),
            &format!("{} 11:01:00", day),
        );
    }

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Host Filter",
                "steps": [
                    {
                        "step_type": "page_view",
                        "match_value": "pricing",
                        "match_operator": "contains"
                    },
                    {
                        "step_type": "event",
                        "match_value": "signup_",
                        "match_operator": "contains"
                    }
                ]
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let results_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={day}&end_date={day}&filter_hostname=example.com"
        ))
        .body(Body::empty())
        .expect("build request");
    let results_res = app.clone().oneshot(results_req).await.expect("request");
    assert_eq!(results_res.status(), StatusCode::OK);
    let results_json = json_body(results_res).await;
    assert_eq!(results_json["data"]["total_sessions_entered"], 1);
    assert_eq!(results_json["data"]["steps"][1]["sessions_reached"], 1);

    let browser_filtered_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={day}&end_date={day}&filter_hostname=example.com&filter_browser=Chrome"
        ))
        .body(Body::empty())
        .expect("build request");
    let browser_filtered_res = app
        .clone()
        .oneshot(browser_filtered_req)
        .await
        .expect("request");
    assert_eq!(browser_filtered_res.status(), StatusCode::OK);
    let browser_filtered_json = json_body(browser_filtered_res).await;
    assert_eq!(browser_filtered_json["data"]["total_sessions_entered"], 1);
    assert_eq!(
        browser_filtered_json["data"]["steps"][1]["sessions_reached"],
        1
    );
}

#[tokio::test]
async fn test_funnel_results_with_rich_dataset_and_filters() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let today = chrono::Utc::now().date_naive();

    // Build a richer test dataset: 300 sessions over 45 days, each with
    // step candidates plus noise events. This keeps tests realistic while
    // still deterministic.
    let mut event_counter: usize = 0;
    for idx in 0..300 {
        let day = today - chrono::Duration::days((idx % 45) as i64);
        let session_id = format!("sess_rich_{idx}");
        let host = if idx % 3 == 0 {
            "https://example.com:3000"
        } else {
            "https://other.com"
        };
        let browser = if idx % 2 == 0 { "Chrome" } else { "Firefox" };
        let country = if idx % 5 == 0 { "US" } else { "DE" };

        event_counter += 1;
        insert_event_row(
            &state,
            &website_id,
            &format!("evt_rich_{event_counter}"),
            &session_id,
            "pageview",
            &format!("{host}/pricing"),
            None,
            browser,
            country,
            &format!("{day} 10:00:00"),
        )
        .await;

        // Every 4th session completes step 2.
        if idx % 4 == 0 {
            event_counter += 1;
            insert_event_row(
                &state,
                &website_id,
                &format!("evt_rich_{event_counter}"),
                &session_id,
                "event",
                &format!("{host}/signup"),
                Some("signup_completed"),
                browser,
                country,
                &format!("{day} 10:01:00"),
            )
            .await;
        }

        // Noise events that should not affect funnel matching.
        event_counter += 1;
        insert_event_row(
            &state,
            &website_id,
            &format!("evt_rich_{event_counter}"),
            &session_id,
            "event",
            &format!("{host}/other"),
            Some("cta_clicked"),
            browser,
            country,
            &format!("{day} 09:55:00"),
        )
        .await;
    }

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Rich Dataset Funnel",
                "steps": [
                    {
                        "step_type": "page_view",
                        "match_value": "pricing",
                        "match_operator": "contains"
                    },
                    {
                        "step_type": "event",
                        "match_value": "signup_",
                        "match_operator": "contains"
                    }
                ]
            })
            .to_string(),
        ))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let start = today - chrono::Duration::days(60);
    let end = today;
    let results_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={start}&end_date={end}&filter_hostname=example.com&filter_browser=Chrome"
        ))
        .body(Body::empty())
        .expect("build request");

    let results_res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        app.clone().oneshot(results_req),
    )
    .await
    .expect("request should not time out")
    .expect("request");
    assert_eq!(results_res.status(), StatusCode::OK);
    let results_json = json_body(results_res).await;

    // idx % 3 == 0 and idx % 2 == 0 => every 6th session => 50 sessions entered.
    // Of those, idx % 4 == 0 completes step 2 => every 12th => 25 sessions.
    assert_eq!(results_json["data"]["total_sessions_entered"], 50);
    assert_eq!(results_json["data"]["steps"][0]["sessions_reached"], 50);
    assert_eq!(results_json["data"]["steps"][1]["sessions_reached"], 25);
}

#[tokio::test]
async fn test_funnel_results_returns_429_when_query_slot_busy() {
    let (state, app) = setup_none().await;
    let website_id = create_website(&app).await;
    let day = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    seed_funnel_events(&state, &website_id, day).await;

    let create_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(create_funnel_request("Busy Slot").to_string()))
        .expect("build request");
    let create_res = app.clone().oneshot(create_req).await.expect("request");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let funnel_id = create_json["data"]["id"]
        .as_str()
        .expect("funnel id")
        .to_string();

    let _held = state
        .funnel_results_semaphore
        .acquire()
        .await
        .expect("acquire semaphore");

    let blocked_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/funnels/{funnel_id}/results?start_date={day}&end_date={day}"
        ))
        .body(Body::empty())
        .expect("build request");
    let blocked_res = app.clone().oneshot(blocked_req).await.expect("request");
    assert_eq!(blocked_res.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_funnels_cap_enforced_at_20() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    for idx in 0..20 {
        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/websites/{website_id}/funnels"))
            .header("content-type", "application/json")
            .body(Body::from(
                create_funnel_request(&format!("Funnel {idx}")).to_string(),
            ))
            .expect("build request");
        let res = app.clone().oneshot(req).await.expect("request");
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    let overflow_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/funnels"))
        .header("content-type", "application/json")
        .body(Body::from(create_funnel_request("Overflow").to_string()))
        .expect("build request");
    let overflow_res = app.clone().oneshot(overflow_req).await.expect("request");
    assert_eq!(overflow_res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let overflow_json = json_body(overflow_res).await;
    assert_eq!(overflow_json["error"]["code"], "limit_exceeded");
}

#[tokio::test]
async fn test_funnels_endpoints_require_auth_in_local_mode() {
    let (_state, app) = setup_auth().await;

    let setup_request = Request::builder()
        .method("POST")
        .uri("/api/auth/setup")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "password": TEST_PASSWORD }).to_string()))
        .expect("build request");
    let setup_response = app.clone().oneshot(setup_request).await.expect("request");
    assert_eq!(setup_response.status(), StatusCode::CREATED);

    let list_request = Request::builder()
        .method("GET")
        .uri("/api/websites/site_any/funnels")
        .body(Body::empty())
        .expect("build request");
    let list_response = app.clone().oneshot(list_request).await.expect("request");
    assert_eq!(list_response.status(), StatusCode::UNAUTHORIZED);
}
