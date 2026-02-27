use std::sync::Arc;

use async_trait::async_trait;

use sparklytics_duckdb::{
    auth::ApiKeyRecord,
    website::{CreateWebsiteParams, UpdateWebsiteParams, Website},
    DuckDbBackend,
};

use super::MetadataStore;

pub struct DuckDbMetadataStore {
    db: Arc<DuckDbBackend>,
}

impl DuckDbMetadataStore {
    pub fn new(db: Arc<DuckDbBackend>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MetadataStore for DuckDbMetadataStore {
    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        self.db.get_setting(key).await
    }

    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.db.set_setting(key, value).await
    }

    async fn ensure_jwt_secret(&self) -> anyhow::Result<String> {
        self.db.ensure_jwt_secret().await
    }

    async fn is_admin_configured(&self) -> anyhow::Result<bool> {
        self.db.is_admin_configured().await
    }

    async fn lookup_api_key(&self, key_hash: &str) -> anyhow::Result<Option<ApiKeyRecord>> {
        self.db.lookup_api_key(key_hash).await
    }

    async fn touch_api_key(&self, key_id: &str) -> anyhow::Result<()> {
        self.db.touch_api_key(key_id).await
    }

    async fn create_api_key(
        &self,
        id: &str,
        name: &str,
        hash: &str,
        prefix: &str,
    ) -> anyhow::Result<()> {
        self.db.create_api_key(id, name, hash, prefix).await
    }

    async fn revoke_api_key(&self, key_id: &str) -> anyhow::Result<bool> {
        self.db.revoke_api_key(key_id).await
    }

    async fn list_api_keys(
        &self,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<(Vec<ApiKeyRecord>, i64)> {
        self.db.list_api_keys(limit, offset).await
    }

    async fn record_login_attempt(&self, ip: &str, succeeded: bool) -> anyhow::Result<()> {
        self.db.record_login_attempt(ip, succeeded).await
    }

    async fn check_login_rate_limit(&self, ip: &str) -> anyhow::Result<bool> {
        self.db.check_login_rate_limit(ip).await
    }

    async fn create_website(&self, params: CreateWebsiteParams) -> anyhow::Result<Website> {
        self.db.create_website(params).await
    }

    async fn list_websites(
        &self,
        limit: i64,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<Website>, i64, bool)> {
        self.db.list_websites(limit, cursor).await
    }

    async fn website_exists(&self, id: &str) -> anyhow::Result<bool> {
        self.db.website_exists(id).await
    }

    async fn get_website(&self, id: &str) -> anyhow::Result<Option<Website>> {
        self.db.get_website(id).await
    }

    async fn update_website(
        &self,
        id: &str,
        params: UpdateWebsiteParams,
    ) -> anyhow::Result<Option<Website>> {
        self.db.update_website(id, params).await
    }

    async fn delete_website(&self, id: &str) -> anyhow::Result<bool> {
        self.db.delete_website(id).await
    }

    async fn get_website_by_share_id(
        &self,
        share_id: &str,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        self.db.get_website_by_share_id(share_id).await
    }

    async fn set_share_id(&self, website_id: &str, share_id: &str) -> anyhow::Result<()> {
        self.db.set_share_id(website_id, share_id).await
    }

    async fn clear_share_id(&self, website_id: &str) -> anyhow::Result<()> {
        self.db.clear_share_id(website_id).await
    }

    async fn get_share_id(&self, website_id: &str) -> anyhow::Result<Option<String>> {
        self.db.get_share_id(website_id).await
    }

    async fn classify_override_for_request(
        &self,
        website_id: &str,
        client_ip: &str,
        user_agent: &str,
    ) -> anyhow::Result<Option<bool>> {
        self.db
            .classify_override_for_request(website_id, client_ip, user_agent)
            .await
    }
}
