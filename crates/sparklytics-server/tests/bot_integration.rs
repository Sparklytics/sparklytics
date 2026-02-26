use std::sync::Arc;
use std::time::Duration as StdDuration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
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
        argon2_memory_kb: 65536,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

async fn setup() -> (Arc<AppState>, axum::Router) {
    let db = DuckDbBackend::open_in_memory().expect("in-memory db");
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
    serde_json::from_slice(&bytes).expect("json body")
}

async fn create_website(app: &axum::Router) -> String {
    let request = Request::builder()
        .method("POST")
        .uri("/api/websites")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": "Bot Test", "domain": "bot.example.com" }).to_string(),
        ))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("create website");
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_body(response).await;
    body["data"]["id"].as_str().expect("website id").to_string()
}

async fn collect_event(
    app: &axum::Router,
    website_id: &str,
    user_agent: &str,
    client_ip: &str,
    url: &str,
) {
    let request = Request::builder()
        .method("POST")
        .uri("/api/collect")
        .header("content-type", "application/json")
        .header("x-forwarded-for", client_ip)
        .header(header::USER_AGENT, user_agent)
        .header(header::ACCEPT, "text/html")
        .header(header::ACCEPT_LANGUAGE, "en-US")
        .body(Body::from(
            json!({
                "website_id": website_id,
                "type": "pageview",
                "url": url,
            })
            .to_string(),
        ))
        .expect("collect request");
    let response = app.clone().oneshot(request).await.expect("collect");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn bot_classification_marks_known_crawler() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "203.0.113.20",
        "https://bot.example.com/",
    )
    .await;

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let (is_bot, reason): (bool, Option<String>) = conn
        .prepare("SELECT is_bot, bot_reason FROM events WHERE website_id = ?1 LIMIT 1")
        .expect("prepare")
        .query_row(sparklytics_duckdb::duckdb::params![website_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("read event");
    assert!(is_bot);
    assert_eq!(reason.as_deref(), Some("ua_signature"));
}

#[tokio::test]
async fn human_browser_traffic_remains_non_bot() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/122.0.0.0 Safari/537.36",
        "198.51.100.5",
        "https://bot.example.com/human",
    )
    .await;

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let (event_is_bot, event_reason): (bool, Option<String>) = conn
        .prepare("SELECT is_bot, bot_reason FROM events WHERE website_id = ?1 LIMIT 1")
        .expect("prepare")
        .query_row(sparklytics_duckdb::duckdb::params![website_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("event row");
    assert!(!event_is_bot);
    assert_eq!(event_reason, None);
}

#[tokio::test]
async fn stats_excludes_bots_by_default_and_can_include_them() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36 Chrome/122.0 Safari/537.36",
        "198.51.100.10",
        "https://bot.example.com/human",
    )
    .await;
    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "198.51.100.11",
        "https://bot.example.com/bot",
    )
    .await;
    state.flush_buffer().await;

    let today = chrono::Utc::now().date_naive();
    let start = today.format("%Y-%m-%d");
    let end = today.format("%Y-%m-%d");

    let default_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/stats?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("stats req");
    let default_res = app.clone().oneshot(default_req).await.expect("stats");
    assert_eq!(default_res.status(), StatusCode::OK);
    let default_json = json_body(default_res).await;
    assert_eq!(default_json["data"]["pageviews"], 1);

    let include_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/stats?start_date={start}&end_date={end}&include_bots=true"
        ))
        .body(Body::empty())
        .expect("stats req");
    let include_res = app.clone().oneshot(include_req).await.expect("stats");
    assert_eq!(include_res.status(), StatusCode::OK);
    let include_json = json_body(include_res).await;
    assert_eq!(include_json["data"]["pageviews"], 2);
}

#[tokio::test]
async fn bot_summary_reports_split_and_reasons() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36 Chrome/122.0 Safari/537.36",
        "198.51.100.20",
        "https://bot.example.com/human",
    )
    .await;
    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "198.51.100.21",
        "https://bot.example.com/bot",
    )
    .await;
    state.flush_buffer().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot-summary"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("bot summary");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["data"]["bot_events"], 1);
    assert_eq!(body["data"]["human_events"], 1);
    assert_eq!(body["data"]["top_reasons"][0]["code"], "ua_signature");
}

