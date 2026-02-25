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

fn unique_data_dir(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir()
        .join(format!(
            "sparklytics-{prefix}-{}-{nanos}",
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned()
}

fn config(auth_mode: AuthMode) -> Config {
    Config {
        port: 0,
        data_dir: unique_data_dir("reports"),
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

fn create_stats_report_req(website_id: &str, name: &str) -> Request<Body> {
    create_report_req(
        website_id,
        name,
        json!({
            "version": 1,
            "report_type": "stats",
            "date_range_type": "relative",
            "relative_days": 30,
            "timezone": "UTC"
        }),
    )
}

fn create_report_req(website_id: &str, name: &str, config: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/reports"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": name,
                "description": "Weekly KPI snapshot",
                "config": config
            })
            .to_string(),
        ))
        .expect("build request")
}

#[tokio::test]
async fn test_reports_crud_preview_and_run() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let create_res = app
        .clone()
        .oneshot(create_stats_report_req(&website_id, "Weekly KPI"))
        .await
        .expect("create report");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let report_id = create_json["data"]["id"]
        .as_str()
        .expect("report id")
        .to_string();
    assert_eq!(create_json["data"]["name"], "Weekly KPI");

    let list_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/reports"))
        .body(Body::empty())
        .expect("build request");
    let list_res = app.clone().oneshot(list_req).await.expect("list reports");
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_json = json_body(list_res).await;
    assert_eq!(list_json["data"].as_array().expect("array").len(), 1);

    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/reports/{report_id}"))
        .body(Body::empty())
        .expect("build request");
    let get_res = app.clone().oneshot(get_req).await.expect("get report");
    assert_eq!(get_res.status(), StatusCode::OK);
    let get_json = json_body(get_res).await;
    assert!(get_json["data"]["last_run_at"].is_null());

    let preview_req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/reports/preview"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "version": 1,
                "report_type": "stats",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC"
            })
            .to_string(),
        ))
        .expect("build request");
    let preview_res = app
        .clone()
        .oneshot(preview_req)
        .await
        .expect("preview report");
    assert_eq!(preview_res.status(), StatusCode::OK);
    let preview_json = json_body(preview_res).await;
    assert!(preview_json["data"]["report_id"].is_null());
    assert!(preview_json["data"]["data"]["pageviews"].is_number());

    let run_req = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/websites/{website_id}/reports/{report_id}/run"
        ))
        .body(Body::empty())
        .expect("build request");
    let run_res = app.clone().oneshot(run_req).await.expect("run report");
    assert_eq!(run_res.status(), StatusCode::OK);
    let run_json = json_body(run_res).await;
    assert_eq!(run_json["data"]["report_id"], report_id);
    assert!(run_json["data"]["ran_at"].is_string());

    let get_after_run_req = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_id}/reports/{report_id}"))
        .body(Body::empty())
        .expect("build request");
    let get_after_run_res = app
        .clone()
        .oneshot(get_after_run_req)
        .await
        .expect("get report after run");
    assert_eq!(get_after_run_res.status(), StatusCode::OK);
    let get_after_run_json = json_body(get_after_run_res).await;
    assert!(get_after_run_json["data"]["last_run_at"].is_string());

    let update_req = Request::builder()
        .method("PUT")
        .uri(format!("/api/websites/{website_id}/reports/{report_id}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "Weekly KPI v2",
                "description": null,
                "config": {
                    "version": 1,
                    "report_type": "events",
                    "date_range_type": "absolute",
                    "start_date": "2026-01-01",
                    "end_date": "2026-01-31",
                    "timezone": "UTC"
                }
            })
            .to_string(),
        ))
        .expect("build request");
    let update_res = app
        .clone()
        .oneshot(update_req)
        .await
        .expect("update report");
    assert_eq!(update_res.status(), StatusCode::OK);
    let update_json = json_body(update_res).await;
    assert_eq!(update_json["data"]["name"], "Weekly KPI v2");
    assert_eq!(update_json["data"]["config"]["report_type"], "events");
    assert!(update_json["data"]["description"].is_null());

    let delete_req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/websites/{website_id}/reports/{report_id}"))
        .body(Body::empty())
        .expect("build request");
    let delete_res = app
        .clone()
        .oneshot(delete_req)
        .await
        .expect("delete report");
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_reports_duplicate_name_returns_422() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let first = app
        .clone()
        .oneshot(create_stats_report_req(&website_id, "Acquisition"))
        .await
        .expect("create first");
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .clone()
        .oneshot(create_stats_report_req(&website_id, "Acquisition"))
        .await
        .expect("create second");
    assert_eq!(second.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json_body(second).await;
    assert_eq!(body["error"]["code"], "duplicate_name");
}

#[tokio::test]
async fn test_reports_preview_invalid_absolute_range_returns_400() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/reports/preview"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "version": 1,
                "report_type": "stats",
                "date_range_type": "absolute",
                "start_date": "2026-02-10",
                "end_date": "2026-02-01",
                "timezone": "UTC"
            })
            .to_string(),
        ))
        .expect("build request");
    let res = app.clone().oneshot(req).await.expect("preview report");
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_reports_preview_with_compare_returns_compare_metadata() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/reports/preview"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "version": 1,
                "report_type": "stats",
                "date_range_type": "relative",
                "relative_days": 7,
                "compare_mode": "previous_period",
                "timezone": "UTC"
            })
            .to_string(),
        ))
        .expect("build request");
    let res = app.clone().oneshot(req).await.expect("preview report");
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["data"]["data"]["compare"]["mode"], "previous_period");
    assert!(body["data"]["data"]["compare"]["comparison_range"][0].is_string());
}

