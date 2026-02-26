use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use sparklytics_core::analytics::{
    AnalyticsFilter, SessionListItem, SessionSort, SessionsPagination, SessionsQuery,
    SessionsResponse,
};

use crate::queries::bot_filters::{append_event_bot_filter, append_session_bot_filter};
use crate::DuckDbBackend;

#[derive(Debug, Serialize, Deserialize)]
struct CursorPayload {
    ls: String,
    sid: String,
}

fn encode_cursor(last_seen: &str, session_id: &str) -> Result<String> {
    let payload = CursorPayload {
        ls: last_seen.to_string(),
        sid: session_id.to_string(),
    };
    let json = serde_json::to_vec(&payload)?;
    Ok(STANDARD.encode(json))
}

fn decode_cursor(cursor: &str) -> Result<CursorPayload> {
    let decoded = STANDARD
        .decode(cursor)
        .map_err(|_| anyhow!("invalid_cursor"))?;
    serde_json::from_slice::<CursorPayload>(&decoded).map_err(|_| anyhow!("invalid_cursor"))
}

fn append_event_filters(
    filter: &AnalyticsFilter,
    filter_sql: &mut String,
    params: &mut Vec<Box<dyn duckdb::types::ToSql>>,
    param_idx: &mut usize,
) {
    append_event_bot_filter(filter_sql, filter.include_bots, "e.");
    if let Some(ref country) = filter.filter_country {
        filter_sql.push_str(&format!(" AND e.country = ?{}", *param_idx));
        params.push(Box::new(country.clone()));
        *param_idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        filter_sql.push_str(&format!(" AND e.url LIKE ?{}", *param_idx));
        params.push(Box::new(format!("%{}%", page)));
        *param_idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        filter_sql.push_str(&format!(" AND e.referrer_domain = ?{}", *param_idx));
        params.push(Box::new(referrer.clone()));
        *param_idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        filter_sql.push_str(&format!(" AND e.browser = ?{}", *param_idx));
        params.push(Box::new(browser.clone()));
        *param_idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        filter_sql.push_str(&format!(" AND e.os = ?{}", *param_idx));
        params.push(Box::new(os.clone()));
        *param_idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        filter_sql.push_str(&format!(" AND e.device_type = ?{}", *param_idx));
        params.push(Box::new(device.clone()));
        *param_idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        filter_sql.push_str(&format!(" AND e.language = ?{}", *param_idx));
        params.push(Box::new(language.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        filter_sql.push_str(&format!(" AND e.utm_source = ?{}", *param_idx));
        params.push(Box::new(utm_source.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        filter_sql.push_str(&format!(" AND e.utm_medium = ?{}", *param_idx));
        params.push(Box::new(utm_medium.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        filter_sql.push_str(&format!(" AND e.utm_campaign = ?{}", *param_idx));
        params.push(Box::new(utm_campaign.clone()));
        *param_idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        filter_sql.push_str(&format!(" AND e.region = ?{}", *param_idx));
        params.push(Box::new(region.clone()));
        *param_idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        filter_sql.push_str(&format!(" AND e.city = ?{}", *param_idx));
        params.push(Box::new(city.clone()));
        *param_idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        filter_sql.push_str(&format!(
            " AND lower(regexp_extract(e.url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            *param_idx
        ));
        params.push(Box::new(hostname.clone()));
        *param_idx += 1;
    }
}

pub async fn get_sessions_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    query: &SessionsQuery,
) -> Result<SessionsResponse> {
    if query.limit == 0 {
        return Err(anyhow!("invalid limit"));
    }
    if query.limit > 200 {
        return Err(anyhow!("invalid limit"));
    }
    if query.sort != SessionSort::LastSeenDesc {
        return Err(anyhow!("unsupported sort"));
    }

    let conn = db.conn.lock().await;

    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut filter_sql = String::new();
    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut param_idx = 4;
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);
    let mut session_filter_sql = String::new();
    append_session_bot_filter(&mut session_filter_sql, filter.include_bots, "s.");

    let mut cursor_clause = String::new();
    if let Some(ref raw_cursor) = query.cursor {
        let cursor = decode_cursor(raw_cursor)?;
        cursor_clause.push_str(&format!(
            " AND (sr.last_seen_ts < CAST(?{} AS TIMESTAMP) OR \
               (sr.last_seen_ts = CAST(?{} AS TIMESTAMP) AND sr.session_id < ?{}))",
            param_idx,
            param_idx,
            param_idx + 1
        ));
        params.push(Box::new(cursor.ls));
        params.push(Box::new(cursor.sid));
    }

    let limit_plus_one = i64::from(query.limit) + 1;

    let sql = format!(
        r#"
        WITH filtered_events AS (
            SELECT e.session_id
            FROM events e
            WHERE e.website_id = ?1
              AND e.created_at >= ?2
              AND e.created_at < ?3
              {filter_sql}
        ),
        matching_sessions AS (
            SELECT DISTINCT session_id
            FROM filtered_events
        ),
        session_events AS (
            SELECT
                e.session_id,
                e.url,
                e.created_at,
                e.country,
                e.browser,
                e.os,
                e.device_type
            FROM events e
            WHERE e.website_id = ?1
              AND e.session_id IN (SELECT session_id FROM matching_sessions)
        ),
        session_rollup AS (
            SELECT
                s.session_id AS session_id,
                s.visitor_id AS visitor_id,
                s.first_seen AS first_seen_ts,
                s.last_seen AS last_seen_ts,
                date_diff('second', s.first_seen, s.last_seen) AS duration_seconds,
                CAST(s.pageview_count AS BIGINT) AS pageview_count,
                COUNT(se.url) AS event_count,
                s.entry_page AS entry_page,
                LAST(se.url ORDER BY se.created_at) AS exit_page,
                LAST(se.country ORDER BY se.created_at) AS country,
                LAST(se.browser ORDER BY se.created_at) AS browser,
                LAST(se.os ORDER BY se.created_at) AS os,
                LAST(se.device_type ORDER BY se.created_at) AS device_type
            FROM sessions s
            JOIN matching_sessions ms ON ms.session_id = s.session_id
            LEFT JOIN session_events se ON se.session_id = s.session_id
            WHERE s.website_id = ?1
              {session_filter_sql}
            GROUP BY
                s.session_id,
                s.visitor_id,
                s.first_seen,
                s.last_seen,
                s.pageview_count,
                s.entry_page
        ),
        cursor_filtered AS (
            SELECT *
            FROM session_rollup sr
            WHERE 1 = 1
              {cursor_clause}
        )
        SELECT
            session_id,
            visitor_id,
            CAST(first_seen_ts AS VARCHAR) AS first_seen,
            CAST(last_seen_ts AS VARCHAR) AS last_seen,
            duration_seconds,
            pageview_count,
            event_count,
            entry_page,
            exit_page,
            country,
            browser,
            os,
            device_type
        FROM cursor_filtered
        ORDER BY last_seen_ts DESC, session_id DESC
        LIMIT {limit_plus_one}
        "#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
    })?;

    let mut mapped_rows = Vec::new();
    for row in rows {
        mapped_rows.push(row?);
    }

    let has_more = mapped_rows.len() > query.limit as usize;
    if has_more {
        mapped_rows.pop();
    }

    let next_cursor = if has_more {
        if let Some(last) = mapped_rows.last() {
            Some(encode_cursor(&last.last_seen, &last.session_id)?)
        } else {
            None
        }
    } else {
        None
    };

    Ok(SessionsResponse {
        rows: mapped_rows,
        pagination: SessionsPagination {
            limit: query.limit,
            next_cursor,
            has_more,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_cursor, encode_cursor};

    #[test]
    fn test_cursor_round_trip() {
        let encoded = encode_cursor("2026-01-01 10:00:00", "sess_1").expect("encode");
        let decoded = decode_cursor(&encoded).expect("decode");
        assert_eq!(decoded.ls, "2026-01-01 10:00:00");
        assert_eq!(decoded.sid, "sess_1");
    }

    #[test]
    fn test_invalid_cursor_base64() {
        let err = decode_cursor("not-valid-@@@").expect_err("should fail");
        assert!(err.to_string().contains("invalid_cursor"));
    }
}
