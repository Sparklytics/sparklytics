use anyhow::Result;
pub use sparklytics_metadata::ApiKeyRecord;

use crate::backend::rand_hex;
use crate::DuckDbBackend;

impl DuckDbBackend {
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
}
