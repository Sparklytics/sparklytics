use anyhow::Result;
pub use sparklytics_metadata::ApiKeyRecord;

use crate::backend::rand_hex;
use crate::DuckDbBackend;

#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[cfg(test)]
static AUTH_WRITE_FAIL_AFTER: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(test)]
static AUTH_WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static AUTH_WRITE_TEST_LOCK: Mutex<()> = Mutex::new(());

impl DuckDbBackend {
    fn get_setting_tx(tx: &duckdb::Transaction<'_>, key: &str) -> Result<Option<String>> {
        let result = tx
            .prepare("SELECT value FROM settings WHERE key = ?1")?
            .query_row(duckdb::params![key], |row| row.get::<_, String>(0))
            .ok();
        Ok(result)
    }

    fn upsert_setting_tx(tx: &duckdb::Transaction<'_>, key: &str, value: &str) -> Result<()> {
        #[cfg(test)]
        {
            let write_index = AUTH_WRITE_COUNT.fetch_add(1, Ordering::SeqCst);
            let fail_after = AUTH_WRITE_FAIL_AFTER.load(Ordering::SeqCst);
            anyhow::ensure!(
                write_index < fail_after,
                "injected auth write failure at index {write_index}"
            );
        }

        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            duckdb::params![key, value],
        )?;
        Ok(())
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let result = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?
            .query_row(duckdb::params![key], |row| row.get::<_, String>(0))
            .ok();
        Ok(result)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            duckdb::params![key, value],
        )?;
        Ok(())
    }

    pub async fn complete_admin_setup(
        &self,
        password_hash: &str,
        password_change_required: bool,
    ) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let tx = conn.transaction()?;
        anyhow::ensure!(
            Self::get_setting_tx(&tx, "admin_password_hash")?.is_none(),
            "admin is already configured"
        );
        Self::upsert_setting_tx(&tx, "admin_password_hash", password_hash)?;
        Self::upsert_setting_tx(
            &tx,
            "password_change_required",
            if password_change_required {
                "true"
            } else {
                "false"
            },
        )?;
        tx.commit()?;
        Ok(())
    }

    pub async fn complete_password_change(
        &self,
        password_hash: &str,
        jwt_secret: &str,
    ) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let tx = conn.transaction()?;
        Self::upsert_setting_tx(&tx, "admin_password_hash", password_hash)?;
        Self::upsert_setting_tx(&tx, "jwt_secret", jwt_secret)?;
        Self::upsert_setting_tx(&tx, "password_change_required", "false")?;
        tx.commit()?;
        Ok(())
    }

    /// Ensure a JWT secret exists in settings. If not, generate one.
    /// Returns the JWT secret.
    pub async fn ensure_jwt_secret(&self) -> Result<String> {
        if let Some(secret) = self.get_setting("jwt_secret").await? {
            return Ok(secret);
        }
        let secret = rand_hex(32);
        self.set_setting("jwt_secret", &secret).await?;
        Ok(secret)
    }

    /// Check if admin password has been configured (for `local` mode).
    pub async fn is_admin_configured(&self) -> Result<bool> {
        let result = self.get_setting("admin_password_hash").await?;
        Ok(result.is_some())
    }

    /// Look up an API key by its hash. Returns None if not found or revoked.
    pub async fn lookup_api_key(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>> {
        let conn = self.conn.lock().await;
        let result = conn
            .prepare(
                "SELECT id, name, key_prefix, \
                 CAST(created_at AS VARCHAR), \
                 CAST(last_used_at AS VARCHAR), \
                 CAST(revoked_at AS VARCHAR) \
                 FROM local_api_keys WHERE key_hash = ?1 AND revoked_at IS NULL",
            )?
            .query_row(duckdb::params![key_hash], |row| {
                Ok(ApiKeyRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key_prefix: row.get(2)?,
                    created_at: row.get(3)?,
                    last_used_at: row.get(4)?,
                    revoked_at: row.get(5)?,
                })
            })
            .ok();
        Ok(result)
    }

    /// Update last_used_at for an API key.
    pub async fn touch_api_key(&self, key_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE local_api_keys SET last_used_at = CURRENT_TIMESTAMP WHERE id = ?1",
            duckdb::params![key_id],
        )?;
        Ok(())
    }

    /// Create a new API key record.
    pub async fn create_api_key(
        &self,
        id: &str,
        name: &str,
        hash: &str,
        prefix: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO local_api_keys (id, name, key_hash, key_prefix, created_at) \
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
            duckdb::params![id, name, hash, prefix],
        )?;
        Ok(())
    }

    /// Revoke an API key by setting revoked_at.
    pub async fn revoke_api_key(&self, key_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "UPDATE local_api_keys SET revoked_at = CURRENT_TIMESTAMP \
             WHERE id = ?1 AND revoked_at IS NULL",
            duckdb::params![key_id],
        )?;
        Ok(rows > 0)
    }

    /// List API keys (paginated). Includes revoked keys.
    pub async fn list_api_keys(&self, limit: i64, offset: i64) -> Result<(Vec<ApiKeyRecord>, i64)> {
        let conn = self.conn.lock().await;

        let total: i64 = conn
            .prepare("SELECT COUNT(*) FROM local_api_keys")?
            .query_row([], |row| row.get(0))?;

        let mut stmt = conn.prepare(
            "SELECT id, name, key_prefix, \
             CAST(created_at AS VARCHAR), \
             CAST(last_used_at AS VARCHAR), \
             CAST(revoked_at AS VARCHAR) \
             FROM local_api_keys ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(duckdb::params![limit, offset], |row| {
            Ok(ApiKeyRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                key_prefix: row.get(2)?,
                created_at: row.get(3)?,
                last_used_at: row.get(4)?,
                revoked_at: row.get(5)?,
            })
        })?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row?);
        }
        Ok((keys, total))
    }

    /// Record a login attempt for rate limiting.
    pub async fn record_login_attempt(&self, ip: &str, succeeded: bool) -> Result<()> {
        let id = rand_hex(5);
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO login_attempts (id, ip_address, attempted_at, succeeded) \
             VALUES (?1, ?2, CURRENT_TIMESTAMP, ?3)",
            duckdb::params![id, ip, succeeded],
        )?;
        Ok(())
    }

    /// Check if the IP is rate-limited (5 failed attempts in last 15 min).
    pub async fn check_login_rate_limit(&self, ip: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .prepare(
                "SELECT COUNT(*) FROM login_attempts \
                 WHERE ip_address = ?1 \
                 AND attempted_at > CAST(NOW() AS TIMESTAMP) - INTERVAL '15 minutes' \
                 AND succeeded = false",
            )?
            .query_row(duckdb::params![ip], |row| row.get(0))?;
        // Returns true if allowed (under limit), false if blocked.
        Ok(count < 5)
    }

    /// Delete stale login attempts older than 24 hours.
    pub async fn prune_login_attempts(&self) -> Result<u64> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "DELETE FROM login_attempts \
             WHERE attempted_at < CAST(NOW() AS TIMESTAMP) - INTERVAL '24 hours'",
            [],
        )?;
        Ok(rows as u64)
    }

    #[cfg(test)]
    fn inject_auth_write_failure_after(write_count: usize) {
        AUTH_WRITE_COUNT.store(0, Ordering::SeqCst);
        AUTH_WRITE_FAIL_AFTER.store(write_count, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn clear_auth_write_failure() {
        AUTH_WRITE_COUNT.store(0, Ordering::SeqCst);
        AUTH_WRITE_FAIL_AFTER.store(usize::MAX, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::{DuckDbBackend, AUTH_WRITE_TEST_LOCK};

    #[tokio::test]
    async fn complete_admin_setup_rolls_back_on_write_failure() {
        let _guard = AUTH_WRITE_TEST_LOCK.lock().expect("test lock");
        let db = DuckDbBackend::open_in_memory().expect("db");
        DuckDbBackend::inject_auth_write_failure_after(1);

        let result = db.complete_admin_setup("new_hash", true).await;

        DuckDbBackend::clear_auth_write_failure();
        assert!(result.is_err());
        assert_eq!(
            db.get_setting("admin_password_hash")
                .await
                .expect("setting read"),
            None
        );
        assert_eq!(
            db.get_setting("password_change_required")
                .await
                .expect("setting read"),
            None
        );
    }

    #[tokio::test]
    async fn complete_password_change_rolls_back_on_write_failure() {
        let _guard = AUTH_WRITE_TEST_LOCK.lock().expect("test lock");
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.set_setting("admin_password_hash", "old_hash")
            .await
            .expect("seed admin hash");
        db.set_setting("jwt_secret", "old_secret")
            .await
            .expect("seed jwt secret");
        db.set_setting("password_change_required", "true")
            .await
            .expect("seed rotation flag");

        DuckDbBackend::inject_auth_write_failure_after(1);
        let result = db.complete_password_change("new_hash", "new_secret").await;

        DuckDbBackend::clear_auth_write_failure();
        assert!(result.is_err());
        assert_eq!(
            db.get_setting("admin_password_hash")
                .await
                .expect("setting read"),
            Some("old_hash".to_string())
        );
        assert_eq!(
            db.get_setting("jwt_secret").await.expect("setting read"),
            Some("old_secret".to_string())
        );
        assert_eq!(
            db.get_setting("password_change_required")
                .await
                .expect("setting read"),
            Some("true".to_string())
        );
    }

    #[tokio::test]
    async fn complete_admin_setup_refuses_to_overwrite_existing_admin() {
        let _guard = AUTH_WRITE_TEST_LOCK.lock().expect("test lock");
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.set_setting("admin_password_hash", "existing_hash")
            .await
            .expect("seed admin hash");

        let result = db.complete_admin_setup("new_hash", false).await;

        assert!(result.is_err());
        assert_eq!(
            db.get_setting("admin_password_hash")
                .await
                .expect("setting read"),
            Some("existing_hash".to_string())
        );
    }
}
