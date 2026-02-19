use anyhow::Result;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::DuckDbBackend;

/// Result of a session lookup/create operation.
pub struct SessionResult {
    pub session_id: String,
    pub is_new: bool,
}

impl DuckDbBackend {
    /// Look up or create a session for the given visitor on the given website.
    ///
    /// Logic:
    /// 1. Query most recent session where `last_seen > now - 30min` (strictly greater —
    ///    exactly 30min = new session).
    /// 2. If found: UPDATE `last_seen = now`, `pageview_count += 1`, return existing session_id.
    /// 3. If not found: INSERT new session with `ON CONFLICT (session_id) DO UPDATE`
    ///    for race-condition safety.
    ///
    /// `session_id = sha256(visitor_id + website_id + entry_page + first_seen_ms)[0:16]`
    pub async fn get_or_create_session(
        &self,
        visitor_id: &str,
        website_id: &str,
        url: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionResult> {
        let conn = self.conn.lock().await;
        let cutoff = now - chrono::Duration::minutes(30);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S%.f").to_string();

        // Look for an active session (last_seen strictly greater than cutoff).
        let mut stmt = conn.prepare(
            "SELECT session_id, pageview_count FROM sessions \
             WHERE visitor_id = ?1 AND website_id = ?2 AND last_seen > ?3 \
             ORDER BY last_seen DESC LIMIT 1",
        )?;

        let now_str = now.format("%Y-%m-%d %H:%M:%S%.f").to_string();

        let existing: Option<(String, i32)> = stmt
            .query_row(
                duckdb::params![visitor_id, website_id, cutoff_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((session_id, pageview_count)) = existing {
            // Update existing session.
            conn.execute(
                "UPDATE sessions SET last_seen = ?1, pageview_count = ?2 WHERE session_id = ?3",
                duckdb::params![now_str, pageview_count + 1, session_id],
            )?;
            return Ok(SessionResult {
                session_id,
                is_new: false,
            });
        }

        // Create a new session.
        let first_seen_ms = now.timestamp_millis();
        let session_id = compute_session_id(visitor_id, website_id, url, first_seen_ms);

        // Use ON CONFLICT for race-condition safety: if another request created
        // the same session_id concurrently, just update it.
        conn.execute(
            "INSERT INTO sessions (session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page) \
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, 1, ?6) \
             ON CONFLICT (session_id) DO UPDATE SET last_seen = EXCLUDED.last_seen, pageview_count = sessions.pageview_count + 1",
            duckdb::params![session_id, website_id, visitor_id, now_str, now_str, url],
        )?;

        Ok(SessionResult {
            session_id,
            is_new: true,
        })
    }
}

/// Compute a deterministic session ID.
///
/// `session_id = sha256(visitor_id + website_id + entry_page + first_seen_ms)[0:16]`
fn compute_session_id(
    visitor_id: &str,
    website_id: &str,
    entry_page: &str,
    first_seen_ms: i64,
) -> String {
    let input = format!("{}{}{}{}", visitor_id, website_id, entry_page, first_seen_ms);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(&hash[..8])
}