#[tokio::test]
async fn bot_report_returns_split_timeseries_and_top_user_agents() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36 Chrome/122.0 Safari/537.36",
        "198.51.100.70",
        "https://bot.example.com/human",
    )
    .await;
    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "198.51.100.71",
        "https://bot.example.com/bot",
    )
    .await;
    state.flush_buffer().await;

    let today = chrono::Utc::now().date_naive();
    let date = today.format("%Y-%m-%d");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/websites/{website_id}/bot/report?start_date={date}&end_date={date}&granularity=day"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("bot report");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["data"]["split"]["bot_events"], 1);
    assert_eq!(body["data"]["split"]["human_events"], 1);
    assert_eq!(body["data"]["timeseries"][0]["bot_events"], 1);
    assert_eq!(body["data"]["timeseries"][0]["human_events"], 1);
    assert_eq!(body["data"]["top_reasons"][0]["code"], "ua_signature");
    assert_eq!(
        body["data"]["top_user_agents"][0]["value"],
        "Googlebot/2.1 (+http://www.google.com/bot.html)"
    );
}

#[tokio::test]
async fn bot_policy_off_includes_all_by_default_and_audits_update() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/537.36 Chrome/122.0 Safari/537.36",
        "198.51.100.30",
        "https://bot.example.com/human",
    )
    .await;
    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "198.51.100.31",
        "https://bot.example.com/bot",
    )
    .await;
    state.flush_buffer().await;

    let update_policy = Request::builder()
        .method("PUT")
        .uri(format!("/api/websites/{website_id}/bot/policy"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "mode": "off", "threshold_score": 70 }).to_string(),
        ))
        .expect("request");
    let update_response = app.clone().oneshot(update_policy).await.expect("policy");
    if update_response.status() != StatusCode::OK {
        let body = json_body(update_response).await;
        panic!("policy update failed: {body}");
    }

    let today = chrono::Utc::now().date_naive();
    let start = today.format("%Y-%m-%d");
    let end = today.format("%Y-%m-%d");
    let stats_req = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/websites/{website_id}/stats?start_date={start}&end_date={end}"
        ))
        .body(Body::empty())
        .expect("request");
    let stats_response = app.clone().oneshot(stats_req).await.expect("stats");
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats = json_body(stats_response).await;
    assert_eq!(stats["data"]["pageviews"], 2);

    let audit_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/bot/audit"))
        .body(Body::empty())
        .expect("request");
    let audit_response = app.clone().oneshot(audit_req).await.expect("audit");
    assert_eq!(audit_response.status(), StatusCode::OK);
    let audit = json_body(audit_response).await;
    let first_action = audit["data"][0]["action"].as_str().unwrap_or_default();
    assert_eq!(first_action, "policy_update");
}

#[tokio::test]
async fn recompute_returns_job_status_and_audit_record() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    collect_event(
        &app,
        &website_id,
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
        "203.0.113.40",
        "https://bot.example.com/recompute",
    )
    .await;
    state.flush_buffer().await;

    let today = chrono::Utc::now().date_naive();
    let date = today.format("%Y-%m-%d");
    let recompute_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/websites/{website_id}/bot/recompute"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "start_date": date.to_string(),
                        "end_date": date.to_string(),
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("recompute start");
    assert_eq!(recompute_response.status(), StatusCode::ACCEPTED);
    let recompute_json = json_body(recompute_response).await;
    let job_id = recompute_json["job_id"].as_str().expect("job_id");
    assert_eq!(recompute_json["status"], "queued");

    let mut status = String::new();
    let mut run_payload = Value::Null;
    for _ in 0..20 {
        let status_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/websites/{website_id}/bot/recompute/{job_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("recompute status");
        assert_eq!(status_response.status(), StatusCode::OK);
        let status_json = json_body(status_response).await;
        run_payload = status_json["data"].clone();
        status = run_payload["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if status == "success" {
            break;
        }
        tokio::time::sleep(StdDuration::from_millis(25)).await;
    }
    assert_eq!(run_payload["website_id"], website_id);
    assert!(run_payload["created_at"].as_str().is_some());
    assert_eq!(status, "success");

    let audit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot/audit"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("audit");
    assert_eq!(audit_response.status(), StatusCode::OK);
    let audit_json = json_body(audit_response).await;
    let first_action = audit_json["data"][0]["action"].as_str().unwrap_or_default();
    assert_eq!(first_action, "recompute_start");
}

