use anyhow::Result;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::DuckDbBackend;

/// Result of a session lookup/create operation.
pub struct SessionResult {
    pub session_id: String,
    pub is_new: bool,
}

/// Look up or create a session for the given visitor on the given website.
///
/// Parameter order is intentionally `(visitor_id, website_id, url, now)` to match
/// existing call sites in this crate.
pub(crate) async fn get_or_create_session_inner(
    db: &DuckDbBackend,
    visitor_id: &str,
    website_id: &str,
    url: &str,
    now: DateTime<Utc>,
) -> Result<SessionResult> {
    let conn = db.conn.lock().await;
    let cutoff = now - chrono::Duration::minutes(30);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S%.f").to_string();

    let mut stmt = conn.prepare(
        "SELECT session_id, pageview_count FROM sessions \
         WHERE visitor_id = ?1 AND website_id = ?2 AND last_seen > ?3 \
         ORDER BY last_seen DESC LIMIT 1",
    )?;

    let now_str = now.format("%Y-%m-%d %H:%M:%S%.f").to_string();

    let existing: Option<(String, i32)> = stmt
        .query_row(duckdb::params![visitor_id, website_id, cutoff_str], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .ok();

    if let Some((session_id, pageview_count)) = existing {
        conn.execute(
            "UPDATE sessions SET last_seen = ?1, pageview_count = ?2 WHERE session_id = ?3",
            duckdb::params![now_str, pageview_count + 1, session_id],
        )?;
        return Ok(SessionResult {
            session_id,
            is_new: false,
        });
    }

    let first_seen_ms = now.timestamp_millis();
    let session_id = compute_session_id(visitor_id, website_id, url, first_seen_ms);

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

pub(crate) async fn increment_session_pageviews_inner(
    db: &DuckDbBackend,
    session_id: &str,
    additional_pageviews: u32,
    now: DateTime<Utc>,
) -> Result<()> {
    if additional_pageviews == 0 {
        return Ok(());
    }

    let conn = db.conn.lock().await;
    let now_str = now.format("%Y-%m-%d %H:%M:%S%.f").to_string();
    conn.execute(
        "UPDATE sessions
         SET last_seen = ?1, pageview_count = pageview_count + ?2
         WHERE session_id = ?3",
        duckdb::params![now_str, additional_pageviews as i64, session_id],
    )?;

    Ok(())
}

pub(crate) async fn set_session_bot_classification_inner(
    db: &DuckDbBackend,
    session_id: &str,
    is_bot: bool,
    bot_score: i32,
    bot_reason: Option<&str>,
) -> Result<()> {
    let conn = db.conn.lock().await;
    conn.execute(
        "UPDATE sessions
         SET is_bot = CASE WHEN ?2 THEN TRUE ELSE is_bot END,
             bot_score = GREATEST(bot_score, ?3),
             bot_reason = CASE
                 WHEN ?4 IS NULL THEN bot_reason
                 WHEN ?3 >= bot_score THEN ?4
                 ELSE bot_reason
             END
         WHERE session_id = ?1",
        duckdb::params![session_id, is_bot, bot_score, bot_reason],
    )?;
    Ok(())
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
    let input = format!(
        "{}{}{}{}",
        visitor_id, website_id, entry_page, first_seen_ms
    );
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(&hash[..8])
}
