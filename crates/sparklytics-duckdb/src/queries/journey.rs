use anyhow::{anyhow, Result};
use chrono::{LocalResult, TimeZone};
use chrono_tz::Tz;

use sparklytics_core::analytics::{
    AnalyticsFilter, AnchorType, JourneyBranch, JourneyDirection, JourneyNode, JourneyQuery,
    JourneyResponse,
};

use crate::queries::bot_filters::append_event_bot_filter;
use crate::DuckDbBackend;

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
        filter_sql.push_str(&format!(" AND position(?{} in e.url) > 0", *param_idx));
        params.push(Box::new(page.clone()));
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
            " AND lower(regexp_extract(e.url, '^https?://([^/:?#]+)', 1)) = lower(?{})",
            *param_idx
        ));
        params.push(Box::new(hostname.clone()));
        *param_idx += 1;
    }
}

fn resolve_timezone(
    conn: &duckdb::Connection,
    website_id: &str,
    requested_timezone: Option<&str>,
) -> Result<Tz> {
    if let Some(raw) = requested_timezone {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("invalid_timezone"));
        }
        return trimmed
            .parse::<Tz>()
            .map_err(|_| anyhow!("invalid_timezone"));
    }

    let website_tz: String = conn
        .prepare("SELECT timezone FROM websites WHERE id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))
        .unwrap_or_else(|_| "UTC".to_string());

    website_tz
        .parse::<Tz>()
        .or_else(|_| "UTC".parse::<Tz>())
        .map_err(|_| anyhow!("invalid_timezone"))
}

fn local_midnight_utc(tz: Tz, date: chrono::NaiveDate) -> Result<chrono::NaiveDateTime> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid_date_boundary"))?;
    let zoned = match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(a, b) => a.min(b),
        LocalResult::None => return Err(anyhow!("invalid_timezone_transition")),
    };
    Ok(zoned.with_timezone(&chrono::Utc).naive_utc())
}

fn utc_bounds_for_filter(
    tz: Tz,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
) -> Result<(String, String)> {
    let start_utc = local_midnight_utc(tz, start_date)?;
    let end_next_utc = local_midnight_utc(tz, end_date + chrono::Duration::days(1))?;
    Ok((
        start_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
        end_next_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
    ))
}

pub(crate) fn normalize_url_rust(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);

    let mut normalized = without_query.trim().to_string();
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized.to_lowercase()
    }
}

fn build_journey_sql(
    direction: &JourneyDirection,
    filter_sql: &str,
    max_depth_param_idx: usize,
) -> String {
    let (steps_cte, steps_order) = match direction {
        JourneyDirection::Next => (
            format!(
                r#"
journey_steps AS (
    SELECT
        ap.session_id,
        e.seq - ap.anchor_seq AS step_offset,
        e.node_value
    FROM anchor_positions ap
    JOIN ordered_events e
      ON e.session_id = ap.session_id
     AND e.seq > ap.anchor_seq
     AND e.seq <= ap.anchor_seq + ?{max_depth_param_idx}
),
"#
            ),
            "js.step_offset ASC",
        ),
        JourneyDirection::Previous => (
            format!(
                r#"
journey_steps AS (
    SELECT
        ap.session_id,
        ap.anchor_seq - e.seq AS step_offset,
        e.node_value
    FROM anchor_positions ap
    JOIN ordered_events e
      ON e.session_id = ap.session_id
     AND e.seq < ap.anchor_seq
     AND e.seq >= ap.anchor_seq - ?{max_depth_param_idx}
),
"#
            ),
            "js.step_offset DESC",
        ),
    };

    format!(
        r#"
WITH ordered_events AS (
    SELECT
        e.session_id,
        CASE
            WHEN e.event_type = 'pageview' THEN 'page'
            ELSE 'event'
        END AS node_type,
        CASE
            WHEN e.event_type = 'pageview' THEN
                lower(
                    CASE
                        WHEN length(split_part(split_part(e.url, '#', 1), '?', 1)) > 1
                            AND right(split_part(split_part(e.url, '#', 1), '?', 1), 1) = '/'
                        THEN substr(
                            split_part(split_part(e.url, '#', 1), '?', 1),
                            1,
                            length(split_part(split_part(e.url, '#', 1), '?', 1)) - 1
                        )
                        ELSE split_part(split_part(e.url, '#', 1), '?', 1)
                    END
                )
            ELSE e.event_name
        END AS node_value,
        ROW_NUMBER() OVER (PARTITION BY e.session_id ORDER BY e.created_at ASC, e.id ASC) AS seq
    FROM events e
    WHERE e.website_id = ?1
      AND e.created_at >= CAST(?2 AS TIMESTAMP)
      AND e.created_at < CAST(?3 AS TIMESTAMP)
      AND (
        (e.event_type = 'pageview' AND e.url IS NOT NULL)
        OR (e.event_type = 'event' AND e.event_name IS NOT NULL)
      )
      {filter_sql}
),
anchor_positions AS (
    SELECT
        session_id,
        MIN(seq) AS anchor_seq
    FROM ordered_events
    WHERE node_type = ?4
      AND node_value = ?5
    GROUP BY session_id
),
total_anchor AS (
    SELECT COUNT(*) AS cnt FROM anchor_positions
),
{steps_cte}session_paths AS (
    SELECT
        ap.session_id,
        string_agg(js.node_value, '|' ORDER BY {steps_order}) AS path_str
    FROM anchor_positions ap
    LEFT JOIN journey_steps js ON js.session_id = ap.session_id
    GROUP BY ap.session_id
),
branch_counts AS (
    SELECT
        COALESCE(path_str, '') AS path_str,
        COUNT(*) AS sessions
    FROM session_paths
    GROUP BY COALESCE(path_str, '')
    ORDER BY sessions DESC, path_str ASC
    LIMIT 20
)
SELECT
    path_str,
    sessions,
    COALESCE(CAST(sessions AS DOUBLE) / NULLIF((SELECT cnt FROM total_anchor), 0), 0.0) AS share,
    (SELECT cnt FROM total_anchor) AS total_anchor_sessions
FROM branch_counts
ORDER BY sessions DESC, path_str ASC
"#
    )
}