#[tokio::test]
async fn recompute_returns_conflict_when_active_job_exists() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;
    let now = chrono::Utc::now();
    state
        .db
        .create_bot_recompute_run(&website_id, now - chrono::Duration::days(1), now)
        .await
        .expect("create run");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/websites/{website_id}/bot/recompute"))
                .header("content-type", "application/json")
                .body(Body::from("{}".to_string()))
                .expect("request"),
        )
        .await
        .expect("recompute");
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn allowlist_and_blocklist_override_classification() {
    let (state, app) = setup().await;
    let website_id = create_website(&app).await;

    let add_allow = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/bot/allowlist"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "match_type": "ua_contains",
                "match_value": "my-monitor",
                "note": "synthetic traffic"
            })
            .to_string(),
        ))
        .expect("request");
    let allow_response = app.clone().oneshot(add_allow).await.expect("allow");
    assert_eq!(allow_response.status(), StatusCode::CREATED);
    let allow_json = json_body(allow_response).await;
    let allow_id = allow_json["data"]["id"]
        .as_str()
        .expect("allow id")
        .to_string();

    let add_block = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/bot/blocklist"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "match_type": "ip_exact",
                "match_value": "203.0.113.10",
                "note": "abusive client"
            })
            .to_string(),
        ))
        .expect("request");
    let block_response = app.clone().oneshot(add_block).await.expect("block");
    assert_eq!(block_response.status(), StatusCode::CREATED);
    let block_json = json_body(block_response).await;
    let block_id = block_json["data"]["id"]
        .as_str()
        .expect("block id")
        .to_string();

    let allow_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot/allowlist?limit=1"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("allow list");
    assert_eq!(allow_list_response.status(), StatusCode::OK);
    let allow_list_json = json_body(allow_list_response).await;
    assert_eq!(allow_list_json["data"][0]["id"], allow_id);

    let block_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot/blocklist?limit=1"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("block list");
    assert_eq!(block_list_response.status(), StatusCode::OK);
    let block_list_json = json_body(block_list_response).await;
    assert_eq!(block_list_json["data"][0]["id"], block_id);

    collect_event(
        &app,
        &website_id,
        "my-monitor/1.0",
        "198.51.100.40",
        "https://bot.example.com/allow",
    )
    .await;
    collect_event(
        &app,
        &website_id,
        "Mozilla/5.0",
        "203.0.113.10",
        "https://bot.example.com/block",
    )
    .await;

    state.flush_buffer().await;
    let conn = state.db.conn_for_test().await;
    let mut stmt = conn
        .prepare(
            "SELECT url, is_bot, bot_reason
             FROM events
             WHERE website_id = ?1
             ORDER BY created_at ASC",
        )
        .expect("prepare");
    let rows = stmt
        .query_map(sparklytics_duckdb::duckdb::params![website_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, bool>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, false);
    assert_eq!(rows[0].2.as_deref(), Some("allowlist"));
    assert_eq!(rows[1].1, true);
    assert_eq!(rows[1].2.as_deref(), Some("blocklist"));
    drop(stmt);
    drop(conn);

    let delete_allow = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/websites/{website_id}/bot/allowlist/{allow_id}"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("delete allow");
    assert_eq!(delete_allow.status(), StatusCode::NO_CONTENT);

    let delete_block = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/websites/{website_id}/bot/blocklist/{block_id}"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("delete block");
    assert_eq!(delete_block.status(), StatusCode::NO_CONTENT);

    let missing_allow_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/websites/{website_id}/bot/allowlist/{allow_id}"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("missing delete");
    assert_eq!(missing_allow_delete.status(), StatusCode::NOT_FOUND);

    let allow_after_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot/allowlist"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("allow list");
    let allow_after_delete_json = json_body(allow_after_delete).await;
    assert_eq!(
        allow_after_delete_json["data"].as_array().map(Vec::len),
        Some(0)
    );

    let audit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/websites/{website_id}/bot/audit"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("audit");
    let audit_json = json_body(audit_response).await;
    let actions = audit_json["data"]
        .as_array()
        .expect("audit entries")
        .iter()
        .map(|entry| entry["action"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(actions.contains(&"allow_remove"));
    assert!(actions.contains(&"block_remove"));
}
