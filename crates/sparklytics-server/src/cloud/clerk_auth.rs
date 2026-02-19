use clerk_rs::{
    clerk::Clerk, validators::axum::ClerkLayer, validators::jwks::MemoryCacheJwksProvider,
    ClerkConfiguration,
};

/// Build a `ClerkLayer` that validates all incoming JWT tokens against
/// Clerk's JWKS endpoint, with in-process JWKS caching.
///
/// The layer is applied to all cloud-mode protected routes. Any request with
/// an invalid or missing token is rejected with HTTP 401 before reaching the
/// handler. Successful requests continue; `TenantContext` then extracts the
/// `org_id` and `user_id` from the already-validated JWT.
pub fn build_clerk_layer(secret_key: &str) -> ClerkLayer<MemoryCacheJwksProvider> {
    let config = ClerkConfiguration::new(None, None, Some(secret_key.to_string()), None);
    let clerk = Clerk::new(config);
    // None = protect all routes; true = also validate session cookies.
    ClerkLayer::new(MemoryCacheJwksProvider::new(clerk), None, true)
}
