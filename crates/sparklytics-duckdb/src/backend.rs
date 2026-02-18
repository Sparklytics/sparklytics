use std::sync::Arc;

use anyhow::Result;
use duckdb::Connection;
use tokio::sync::Mutex;
use tracing::info;

use sparklytics_core::event::Event;

use crate::schema::{INIT_SQL, MIGRATIONS_TABLE_SQL};

/// A DuckDB backend for Sparklytics.
///
/// DuckDB is single-writer: concurrent reads are fine, but concurrent writes
/// cause contention. We wrap the connection in `Arc<Mutex<_>>` so the async
/// runtime serialises all writes through the buffer-flush task while still
/// allowing the struct to be cheaply cloned and shared across Axum handlers.
///
/// Memory and thread limits are enforced by [`INIT_SQL`] at open time
/// (`SET memory_limit = '128MB'; SET threads = 2;`) per CLAUDE.md critical
/// fact #12.
///
/// `tenant_id` is always `NULL` in self-hosted mode (critical fact #2).
pub struct DuckDbBackend {
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbBackend {
    /// Open (or create) a DuckDB database file at `path`.
    ///
    /// Runs [`MIGRATIONS_TABLE_SQL`] then [`INIT_SQL`] on the connection so
    /// all tables and indexes are created if they do not already exist.
    /// The memory limit and thread cap defined in [`INIT_SQL`] are applied
    /// at this point.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(MIGRATIONS_TABLE_SQL)?;
        conn.execute_batch(INIT_SQL)?;
        info!("DuckDB opened at {} with memory_limit=128MB, threads=2", path);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an **in-memory** DuckDB database.
    ///
    /// Intended for unit tests only — data is discarded when the struct is
    /// dropped.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(MIGRATIONS_TABLE_SQL)?;
        conn.execute_batch(INIT_SQL)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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

        let conn = self.conn.lock().await;

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
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM websites WHERE id = ?1",
        )?;
        let count: i64 =
            stmt.query_row(duckdb::params![website_id], |row| row.get(0))?;
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

    /// Insert or replace a website row.
    ///
    /// Intended for test fixtures and the website-management API. Uses
    /// `INSERT OR REPLACE` so it is safe to call repeatedly with the same `id`.
    ///
    /// `tenant_id` is always `NULL` in self-hosted mode.
    pub async fn seed_website(&self, id: &str, domain: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            r#"INSERT OR REPLACE INTO websites (id, name, domain, timezone, created_at, updated_at)
               VALUES (?1, ?2, ?3, 'UTC', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
            duckdb::params![id, domain, domain],
        )?;
        Ok(())
    }
}
