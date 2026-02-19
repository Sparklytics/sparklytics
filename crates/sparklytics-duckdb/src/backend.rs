use std::sync::Arc;

use anyhow::Result;
use duckdb::Connection;
use tokio::sync::Mutex;
use tracing::info;

use sparklytics_core::event::Event;

use crate::schema::{init_sql, MIGRATIONS_TABLE_SQL};

/// Generate a cryptographically random hex string of `n` bytes (2n hex chars).
pub(crate) fn rand_hex(n: usize) -> String {
    use rand::RngCore;
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// A DuckDB backend for Sparklytics.
///
/// DuckDB is single-writer: concurrent reads are fine, but concurrent writes
/// cause contention. We wrap the connection in `Arc<Mutex<_>>` so the async
/// runtime serialises all writes through the buffer-flush task while still
/// allowing the struct to be cheaply cloned and shared across Axum handlers.
///
/// Memory and thread limits are enforced by [`init_sql`] at open time.
/// The memory limit is configurable via `SPARKLYTICS_DUCKDB_MEMORY`
/// (default `"1GB"`). See CLAUDE.md critical fact #12.
///
/// `tenant_id` is always `NULL` in self-hosted mode (critical fact #2).
pub struct DuckDbBackend {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl DuckDbBackend {
    /// Open (or create) a DuckDB database file at `path`.
    ///
    /// `memory_limit` is a DuckDB size string such as `"1GB"` or `"512MB"`.
    /// It is read from `Config.duckdb_memory_limit` at the call site.
    /// Runs [`MIGRATIONS_TABLE_SQL`] then the schema init SQL on the connection
    /// so all tables and indexes are created if they do not already exist.
    pub fn open(path: &str, memory_limit: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(MIGRATIONS_TABLE_SQL)?;
        conn.execute_batch(&init_sql(memory_limit))?;
        // Seed settings (daily_salt, install_id, etc.) if this is a fresh database.
        Self::seed_settings_sync(&conn)?;
        info!(
            "DuckDB opened at {} with memory_limit={}, threads=2",
            path, memory_limit
        );
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an **in-memory** DuckDB database.
    ///
    /// Intended for unit tests only — data is discarded when the struct is
    /// dropped. Uses a 1GB memory limit (tests are not memory-constrained).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(MIGRATIONS_TABLE_SQL)?;
        conn.execute_batch(&init_sql("1GB"))?;
        Self::seed_settings_sync(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Seed the `settings` table with initial values if they don't already exist.
    ///
    /// Uses `INSERT OR IGNORE` so re-runs on every startup are safe.
    /// - `daily_salt`:    32-byte random hex, used for visitor_id hashing
    /// - `previous_salt`: same as daily_salt initially; updated by midnight rotation
    /// - `version`:       schema version "1"
    /// - `install_id`:    unique 8-byte hex installation identifier
    fn seed_settings_sync(conn: &Connection) -> Result<()> {
        let salt = rand_hex(32);
        let install_id = rand_hex(8);
        // Use separate parameterized execute() calls — DuckDB does not support
        // multi-statement batches with parameters, and format!() into SQL is forbidden.
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('daily_salt', ?1)",
            duckdb::params![salt],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('previous_salt', ?1)",
            duckdb::params![salt],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('version', ?1)",
            duckdb::params!["1"],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('install_id', ?1)",
            duckdb::params![install_id],
        )?;
        Ok(())
    }

    /// Read the current `daily_salt` from the `settings` table.
    pub async fn get_daily_salt(&self) -> Result<String> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'daily_salt'")?;
        let salt: String = stmt.query_row([], |row| row.get(0))?;
        Ok(salt)
    }

    /// Rotate the daily salt at midnight UTC.
    ///
    /// Moves `daily_salt` → `previous_salt` (for the 5-min grace period),
    /// then generates a new `daily_salt`. Both updates run in a single
    /// transaction so there is never a window with a missing salt.
    pub async fn rotate_salt(&self) -> Result<()> {
        let mut conn = self.conn.lock().await;
        let tx = conn.transaction()?;
        let new_salt = rand_hex(32);
        // Copy current daily_salt → previous_salt
        tx.execute_batch(
            "UPDATE settings SET value = (SELECT value FROM settings WHERE key = 'daily_salt') \
             WHERE key = 'previous_salt'",
        )?;
        // Generate fresh daily_salt
        tx.execute(
            "UPDATE settings SET value = ?1 WHERE key = 'daily_salt'",
            duckdb::params![new_salt],
        )?;
        tx.commit()?;
        tracing::info!("Daily salt rotated");
        Ok(())
    }

    /// Insert a batch of enriched events in a single transaction.
    ///
    /// Called by the buffer-flush background task. Each `Event` must already
    /// have `visitor_id`, `session_id`, GeoIP fields, and UA fields populated
    /// by the enrichment layer in `sparklytics-server`.
    ///
    /// Returns immediately (no-op) if `events` is empty.
    ///
    /// IMPORTANT: `tenant_id` is always `None` / `NULL` in self-hosted mode.
    pub async fn insert_events(&self, events: &[Event]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().await;

        // Wrap the entire batch in a single transaction for atomicity and
        // throughput (one fsync instead of N).
        let tx = conn.transaction()?;

        for event in events {
            tx.execute(
                r#"INSERT INTO events (
                    id, website_id, tenant_id, session_id, visitor_id,
                    event_type, url, referrer_url, referrer_domain,
                    event_name, event_data,
                    country, region, city,
                    browser, browser_version, os, os_version, device_type,
                    screen, language,
                    utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                    created_at
                ) VALUES (
                    ?1,  ?2,  ?3,  ?4,  ?5,
                    ?6,  ?7,  ?8,  ?9,
                    ?10, ?11,
                    ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19,
                    ?20, ?21,
                    ?22, ?23, ?24, ?25, ?26,
                    ?27
                )"#,
                duckdb::params![
                    event.id,
                    event.website_id,
                    event.tenant_id,
                    event.session_id,
                    event.visitor_id,
                    event.event_type,
                    event.url,
                    event.referrer_url,
                    event.referrer_domain,
                    event.event_name,
                    event.event_data,
                    event.country,
                    event.region,
                    event.city,
                    event.browser,
                    event.browser_version,
                    event.os,
                    event.os_version,
                    event.device_type,
                    event.screen,
                    event.language,
                    event.utm_source,
                    event.utm_medium,
                    event.utm_campaign,
                    event.utm_term,
                    event.utm_content,
                    event.created_at.to_rfc3339(),
                ],
            )?;
        }

        tx.commit()?;
        tracing::info!("Inserted {} events into DuckDB", events.len());
        Ok(())
    }

    /// Return `true` if a website with the given `website_id` exists in the
    /// `websites` table.
    ///
    /// Used at collect time to reject events for unknown sites before they
    /// enter the event buffer.
    pub async fn website_exists(&self, website_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM websites WHERE id = ?1")?;
        let count: i64 = stmt.query_row(duckdb::params![website_id], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Execute `SELECT 1` as a lightweight liveness check.
    ///
    /// Called by the `/health` endpoint. Returns an error if the connection
    /// is unavailable (file locked, disk full, etc.).
    pub async fn ping(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch("SELECT 1")?;
        Ok(())
    }

    /// Acquire the DuckDB connection lock for direct queries.
    ///
    /// Intended for integration tests that need to verify stored data.
    /// Production code should use the typed methods above.
    pub async fn conn_for_test(&self) -> tokio::sync::MutexGuard<'_, Connection> {
        self.conn.lock().await
    }

    /// Insert or replace a website row.
    ///
    /// Intended for test fixtures and the website-management API. Uses
    /// `INSERT OR REPLACE` so it is safe to call repeatedly with the same `id`.
    ///
    /// `tenant_id` is always `NULL` in self-hosted mode.
    pub async fn seed_website(&self, id: &str, domain: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            r#"INSERT INTO websites (id, name, domain, timezone, created_at, updated_at)
               VALUES (?1, ?2, ?3, 'UTC', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
               ON CONFLICT (id) DO UPDATE SET domain = EXCLUDED.domain"#,
            duckdb::params![id, domain, domain],
        )?;
        Ok(())
    }
}
