use async_trait::async_trait;
use serde::Serialize;
use sparklytics_core::analytics::BotPolicy;

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Website {
    pub id: String,
    pub tenant_id: Option<String>,
    pub name: String,
    pub domain: String,
    pub timezone: String,
    pub share_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateWebsiteParams {
    pub name: String,
    pub domain: String,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateWebsiteParams {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub timezone: Option<String>,
}

/// Storage interface for non-analytics metadata operations.
///
/// Self-hosted mode uses a DuckDB implementation. Cloud mode can swap this for
/// a PostgreSQL-backed implementation while keeping route handlers unchanged.
#[async_trait]
pub trait MetadataStore: Send + Sync + 'static {
    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>>;
    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()>;
    async fn ensure_jwt_secret(&self) -> anyhow::Result<String>;
    async fn is_admin_configured(&self) -> anyhow::Result<bool>;

    async fn lookup_api_key(&self, key_hash: &str) -> anyhow::Result<Option<ApiKeyRecord>>;
    async fn touch_api_key(&self, key_id: &str) -> anyhow::Result<()>;
    async fn create_api_key(
        &self,
        id: &str,
        name: &str,
        hash: &str,
        prefix: &str,
    ) -> anyhow::Result<()>;
    async fn revoke_api_key(&self, key_id: &str) -> anyhow::Result<bool>;
    async fn list_api_keys(
        &self,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<(Vec<ApiKeyRecord>, i64)>;
    async fn record_login_attempt(&self, ip: &str, succeeded: bool) -> anyhow::Result<()>;
    async fn check_login_rate_limit(&self, ip: &str) -> anyhow::Result<bool>;

    async fn create_website(&self, params: CreateWebsiteParams) -> anyhow::Result<Website>;
    async fn list_websites(
        &self,
        limit: i64,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<Website>, i64, bool)>;
    async fn website_exists(&self, id: &str) -> anyhow::Result<bool>;
    async fn get_website(&self, id: &str) -> anyhow::Result<Option<Website>>;
    async fn update_website(
        &self,
        id: &str,
        params: UpdateWebsiteParams,
    ) -> anyhow::Result<Option<Website>>;
    async fn delete_website(&self, id: &str) -> anyhow::Result<bool>;

    /// Resolve a share id to `(website_id, tenant_id_opt)`.
    ///
    /// Returns:
    /// - `Ok(None)` when no matching shared website exists.
    /// - `Ok(Some((website_id, tenant_id_opt)))` when found.
    async fn get_website_by_share_id(
        &self,
        share_id: &str,
    ) -> anyhow::Result<Option<(String, Option<String>)>>;
    async fn set_share_id(&self, website_id: &str, share_id: &str) -> anyhow::Result<()>;
    async fn clear_share_id(&self, website_id: &str) -> anyhow::Result<()>;
    async fn get_share_id(&self, website_id: &str) -> anyhow::Result<Option<String>>;
    async fn get_bot_policy(&self, website_id: &str) -> anyhow::Result<BotPolicy>;

    /// Classify request overrides from allow/block lists.
    ///
    /// Returns:
    /// - `Ok(Some(true))` to force bot classification.
    /// - `Ok(Some(false))` to force human classification.
    /// - `Ok(None)` when no override rule matches.
    async fn classify_override_for_request(
        &self,
        website_id: &str,
        client_ip: &str,
        user_agent: &str,
    ) -> anyhow::Result<Option<bool>>;
}
