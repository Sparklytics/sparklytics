use async_trait::async_trait;

use sparklytics_duckdb::{
    auth::ApiKeyRecord,
    website::{CreateWebsiteParams, UpdateWebsiteParams, Website},
};

pub mod duckdb;

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

    async fn get_website_by_share_id(
        &self,
        share_id: &str,
    ) -> anyhow::Result<Option<(String, Option<String>)>>;
    async fn set_share_id(&self, website_id: &str, share_id: &str) -> anyhow::Result<()>;
    async fn clear_share_id(&self, website_id: &str) -> anyhow::Result<()>;
    async fn get_share_id(&self, website_id: &str) -> anyhow::Result<Option<String>>;
}
