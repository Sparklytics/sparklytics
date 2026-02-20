use anyhow::{anyhow, Result};

use sparklytics_core::analytics::{SessionDetailResponse, SessionEventItem, SessionListItem};

use crate::DuckDbBackend;

const MAX_EVENTS_PER_SESSION: usize = 2000;

pub async fn get_session_detail_inner(
    db: &DuckDbBackend,
    website_id: &str,
    session_id: &str,
) -> Result<SessionDetailResponse> {
    let conn = db.conn.lock().await;

    let summary_sql = r#"
        WITH session_events AS (
            SELECT
                e.url,
                e.created_at,
                e.country,
                e.browser,
                e.os,
                e.device_type
            FROM events e
            WHERE e.website_id = ?1
              AND e.session_id = ?2
        )
        SELECT
            s.session_id,
            s.visitor_id,
            CAST(s.first_seen AS VARCHAR) AS first_seen,
            CAST(s.last_seen AS VARCHAR) AS last_seen,
            date_diff('second', s.first_seen, s.last_seen) AS duration_seconds,
            CAST(s.pageview_count AS BIGINT) AS pageview_count,
            COUNT(se.url) AS event_count,
            s.entry_page,
            LAST(se.url ORDER BY se.created_at) AS exit_page,
            LAST(se.country ORDER BY se.created_at) AS country,
            LAST(se.browser ORDER BY se.created_at) AS browser,
            LAST(se.os ORDER BY se.created_at) AS os,
            LAST(se.device_type ORDER BY se.created_at) AS device_type
        FROM sessions s
        LEFT JOIN session_events se ON true
        WHERE s.website_id = ?1
          AND s.session_id = ?2
        GROUP BY
            s.session_id,
            s.visitor_id,
            s.first_seen,
            s.last_seen,
            s.pageview_count,
            s.entry_page
    "#;

    let summary = conn
        .prepare(summary_sql)?
        .query_row(duckdb::params![website_id, session_id], |row| {
            Ok(SessionListItem {
                session_id: row.get(0)?,
                visitor_id: row.get(1)?,
                first_seen: row.get(2)?,
                last_seen: row.get(3)?,
                duration_seconds: row.get(4)?,
                pageview_count: row.get(5)?,
                event_count: row.get(6)?,
                entry_page: row.get(7)?,
                exit_page: row.get(8)?,
                country: row.get(9)?,
                browser: row.get(10)?,
                os: row.get(11)?,
                device_type: row.get(12)?,
            })
        })
        .ok();

    let session = summary.ok_or_else(|| anyhow!("Session not found"))?;

    let events_sql = format!(
        r#"
        SELECT
            id,
            event_type,
            url,
            event_name,
            event_data,
            CAST(created_at AS VARCHAR) AS created_at
        FROM events
        WHERE website_id = ?1
          AND session_id = ?2
        ORDER BY created_at ASC
        LIMIT {}
        "#,
        MAX_EVENTS_PER_SESSION + 1
    );

    let mut stmt = conn.prepare(&events_sql)?;
    let rows = stmt.query_map(duckdb::params![website_id, session_id], |row| {
        Ok(SessionEventItem {
            id: row.get(0)?,
            event_type: row.get(1)?,
            url: row.get(2)?,
            event_name: row.get(3)?,
            event_data: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }

    let truncated = events.len() > MAX_EVENTS_PER_SESSION;
    if truncated {
        events.truncate(MAX_EVENTS_PER_SESSION);
    }

    Ok(SessionDetailResponse {
        session,
        events,
        truncated,
    })
}
