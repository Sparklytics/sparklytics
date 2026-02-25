use std::sync::Arc;

use axum::{body::Body, http::{Request, StatusCode}};
use serde_json::{json, Value};
use sparklytics_core::config::{AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::{app::build_app, state::AppState};
use tower::ServiceExt;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn unique_data_dir(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("sparklytics-{prefix}-{nanos}"))
        .to_string_lossy()
        .to_string()
}

fn config() -> Config {
    Config {
        port: 0,
        data_dir: unique_data_dir("notifications"),
        geoip_path: "./GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5_000,
        buffer_max_size: 100,
        mode: sparklytics_core::config::AppMode::SelfHosted,
        argon2_memory_kb: 65_536,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: true,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

async fn setup() -> (Arc<AppState>, axum::Router) {
    let cfg = config();
    std::fs::create_dir_all(&cfg.data_dir).expect("create data dir");
    let db_path = format!("{}/sparklytics.db", cfg.data_dir);
    let db = DuckDbBackend::open(&db_path, &cfg.duckdb_memory_limit).expect("open db");
    let state = Arc::new(AppState::new(db, cfg));
    let app = build_app(Arc::clone(&state));
    (state, app)
}

fn request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .expect("request")
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn notifications_crud_test_send_and_history() {
    let _smtp_noop = EnvVarGuard::set("SPARKLYTICS_SMTP_NOOP", "1");
    let _scheduler_db_mode = EnvVarGuard::set("SPARKLYTICS_SCHEDULER_DEDICATED_DUCKDB", "0");
    let (_state, app) = setup().await;

    let create_site = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/websites",
            json!({
                "name": "Site A",
                "domain": "example.com",
                "timezone": "UTC"
            }),
        ))
        .await
        .expect("create website");
    assert_eq!(create_site.status(), StatusCode::CREATED);
    let website_id = json_body(create_site).await["data"]["id"]
        .as_str()
        .expect("website id")
        .to_string();

    let create_report = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/reports"),
            json!({
                "name": "Daily KPIs",
                "description": "stats rollup",
                "config": {
                    "version": 1,
                    "report_type": "stats",
                    "date_range_type": "relative",
                    "relative_days": 7,
                    "timezone": "UTC"
                }
            }),
        ))
        .await
        .expect("create report");
    assert_eq!(create_report.status(), StatusCode::CREATED);
    let report_id = json_body(create_report).await["data"]["id"]
        .as_str()
        .expect("report id")
        .to_string();

    let create_subscription = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/subscriptions"),
            json!({
                "report_id": report_id,
                "schedule": "daily",
                "timezone": "UTC",
                "channel": "email",
                "target": "ops@example.com"
            }),
        ))
        .await
        .expect("create subscription");
    assert_eq!(create_subscription.status(), StatusCode::CREATED);
    let subscription_id = json_body(create_subscription).await["data"]["id"]
        .as_str()
        .expect("subscription id")
        .to_string();

    let list_subscriptions = app
        .clone()
        .oneshot(get(&format!("/api/websites/{website_id}/subscriptions")))
        .await
        .expect("list subscriptions");
    assert_eq!(list_subscriptions.status(), StatusCode::OK);
    assert_eq!(
        json_body(list_subscriptions).await["data"]
            .as_array()
            .expect("array")
            .len(),
        1
    );

    let test_subscription = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/subscriptions/{subscription_id}/test"),
            json!({}),
        ))
        .await
        .expect("test subscription");
    assert_eq!(test_subscription.status(), StatusCode::OK);

    let create_alert = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/alerts"),
            json!({
                "name": "Traffic spike",
                "metric": "pageviews",
                "condition_type": "spike",
                "threshold_value": 2.0,
                "lookback_days": 7,
                "channel": "email",
                "target": "ops@example.com"
            }),
        ))
        .await
        .expect("create alert");
    assert_eq!(create_alert.status(), StatusCode::CREATED);
    let alert_id = json_body(create_alert).await["data"]["id"]
        .as_str()
        .expect("alert id")
        .to_string();

    let test_alert = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/alerts/{alert_id}/test"),
            json!({}),
        ))
        .await
        .expect("test alert");
    assert_eq!(test_alert.status(), StatusCode::OK);
    assert_eq!(json_body(test_alert).await["data"]["status"], "sent");

    let update_alert_to_webhook = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/websites/{website_id}/alerts/{alert_id}"),
            json!({
                "channel": "webhook",
                "target": "https://nonexistent-webhook.sparklytics.invalid/hook"
            }),
        ))
        .await
        .expect("update alert to webhook");
    assert_eq!(update_alert_to_webhook.status(), StatusCode::OK);

    let failed_test_alert = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/websites/{website_id}/alerts/{alert_id}/test"),
            json!({}),
        ))
        .await
        .expect("test alert failure");
    assert_eq!(failed_test_alert.status(), StatusCode::OK);
    assert_eq!(json_body(failed_test_alert).await["data"]["status"], "failed");

    let update_subscription = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/websites/{website_id}/subscriptions/{subscription_id}"),
            json!({ "is_active": false }),
        ))
        .await
        .expect("update subscription");
    assert_eq!(update_subscription.status(), StatusCode::OK);
    assert_eq!(json_body(update_subscription).await["data"]["is_active"], false);

    let history = app
        .clone()
        .oneshot(get(&format!(
            "/api/websites/{website_id}/notifications/history?limit=20"
        )))
        .await
        .expect("history");
    assert_eq!(history.status(), StatusCode::OK);
    let history_json = json_body(history).await;
    let rows = history_json["data"].as_array().expect("history rows");
    assert!(
        rows.len() >= 3,
        "expected at least three rows (subscription test + alert test + alert failure)"
    );
    assert!(
        rows.iter().all(|row| row["status"].is_string()),
        "all rows must contain status"
    );

    let delete_subscription = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/websites/{website_id}/subscriptions/{subscription_id}"))
                .body(Body::empty())
                .expect("delete subscription"),
        )
        .await
        .expect("delete subscription");
    assert_eq!(delete_subscription.status(), StatusCode::NO_CONTENT);

    let delete_alert = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/websites/{website_id}/alerts/{alert_id}"))
                .body(Body::empty())
                .expect("delete alert"),
        )
        .await
        .expect("delete alert");
    assert_eq!(delete_alert.status(), StatusCode::NO_CONTENT);
}