pub async fn get_journey_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    query: &JourneyQuery,
) -> Result<JourneyResponse> {
    let conn = db.conn.lock().await;

    let tz = resolve_timezone(&conn, website_id, filter.timezone.as_deref())?;
    let (start_str, end_str) = utc_bounds_for_filter(tz, filter.start_date, filter.end_date)?;

    let normalized_anchor = match query.anchor_type {
        AnchorType::Page => normalize_url_rust(&query.anchor_value),
        AnchorType::Event => query.anchor_value.trim().to_string(),
    };
    if normalized_anchor.is_empty() {
        return Err(anyhow!("invalid_anchor_value"));
    }

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
        Box::new(match query.anchor_type {
            AnchorType::Page => "page".to_string(),
            AnchorType::Event => "event".to_string(),
        }),
        Box::new(normalized_anchor.clone()),
    ];

    let mut filter_sql = String::new();
    let mut param_idx = 6usize;
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);

    let max_depth_param_idx = param_idx;
    let clamped_max_depth = query.max_depth.clamp(1, 5);
    params.push(Box::new(i64::from(clamped_max_depth)));

    let sql = build_journey_sql(&query.direction, &filter_sql, max_depth_param_idx);
    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    if let Err(error) = conn.execute_batch("SET statement_timeout = '5000ms'") {
        tracing::warn!(%error, "Could not set DuckDB statement_timeout");
    }

    let rows_res: Result<Vec<(String, i64, f64, i64)>> = (|| {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, f64>(2).unwrap_or(0.0),
                row.get::<_, i64>(3)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })();

    if let Err(error) = conn.execute_batch("RESET statement_timeout") {
        tracing::warn!(%error, "Could not reset DuckDB statement_timeout");
    }

    let rows = rows_res?;
    let total_anchor_sessions = rows.first().map(|r| r.3).unwrap_or(0);

    let branches = rows
        .into_iter()
        .map(|(path_str, sessions, share, _)| JourneyBranch {
            nodes: if path_str.is_empty() {
                Vec::new()
            } else {
                path_str.split('|').map(|node| node.to_string()).collect()
            },
            sessions,
            share,
        })
        .collect();

    Ok(JourneyResponse {
        anchor: JourneyNode {
            node_type: query.anchor_type.clone(),
            value: normalized_anchor,
        },
        direction: query.direction.clone(),
        max_depth: clamped_max_depth,
        total_anchor_sessions,
        branches,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_url_rust;

    #[test]
    fn normalize_url_rust_strips_query_fragment_and_trailing_slash() {
        assert_eq!(normalize_url_rust("/Pricing/?plan=pro#cta"), "/pricing");
        assert_eq!(normalize_url_rust("/"), "/");
    }
}
