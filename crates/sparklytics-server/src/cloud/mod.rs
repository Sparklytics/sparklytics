pub mod api_keys;
pub mod clerk_auth;
pub mod clickhouse;
pub mod config;
pub mod handlers;
pub mod pg;
pub mod tenant_context;
pub mod usage;
pub mod webhook;

use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::state::AppState;

/// Build the cloud-mode Axum router.
///
/// Includes:
/// - Clerk JWT validation via `ClerkLayer` on all analytics + website routes.
/// - `POST /webhooks/clerk` (no Clerk auth; verified by Svix signature).
/// - Cloud analytics routes (ClickHouse backend).
/// - Cloud API key management endpoints.
pub fn build_cloud_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let clerk_secret = state
        .cloud_cfg()
        .map(|c| c.clerk_secret_key.clone())
        .unwrap_or_else(|e| panic!("Cloud config unavailable when building cloud router: {e}"));
    let clerk_layer = clerk_auth::build_clerk_layer(&clerk_secret);

    // Clerk-protected analytics + website + key management routes.
    let protected = Router::new()
        .route(
            "/api/websites",
            get(handlers::cloud_list_websites).post(handlers::cloud_create_website),
        )
        .route(
            "/api/websites/{id}",
            put(handlers::cloud_update_website).delete(handlers::cloud_delete_website),
        )
        .route("/api/websites/{id}/stats", get(handlers::cloud_stats))
        .route(
            "/api/websites/{id}/pageviews",
            get(handlers::cloud_pageviews),
        )
        .route("/api/websites/{id}/metrics", get(handlers::cloud_metrics))
        .route("/api/websites/{id}/realtime", get(handlers::cloud_realtime))
        .route(
            "/api/websites/{id}/share",
            post(handlers::enable_website_sharing).delete(handlers::disable_website_sharing),
        )
        .route(
            "/api/websites/{id}/export",
            get(handlers::cloud_export_events),
        )
        .route("/api/usage", get(handlers::get_usage))
        .route(
            "/api/keys",
            get(api_keys::list_keys).post(api_keys::create_key),
        )
        .route("/api/keys/{id}", delete(api_keys::revoke_key))
        .layer(clerk_layer);

    // Webhook route: no Clerk auth, verified by Svix HMAC.
    let webhook = Router::new().route("/webhooks/clerk", post(webhook::clerk_webhook_handler));

    Router::new().merge(protected).merge(webhook)
}