#[tokio::test]
async fn test_reports_create_invalid_name_returns_400() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_id}/reports"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "name": "   ",
                "description": null,
                "config": {
                    "version": 1,
                    "report_type": "stats",
                    "date_range_type": "relative",
                    "relative_days": 7,
                    "timezone": "UTC"
                }
            })
            .to_string(),
        ))
        .expect("build request");
    let res = app.clone().oneshot(req).await.expect("create report");
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_reports_limit_and_missing_resources() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    for index in 0..100 {
        let res = app
            .clone()
            .oneshot(create_stats_report_req(
                &website_id,
                &format!("Report {index:03}"),
            ))
            .await
            .expect("create report");
        assert_eq!(res.status(), StatusCode::CREATED);
    }

    let over_limit = app
        .clone()
        .oneshot(create_stats_report_req(&website_id, "Report 999"))
        .await
        .expect("create over limit");
    assert_eq!(over_limit.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let over_limit_json = json_body(over_limit).await;
    assert_eq!(over_limit_json["error"]["code"], "limit_exceeded");

    let run_missing = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/websites/{website_id}/reports/report_missing/run"
        ))
        .body(Body::empty())
        .expect("build request");
    let run_missing_res = app
        .clone()
        .oneshot(run_missing)
        .await
        .expect("run missing report");
    assert_eq!(run_missing_res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reports_metrics_requires_metric_type_and_isolation() {
    let (_state, app) = setup_none().await;
    let website_a = create_website(&app).await;
    let website_b = create_website(&app).await;

    let invalid_metrics = Request::builder()
        .method("POST")
        .uri(format!("/api/websites/{website_a}/reports/preview"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "version": 1,
                "report_type": "metrics",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC"
            })
            .to_string(),
        ))
        .expect("build request");
    let invalid_metrics_res = app
        .clone()
        .oneshot(invalid_metrics)
        .await
        .expect("preview metrics without type");
    assert_eq!(invalid_metrics_res.status(), StatusCode::BAD_REQUEST);

    let create_res = app
        .clone()
        .oneshot(create_stats_report_req(&website_a, "A report"))
        .await
        .expect("create report");
    assert_eq!(create_res.status(), StatusCode::CREATED);
    let create_json = json_body(create_res).await;
    let report_id = create_json["data"]["id"].as_str().expect("report id");

    let get_from_other_site = Request::builder()
        .method("GET")
        .uri(format!("/api/websites/{website_b}/reports/{report_id}"))
        .body(Body::empty())
        .expect("build request");
    let get_from_other_site_res = app
        .clone()
        .oneshot(get_from_other_site)
        .await
        .expect("get report from another website");
    assert_eq!(get_from_other_site_res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_reports_run_supports_all_report_types() {
    let (_state, app) = setup_none().await;
    let website_id = create_website(&app).await;

    let configs = vec![
        (
            "Stats report",
            json!({
                "version": 1,
                "report_type": "stats",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC"
            }),
            "pageviews",
        ),
        (
            "Pageviews report",
            json!({
                "version": 1,
                "report_type": "pageviews",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC"
            }),
            "series",
        ),
        (
            "Metrics report",
            json!({
                "version": 1,
                "report_type": "metrics",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC",
                "metric_type": "browser"
            }),
            "rows",
        ),
        (
            "Events report",
            json!({
                "version": 1,
                "report_type": "events",
                "date_range_type": "relative",
                "relative_days": 7,
                "timezone": "UTC"
            }),
            "rows",
        ),
    ];

    for (name, config, expected_key) in configs {
        let create_res = app
            .clone()
            .oneshot(create_report_req(&website_id, name, config))
            .await
            .expect("create report");
        assert_eq!(create_res.status(), StatusCode::CREATED);
        let create_json = json_body(create_res).await;
        let report_id = create_json["data"]["id"].as_str().expect("report id");

        let run_req = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/websites/{website_id}/reports/{report_id}/run"
            ))
            .body(Body::empty())
            .expect("build request");
        let run_res = app.clone().oneshot(run_req).await.expect("run report");
        assert_eq!(run_res.status(), StatusCode::OK);
        let run_json = json_body(run_res).await;
        assert!(
            run_json["data"]["data"][expected_key].is_array()
                || run_json["data"]["data"][expected_key].is_number()
        );

        let get_req = Request::builder()
            .method("GET")
            .uri(format!("/api/websites/{website_id}/reports/{report_id}"))
            .body(Body::empty())
            .expect("build request");
        let get_res = app.clone().oneshot(get_req).await.expect("get report");
        assert_eq!(get_res.status(), StatusCode::OK);
        let get_json = json_body(get_res).await;
        assert!(get_json["data"]["last_run_at"].is_string());
    }
}
