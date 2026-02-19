/// Sprint 5 BDD tests for TenantContext JWT extraction and cloud config.
///
/// These tests are gated behind the `cloud` feature flag and do NOT require
/// live external services (Clerk, PostgreSQL, ClickHouse).  They exercise:
///
/// - TenantContext JWT payload decoding (happy + error paths)
/// - OrgRole parsing from Clerk JWT claim format
/// - AppError::PlanLimitExceeded → HTTP 429 mapping
/// - NullBillingGate always allows (unit-level verification)
///
/// Scenarios requiring live services (database webhook roundtrip, Clerk token
/// signature verification) are listed as "Manual test" in sprint-5.md.
#[cfg(feature = "cloud")]
mod cloud_tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        response::IntoResponse,
        routing::get,
        Json, Router,
    };
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use sparklytics_server::cloud::tenant_context::TenantContext;
    use sparklytics_server::error::AppError;

    // ─── JWT test helpers ────────────────────────────────────────────────────

    /// Minimal base64url encoder (no padding, URL-safe alphabet).
    fn b64url(data: &[u8]) -> String {
        const ALPHA: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            match chunk.len() {
                3 => {
                    out.push(ALPHA[(chunk[0] >> 2) as usize] as char);
                    out.push(ALPHA[((chunk[0] & 3) << 4 | chunk[1] >> 4) as usize] as char);
                    out.push(ALPHA[((chunk[1] & 0xf) << 2 | chunk[2] >> 6) as usize] as char);
                    out.push(ALPHA[(chunk[2] & 0x3f) as usize] as char);
                }
                2 => {
                    out.push(ALPHA[(chunk[0] >> 2) as usize] as char);
                    out.push(ALPHA[((chunk[0] & 3) << 4 | chunk[1] >> 4) as usize] as char);
                    out.push(ALPHA[((chunk[1] & 0xf) << 2) as usize] as char);
                }
                1 => {
                    out.push(ALPHA[(chunk[0] >> 2) as usize] as char);
                    out.push(ALPHA[((chunk[0] & 3) << 4) as usize] as char);
                }
                _ => {}
            }
        }
        out
    }

    /// Build a fake JWT with the given payload (signature is always "fakesig").
    ///
    /// The TenantContext extractor only base64url-decodes the payload
    /// section — it never re-verifies the signature (ClerkLayer handles that).
    fn fake_jwt(payload: &Value) -> String {
        let header = b64url(b"{\"alg\":\"RS256\"}");
        let body = serde_json::to_string(payload).unwrap();
        let payload_b64 = b64url(body.as_bytes());
        format!("{}.{}.fakesig", header, payload_b64)
    }

    /// Minimal handler that echoes the extracted TenantContext as JSON.
    async fn tenant_echo(tc: TenantContext) -> impl IntoResponse {
        Json(json!({
            "tenant_id": tc.tenant_id,
            "user_id": tc.user_id,
            "role": format!("{:?}", tc.role),
        }))
    }

    /// Build a test router with just the tenant_echo handler (no ClerkLayer).
    fn test_router() -> Router {
        Router::new().route("/test", get(tenant_echo))
    }

    async fn json_body(res: axum::http::Response<Body>) -> Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(json!(null))
    }

    // ─── TenantContext tests ─────────────────────────────────────────────────

    /// BDD: Valid Clerk session token grants access (admin role).
    ///
    /// Feature: Clerk Authentication
    /// Scenario: Valid Clerk session token grants access
    ///   Given I have a valid Clerk session token for user "user_abc" in org "org_tenant1"
    ///   When I GET /api/websites with the token in Authorization header
    ///   Then response status is 200
    ///   And TenantContext has tenant_id = "org_tenant1"
    #[tokio::test]
    async fn test_tenant_context_valid_admin_token() {
        let token = fake_jwt(&json!({
            "sub": "user_abc",
            "o": { "id": "org_tenant1", "slg": "acme", "rol": "admin" }
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["tenant_id"], "org_tenant1");
        assert_eq!(body["user_id"], "user_abc");
        assert_eq!(body["role"], "Admin");
    }

    /// BDD: Viewer role is parsed correctly.
    #[tokio::test]
    async fn test_tenant_context_valid_viewer_token() {
        let token = fake_jwt(&json!({
            "sub": "user_viewer",
            "o": { "id": "org_abc", "slg": "abc", "rol": "viewer" }
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["role"], "Viewer");
    }

    /// BDD: Clerk org-prefixed role format ("org:admin") is normalised.
    #[tokio::test]
    async fn test_tenant_context_clerk_prefixed_role() {
        let token = fake_jwt(&json!({
            "sub": "user_prefixed",
            "o": { "id": "org_xyz", "slg": "xyz", "rol": "org:admin" }
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["role"], "Admin");
    }

    /// BDD: Token without `rol` field defaults to Member.
    #[tokio::test]
    async fn test_tenant_context_default_member_role() {
        let token = fake_jwt(&json!({
            "sub": "user_mem",
            "o": { "id": "org_mem", "slg": "mem" }
            // no "rol" key → should default to Member
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["role"], "Member");
    }

    /// BDD: Token without organization context is rejected with 403.
    ///
    /// Feature: Clerk Authentication
    /// Scenario: Token without organization context is rejected for tenant routes
    ///   Given I have a Clerk token for a user not in any organization
    ///   When I GET /api/websites
    ///   Then response status is 403
    ///   And error message is "Organization context required"
    #[tokio::test]
    async fn test_tenant_context_no_org_claim_returns_403() {
        let token = fake_jwt(&json!({
            "sub": "user_no_org"
            // no "o" field
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let body = json_body(res).await;
        assert_eq!(body["error"]["message"], "Organization context required");
    }

    /// BDD: Missing Authorization header returns 401.
    ///
    /// Feature: Clerk Authentication
    /// Scenario: Invalid/expired token is rejected
    ///   When I GET /api/websites (no auth header)
    ///   Then response status is 401
    #[tokio::test]
    async fn test_tenant_context_no_auth_header_returns_401() {
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    /// BDD: Malformed (non-JWT) bearer token returns 401.
    #[tokio::test]
    async fn test_tenant_context_malformed_token_returns_401() {
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", "Bearer notavalidjwt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    /// BDD: Missing `sub` claim returns 401.
    #[tokio::test]
    async fn test_tenant_context_missing_sub_returns_401() {
        let token = fake_jwt(&json!({
            // no "sub"
            "o": { "id": "org_abc", "slg": "abc", "rol": "admin" }
        }));
        let app = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    // OrgRole parsing is tested indirectly via the JWT tests above.
    // (OrgRole::from_str is private; behaviour is verified through the
    // role field in the TenantContext JSON echo response.)

    // ─── AppError::PlanLimitExceeded → HTTP 429 ──────────────────────────────

    /// BDD: BillingGate blocks event collection when gate returns LimitExceeded.
    ///
    /// Verifies that AppError::PlanLimitExceeded produces HTTP 429 with the
    /// correct JSON error body:
    ///   { "error": { "code": "plan_limit_exceeded", "message": "Event limit reached" } }
    #[tokio::test]
    async fn test_plan_limit_exceeded_maps_to_429() {
        let err = AppError::PlanLimitExceeded;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], "plan_limit_exceeded");
        assert_eq!(body["error"]["message"], "Event limit reached");
        assert_eq!(body["error"]["field"], Value::Null);
    }
}

/// These tests run regardless of feature flags.
///
/// They verify that self-hosted mode compiles and runs correctly
/// without any Clerk / cloud dependencies.
mod selfhosted_tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use sparklytics_core::config::{AppMode, AuthMode, Config};
    use sparklytics_duckdb::DuckDbBackend;
    use sparklytics_server::app::build_app;
    use sparklytics_server::state::AppState;

    fn none_config() -> Config {
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
        }
    }

    /// BDD: Self-hosted mode ignores Clerk entirely.
    ///
    /// Feature: Clerk Authentication
    /// Scenario: Self-hosted mode ignores Clerk entirely
    ///   Given SPARKLYTICS_MODE = "selfhosted"
    ///   And SPARKLYTICS_AUTH = "none"
    ///   When I GET /api/websites with no auth token
    ///   Then response status is 200 (open access; auth is disabled)
    ///   And no Clerk middleware is invoked
    #[tokio::test]
    async fn test_selfhosted_none_mode_no_clerk_required() {
        let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
        let state = Arc::new(AppState::new(db, none_config()));
        let app = build_app(Arc::clone(&state));

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/websites")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // No auth required in none mode — should return 200 with empty list.
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// BDD: NullBillingGate always allows — collect succeeds without a billing check.
    ///
    /// Feature: BillingGate Integration
    /// Scenario: NullBillingGate always allows (self-hosted default)
    #[tokio::test]
    async fn test_selfhosted_null_billing_gate_allows_collect() {
        let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
        db.seed_website("site_billing_test", "billing.example.com")
            .await
            .expect("seed website");
        let state = Arc::new(AppState::new(db, none_config()));
        let app = build_app(Arc::clone(&state));

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/collect")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "type": "pageview",
                            "website_id": "site_billing_test",
                            "url": "https://billing.example.com/"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }
}
