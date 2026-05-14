mod common;

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

const LOCAL_PASSWORD: &str = "correct horse battery staple";
const BOOTSTRAP_PASSWORD: &str = "bootstrap-secret";
const PASSWORD_MODE_SECRET: &str = "password-mode-secret";

fn config(auth_mode: AuthMode, data_dir: String) -> Config {
    Config {
        port: 0,
        data_dir,
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode,
        bootstrap_password: Some(BOOTSTRAP_PASSWORD.to_string()),
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 4096,
        public_url: "http://127.0.0.1:3000".to_string(),
        tracking_public_base: "http://127.0.0.1:3000/_sl".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

fn file_backed_app(auth_mode: AuthMode, data_dir: String) -> (Arc<AppState>, axum::Router) {
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let db_path = format!("{data_dir}/sparklytics.db");
    let db = DuckDbBackend::open(&db_path, "1GB").expect("file-backed DuckDB");
    let state = Arc::new(AppState::new(db, config(auth_mode, data_dir)));
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

fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("user-agent", "SparklyticsInProcessSmoke/1.0")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

fn empty_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("build request")
}

fn cookie_from(response: &axum::http::Response<Body>) -> String {
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("Set-Cookie header")
        .to_str()
        .expect("valid Set-Cookie");
    assert!(set_cookie.contains("spk_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(!set_cookie.contains("Secure"));
    set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

async fn setup_local(app: &axum::Router) {
    let setup = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/setup",
            json!({
                "bootstrap_password": BOOTSTRAP_PASSWORD,
                "password": LOCAL_PASSWORD
            }),
        ))
        .await
        .expect("setup request");
    assert_eq!(setup.status(), StatusCode::CREATED);

    let second_setup = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/setup",
            json!({
                "bootstrap_password": BOOTSTRAP_PASSWORD,
                "password": LOCAL_PASSWORD
            }),
        ))
        .await
        .expect("second setup request");
    assert_eq!(second_setup.status(), StatusCode::GONE);
}

async fn login(app: &axum::Router, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/auth/login",
            json!({ "password": password }),
        ))
        .await
        .expect("login request");
    assert_eq!(response.status(), StatusCode::OK);
    cookie_from(&response)
}

async fn list_websites(app: &axum::Router, cookie: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/websites")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("build websites request"),
        )
        .await
        .expect("websites request");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

#[tokio::test]
async fn selfhost_first_launch_smoke_runs_without_socket_binding() {
    let data_dir = common::unique_data_dir("selfhost-smoke");
    let (state, app) = file_backed_app(AuthMode::Local, data_dir.clone());

    let health = app
        .clone()
        .oneshot(empty_request("/health"))
        .await
        .expect("health request");
    assert_eq!(health.status(), StatusCode::OK);

    let status = app
        .clone()
        .oneshot(empty_request("/api/auth/status"))
        .await
        .expect("auth status request");
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        json_body(status).await,
        json!({
            "authenticated": false,
            "mode": "local",
            "password_change_required": false,
            "setup_required": true
        })
    );

    setup_local(&app).await;
    let cookie = login(&app, LOCAL_PASSWORD).await;

    let websites = list_websites(&app, &cookie).await;
    assert_eq!(
        websites["data"].as_array().expect("websites array").len(),
        0
    );

    let create_website = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/websites")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    json!({
                        "name": "Smoke Site",
                        "domain": "smoke.example.com",
                        "timezone": "UTC"
                    })
                    .to_string(),
                ))
                .expect("build website request"),
        )
        .await
        .expect("create website request");
    assert_eq!(create_website.status(), StatusCode::CREATED);
    let website = json_body(create_website).await["data"].clone();
    let website_id = website["id"].as_str().expect("website id").to_string();
    assert!(website["tenant_id"].is_null());
    assert!(website["tracking_snippet"]
        .as_str()
        .expect("tracking snippet")
        .contains(r#"src="http://127.0.0.1:3000/_sl/s.js""#));

    for script_path in ["/s.js", "/_sl/s.js"] {
        let script = app
            .clone()
            .oneshot(empty_request(script_path))
            .await
            .expect("script request");
        assert_eq!(script.status(), StatusCode::OK);
        assert!(script
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .contains("javascript"));
        let body = script
            .into_body()
            .collect()
            .await
            .expect("read script")
            .to_bytes();
        assert!(std::str::from_utf8(&body)
            .expect("script utf8")
            .contains("Sparklytics tracking script"));
    }

    for (uri, path) in [
        ("/e", "/first-launch-short-alias"),
        ("/api/collect", "/first-launch-collect"),
        ("/_sl/e", "/first-party-proxy-alias"),
    ] {
        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                uri,
                json!({
                    "website_id": website_id,
                    "type": "pageview",
                    "url": path
                }),
            ))
            .await
            .expect("collect request");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    state.flush_buffer().await;
    let (start, end) = common::surrounding_date_window();
    let stats = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/websites/{website_id}/stats?start_date={start}&end_date={end}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .expect("build stats request"),
        )
        .await
        .expect("stats request");
    assert_eq!(stats.status(), StatusCode::OK);
    let stats_json = json_body(stats).await;
    assert!(
        stats_json["data"]["pageviews"].as_i64().unwrap_or_default() >= 3,
        "stats should include pageviews from /e, /api/collect, and /_sl/e"
    );

    drop(app);
    drop(state);

    let (_restarted_state, restarted_app) = file_backed_app(AuthMode::Local, data_dir);
    let restarted_cookie = login(&restarted_app, LOCAL_PASSWORD).await;
    let persisted_websites = list_websites(&restarted_app, &restarted_cookie).await;
    assert_eq!(
        persisted_websites["data"]
            .as_array()
            .expect("persisted websites array")
            .len(),
        1,
        "website should persist across file-backed DuckDB reopen"
    );
}

#[tokio::test]
async fn selfhost_password_and_none_auth_modes_match_release_contract() {
    let password_data_dir = common::unique_data_dir("selfhost-password-smoke");
    let (_password_state, password_app) = file_backed_app(
        AuthMode::Password(PASSWORD_MODE_SECRET.to_string()),
        password_data_dir,
    );

    let status = password_app
        .clone()
        .oneshot(empty_request("/api/auth/status"))
        .await
        .expect("password status request");
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        json_body(status).await,
        json!({
            "authenticated": false,
            "mode": "password",
            "password_change_required": false,
            "setup_required": false
        })
    );
    let _password_cookie = login(&password_app, PASSWORD_MODE_SECRET).await;

    let none_data_dir = common::unique_data_dir("selfhost-none-smoke");
    let (_none_state, none_app) = file_backed_app(AuthMode::None, none_data_dir);
    let none_status = none_app
        .clone()
        .oneshot(empty_request("/api/auth/status"))
        .await
        .expect("none status request");
    assert_eq!(none_status.status(), StatusCode::NOT_FOUND);
}
