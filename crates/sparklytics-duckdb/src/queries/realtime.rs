use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::DuckDbBackend;

#[derive(Debug, Clone, Serialize)]
pub struct RealtimeEvent {
    pub url: String,
    pub referrer_domain: Option<String>,
    pub country: Option<String>,
    pub browser: Option<String>,
    pub device_type: Option<String>,
    pub event_type: String,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealtimePagination {
    pub limit: i64,
    pub total_in_window: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealtimeResult {
    pub active_visitors: i64,
    pub recent_events: Vec<RealtimeEvent>,
    pub pagination: RealtimePagination,
}

pub async fn get_realtime_inner(
    db: &DuckDbBackend,
    website_id: &str,
    include_bots: bool,
) -> Result<RealtimeResult> {
    let conn = db.conn.lock().await;
    let now = Utc::now();
    let cutoff = now - chrono::Duration::minutes(30);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S%.f").to_string();
    let session_bot_filter = if include_bots {
        ""
    } else {
        " AND is_bot = FALSE"
    };
    let event_bot_filter = if include_bots {
        ""
    } else {
        " AND is_bot = FALSE"
    };

    let active_visitors: i64 = conn
        .prepare(&format!(
            "SELECT COUNT(DISTINCT visitor_id) FROM sessions \
             WHERE website_id = ?1 AND last_seen > ?2{session_bot_filter}",
        ))?
        .query_row(duckdb::params![website_id, cutoff_str], |row| row.get(0))?;

    let total_in_window: i64 = conn
        .prepare(&format!(
            "SELECT COUNT(*) FROM events \
             WHERE website_id = ?1 AND created_at > ?2{event_bot_filter}",
        ))?
        .query_row(duckdb::params![website_id, cutoff_str], |row| row.get(0))?;

    let mut stmt = conn.prepare(&format!(
        "SELECT url, referrer_domain, country, browser, device_type, event_type, CAST(created_at AS VARCHAR) \
         FROM events \
         WHERE website_id = ?1 AND created_at > ?2{event_bot_filter} \
         ORDER BY created_at DESC \
         LIMIT 100",
    ))?;

    let rows = stmt.query_map(duckdb::params![website_id, cutoff_str], |row| {
        Ok(RealtimeEvent {
            url: row.get(0)?,
            referrer_domain: row.get(1)?,
            country: row.get(2)?,
            browser: row.get(3)?,
            device_type: row.get(4)?,
            event_type: row.get(5)?,
            ts: row.get::<_, String>(6)?,
        })
    })?;

    let mut recent_events = Vec::new();
    for row in rows {
        recent_events.push(row?);
    }

    Ok(RealtimeResult {
        active_visitors,
        recent_events,
        pagination: RealtimePagination {
            limit: 100,
            total_in_window,
        },
    })
}

impl DuckDbBackend {
    pub async fn get_realtime(
        &self,
        website_id: &str,
        include_bots: bool,
    ) -> Result<RealtimeResult> {
        get_realtime_inner(self, website_id, include_bots).await
    }
}
