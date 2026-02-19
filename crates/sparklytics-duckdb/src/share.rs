use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::DuckDbBackend;

/// A single row returned by `export_events`.
///
/// `visitor_id` is intentionally omitted — it is a pseudonymous identifier
/// derived from IP + user-agent and must not be exported to prevent
/// re-identification of individual users.
#[derive(Debug, Clone, Serialize)]
pub struct ExportRow {
    pub id: String,
    pub website_id: String,
    pub event_type: String,
    pub url: String,
    pub referrer_domain: Option<String>,
    pub event_name: Option<String>,
    pub country: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device_type: Option<String>,
    pub language: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub created_at: String,
}

impl DuckDbBackend {
    /// Look up a website by its public `share_id`.
    ///
    /// Returns `(website_id, tenant_id)` when found, `None` otherwise.
    pub async fn get_website_by_share_id(
        &self,
        share_id: &str,
    ) -> Result<Option<(String, Option<String>)>> {
        let conn = self.conn.lock().await;
        let mut stmt =
            conn.prepare("SELECT id, tenant_id FROM websites WHERE share_id = ?1")?;
        match stmt.query_row(duckdb::params![share_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        }) {
            Ok(v) => Ok(Some(v)),
            Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    /// Assign a `share_id` to a website (enables public sharing).
    pub async fn set_share_id(&self, website_id: &str, share_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE websites SET share_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            duckdb::params![share_id, website_id],
        )?;
        Ok(())
    }

    /// Clear the `share_id` for a website (disables public sharing).
    pub async fn clear_share_id(&self, website_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE websites SET share_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            duckdb::params![website_id],
        )?;
        Ok(())
    }

    /// Retrieve the current `share_id` for a website (may be NULL/None).
    pub async fn get_share_id(&self, website_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        match conn
            .prepare("SELECT share_id FROM websites WHERE id = ?1")?
            .query_row(duckdb::params![website_id], |row| {
                row.get::<_, Option<String>>(0)
            }) {
            Ok(v) => Ok(v),
            Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    /// Export raw events for a date range.
    ///
    /// Returns at most 500 001 rows — the caller must check `len() > 500_000`
    /// and respond with 400 (too many rows) before serialising.
    ///
    /// IMPORTANT: CAST(created_at AS VARCHAR) — DuckDB cannot read TIMESTAMP
    /// columns as `String` via `row.get()` (MEMORY.md critical pattern).
    pub async fn export_events(
        &self,
        website_id: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ExportRow>> {
        let conn = self.conn.lock().await;
        // end is inclusive — add 1 day so the WHERE uses < next-day.
        let end_exclusive = end + chrono::Duration::days(1);
        let start_str = start.format("%Y-%m-%d").to_string();
        let end_str = end_exclusive.format("%Y-%m-%d").to_string();

        let mut stmt = conn.prepare(
            r#"SELECT id, website_id, event_type, url, referrer_domain, event_name,
                      country, browser, os, device_type, language,
                      utm_source, utm_medium, utm_campaign,
                      CAST(created_at AS VARCHAR) AS created_at
               FROM events
               WHERE website_id = ?1
                 AND created_at >= CAST(?2 AS TIMESTAMP)
                 AND created_at <  CAST(?3 AS TIMESTAMP)
               ORDER BY created_at
               LIMIT 500001"#,
        )?;
        let rows = stmt.query_map(duckdb::params![website_id, start_str, end_str], |row| {
            Ok(ExportRow {
                id: row.get(0)?,
                website_id: row.get(1)?,
                event_type: row.get(2)?,
                url: row.get(3)?,
                referrer_domain: row.get(4)?,
                event_name: row.get(5)?,
                country: row.get(6)?,
                browser: row.get(7)?,
                os: row.get(8)?,
                device_type: row.get(9)?,
                language: row.get(10)?,
                utm_source: row.get(11)?,
                utm_medium: row.get(12)?,
                utm_campaign: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}
