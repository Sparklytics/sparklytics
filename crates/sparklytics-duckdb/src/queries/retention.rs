use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use chrono::{Datelike, LocalResult, NaiveDate, TimeZone};
use chrono_tz::Tz;

use sparklytics_core::analytics::{
    AnalyticsFilter, RetentionCohortRow, RetentionGranularity, RetentionPeriod, RetentionQuery,
    RetentionResponse, RetentionSummary,
};

use crate::DuckDbBackend;

const DEFAULT_RETENTION_STATEMENT_TIMEOUT_MS: u64 = 5_000;
const MIN_RETENTION_STATEMENT_TIMEOUT_MS: u64 = 100;
const MAX_RETENTION_STATEMENT_TIMEOUT_MS: u64 = 120_000;
const RETENTION_QUERY_TIMEOUT_MARKER: &str = "retention_query_timeout";

#[derive(Debug)]
struct RetentionRawRow {
    cohort_start: String,
    cohort_size: i64,
    period_offset: u32,
    retained: i64,
    rate: f64,
}

fn retention_statement_timeout_ms() -> u64 {
    static VALUE: OnceLock<u64> = OnceLock::new();
    *VALUE.get_or_init(
        || match std::env::var("SPARKLYTICS_RETENTION_STATEMENT_TIMEOUT_MS") {
            Ok(raw) => match raw.parse::<u64>() {
                Ok(parsed)
                    if (MIN_RETENTION_STATEMENT_TIMEOUT_MS
                        ..=MAX_RETENTION_STATEMENT_TIMEOUT_MS)
                        .contains(&parsed) =>
                {
                    tracing::info!(
                        timeout_ms = parsed,
                        "Using SPARKLYTICS_RETENTION_STATEMENT_TIMEOUT_MS for retention queries"
                    );
                    parsed
                }
                Ok(parsed) => {
                    tracing::warn!(
                        timeout_ms = parsed,
                        min_timeout_ms = MIN_RETENTION_STATEMENT_TIMEOUT_MS,
                        max_timeout_ms = MAX_RETENTION_STATEMENT_TIMEOUT_MS,
                        "SPARKLYTICS_RETENTION_STATEMENT_TIMEOUT_MS out of range; using default"
                    );
                    DEFAULT_RETENTION_STATEMENT_TIMEOUT_MS
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        value = %raw,
                        "Could not parse SPARKLYTICS_RETENTION_STATEMENT_TIMEOUT_MS; using default"
                    );
                    DEFAULT_RETENTION_STATEMENT_TIMEOUT_MS
                }
            },
            Err(_) => DEFAULT_RETENTION_STATEMENT_TIMEOUT_MS,
        },
    )
}

fn is_statement_timeout_error(error: &anyhow::Error) -> bool {
    let msg = error.to_string().to_ascii_lowercase();
    msg.contains("statement timeout")
        || (msg.contains("timeout") && msg.contains("statement"))
        || msg.contains("retention_query_timeout")
        || msg.contains("interrupted")
}

fn append_event_filters(
    filter: &AnalyticsFilter,
    filter_sql: &mut String,
    params: &mut Vec<Box<dyn duckdb::types::ToSql>>,
    param_idx: &mut usize,
) {
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

fn extend_activity_end_date(
    end_date: NaiveDate,
    granularity: &RetentionGranularity,
    max_periods: u32,
) -> Result<NaiveDate> {
    match granularity {
        RetentionGranularity::Day => end_date
            .checked_add_signed(chrono::Duration::days(i64::from(max_periods)))
            .ok_or_else(|| anyhow!("invalid_activity_end_date")),
        RetentionGranularity::Week => end_date
            .checked_add_signed(chrono::Duration::days(i64::from(max_periods) * 7))
            .ok_or_else(|| anyhow!("invalid_activity_end_date")),
        RetentionGranularity::Month => end_date
            .checked_add_months(chrono::Months::new(max_periods))
            .ok_or_else(|| anyhow!("invalid_activity_end_date")),
    }
}

fn granularity_to_sql(granularity: &RetentionGranularity) -> &'static str {
    match granularity {
        RetentionGranularity::Day => "day",
        RetentionGranularity::Week => "week",
        RetentionGranularity::Month => "month",
    }
}

fn clamp_max_periods(granularity: &RetentionGranularity, max_periods: u32) -> u32 {
    let (min, max) = match granularity {
        RetentionGranularity::Day => (1, 30),
        RetentionGranularity::Week => (1, 12),
        RetentionGranularity::Month => (1, 12),
    };
    max_periods.clamp(min, max)
}

fn parse_cohort_date(cohort_start: &str) -> Option<NaiveDate> {
    let prefix = cohort_start.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

fn max_elapsed_offset(
    cohort_start: NaiveDate,
    end_date: NaiveDate,
    granularity: &RetentionGranularity,
) -> Option<i64> {
    if end_date < cohort_start {
        return None;
    }

    let offset = match granularity {
        RetentionGranularity::Day => (end_date - cohort_start).num_days(),
        RetentionGranularity::Week => (end_date - cohort_start).num_days() / 7,
        RetentionGranularity::Month => {
            let years = i64::from(end_date.year() - cohort_start.year());
            let months = i64::from(end_date.month0()) - i64::from(cohort_start.month0());
            years * 12 + months
        }
    };

    Some(offset)
}

fn compute_summary(
    rows: &[RetentionCohortRow],
    granularity: &RetentionGranularity,
    end_date: NaiveDate,
    max_periods: u32,
) -> RetentionSummary {
    let mut period1_sum = 0.0;
    let mut period1_count = 0usize;
    let mut period4_sum = 0.0;
    let mut period4_count = 0usize;

    for row in rows {
        let Some(cohort_start) = parse_cohort_date(&row.cohort_start) else {
            continue;
        };
        let Some(max_elapsed) = max_elapsed_offset(cohort_start, end_date, granularity) else {
            continue;
        };

        if max_periods > 1 && max_elapsed >= 1 {
            period1_sum += row.periods.get(1).map(|period| period.rate).unwrap_or(0.0);
            period1_count += 1;
        }

        if max_periods > 4 && max_elapsed >= 4 {
            period4_sum += row.periods.get(4).map(|period| period.rate).unwrap_or(0.0);
            period4_count += 1;
        }
    }

    let avg_period1_rate = if period1_count == 0 {
        0.0
    } else {
        period1_sum / period1_count as f64
    };

    let avg_period4_rate = if max_periods <= 4 {
        None
    } else if period4_count == 0 {
        Some(0.0)
    } else {
        Some(period4_sum / period4_count as f64)
    };

    RetentionSummary {
        avg_period1_rate,
        avg_period4_rate,
    }
}

fn build_rows(raw_rows: Vec<RetentionRawRow>, max_periods: u32) -> Vec<RetentionCohortRow> {
    let mut grouped: BTreeMap<String, (i64, HashMap<u32, RetentionPeriod>)> = BTreeMap::new();

    for raw in raw_rows {
        let entry = grouped
            .entry(raw.cohort_start)
            .or_insert_with(|| (raw.cohort_size, HashMap::new()));
        entry.0 = raw.cohort_size;
        entry.1.insert(
            raw.period_offset,
            RetentionPeriod {
                offset: raw.period_offset,
                retained: raw.retained,
                rate: raw.rate,
            },
        );
    }

    grouped
        .into_iter()
        .map(|(cohort_start, (cohort_size, cells))| {
            let periods = (0..max_periods)
                .map(|offset| {
                    cells.get(&offset).cloned().unwrap_or(RetentionPeriod {
                        offset,
                        retained: 0,
                        rate: 0.0,
                    })
                })
                .collect::<Vec<_>>();

            RetentionCohortRow {
                cohort_start,
                cohort_size,
                periods,
            }
        })
        .collect()
}

pub async fn get_retention_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    query: &RetentionQuery,
) -> Result<RetentionResponse> {
    let conn = db.conn.lock().await;
    let timezone = resolve_timezone(&conn, website_id, filter.timezone.as_deref())?;
    let (start_str, end_str) = utc_bounds_for_filter(timezone, filter.start_date, filter.end_date)?;

    let clamped_periods = clamp_max_periods(&query.granularity, query.max_periods);
    let activity_end_date =
        extend_activity_end_date(filter.end_date, &query.granularity, clamped_periods)?;
    let activity_end_str =
        local_midnight_utc(timezone, activity_end_date + chrono::Duration::days(1))?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
    let granularity_sql = granularity_to_sql(&query.granularity).to_string();

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(granularity_sql.clone()),
        Box::new(timezone.name().to_string()),
        Box::new(start_str),
        Box::new(end_str),
        Box::new(activity_end_str),
    ];

    let mut filter_sql = String::new();
    let mut param_idx = 7usize;
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);

    let max_periods_param = param_idx;
    params.push(Box::new(i64::from(clamped_periods)));

    let sql = format!(
        r#"
WITH
cohort_events AS (
    SELECT
        e.visitor_id,
        e.created_at
    FROM events e
    WHERE e.website_id = ?1
      AND e.created_at >= CAST(?4 AS TIMESTAMP)
      AND e.created_at < CAST(?5 AS TIMESTAMP)
      {filter_sql}
),
activity_events AS (
    SELECT
        e.visitor_id,
        e.created_at
    FROM events e
    WHERE e.website_id = ?1
      AND e.created_at >= CAST(?4 AS TIMESTAMP)
      AND e.created_at < CAST(?6 AS TIMESTAMP)
      {filter_sql}
),
eligible_visitors AS (
    SELECT DISTINCT visitor_id
    FROM cohort_events
),
visitor_first_seen AS (
    SELECT
        visitor_id,
        MIN(first_seen_ts) AS global_first_seen
    FROM (
        SELECT
            s.visitor_id,
            MIN(s.first_seen) AS first_seen_ts
        FROM sessions s
        JOIN eligible_visitors ev
          ON ev.visitor_id = s.visitor_id
        WHERE s.website_id = ?1
          AND s.first_seen < CAST(?5 AS TIMESTAMP)
        GROUP BY s.visitor_id
        UNION ALL
        SELECT
            e.visitor_id,
            MIN(e.created_at) AS first_seen_ts
        FROM events e
        JOIN eligible_visitors ev
          ON ev.visitor_id = e.visitor_id
        WHERE e.website_id = ?1
          AND e.created_at < CAST(?5 AS TIMESTAMP)
        GROUP BY e.visitor_id
    ) first_seen_sources
    GROUP BY visitor_id
),
cohorts AS (
    SELECT
        v.visitor_id,
        CAST(
            DATE_TRUNC(?2, (v.global_first_seen AT TIME ZONE 'UTC') AT TIME ZONE ?3) AS DATE
        ) AS cohort_start
    FROM visitor_first_seen v
    JOIN eligible_visitors ev
      ON ev.visitor_id = v.visitor_id
    WHERE v.global_first_seen >= CAST(?4 AS TIMESTAMP)
      AND v.global_first_seen < CAST(?5 AS TIMESTAMP)
),
activity AS (
    SELECT
        c.visitor_id,
        c.cohort_start AS active_period
    FROM cohorts c
    UNION
    SELECT DISTINCT
        ae.visitor_id,
        CAST(
            DATE_TRUNC(?2, (ae.created_at AT TIME ZONE 'UTC') AT TIME ZONE ?3) AS DATE
        ) AS active_period
    FROM activity_events ae
    JOIN cohorts c
      ON c.visitor_id = ae.visitor_id
),
retention_raw AS (
    SELECT
        c.cohort_start,
        DATE_DIFF(?2, c.cohort_start, a.active_period) AS period_offset,
        COUNT(DISTINCT c.visitor_id) AS retained
    FROM cohorts c
    JOIN activity a
      ON a.visitor_id = c.visitor_id
    WHERE DATE_DIFF(?2, c.cohort_start, a.active_period) >= 0
      AND DATE_DIFF(?2, c.cohort_start, a.active_period) < ?{max_periods_param}
    GROUP BY c.cohort_start, period_offset
),
cohort_sizes AS (
    SELECT
        cohort_start,
        COUNT(DISTINCT visitor_id) AS cohort_size
    FROM cohorts
    GROUP BY cohort_start
)
SELECT
    CAST(r.cohort_start AS VARCHAR) AS cohort_start,
    cs.cohort_size,
    CAST(r.period_offset AS BIGINT) AS period_offset,
    r.retained,
    CAST(r.retained AS DOUBLE) / NULLIF(cs.cohort_size, 0) AS rate
FROM retention_raw r
JOIN cohort_sizes cs
  ON cs.cohort_start = r.cohort_start
ORDER BY r.cohort_start ASC, r.period_offset ASC
"#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let statement_timeout_ms = retention_statement_timeout_ms();

    if let Err(error) = conn.execute_batch(&format!(
        "SET statement_timeout = '{}ms'",
        statement_timeout_ms
    )) {
        tracing::warn!(%error, "Could not set DuckDB statement_timeout");
    }

    let raw_rows_result: Result<Vec<RetentionRawRow>> = (|| {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let period_offset = row.get::<_, i64>(2).unwrap_or(0).max(0) as u32;
            Ok(RetentionRawRow {
                cohort_start: row.get(0)?,
                cohort_size: row.get(1)?,
                period_offset,
                retained: row.get(3)?,
                rate: row.get::<_, f64>(4).unwrap_or(0.0),
            })
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

    let raw_rows = match raw_rows_result {
        Ok(rows) => rows,
        Err(error) => {
            if is_statement_timeout_error(&error) {
                return Err(anyhow!(RETENTION_QUERY_TIMEOUT_MARKER));
            }
            return Err(error);
        }
    };

    let rows = build_rows(raw_rows, clamped_periods);
    let summary = compute_summary(&rows, &query.granularity, filter.end_date, clamped_periods);

    Ok(RetentionResponse {
        granularity: query.granularity.clone(),
        max_periods: clamped_periods,
        rows,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::{clamp_max_periods, granularity_to_sql};
    use sparklytics_core::analytics::RetentionGranularity;

    #[test]
    fn granularity_to_sql_maps_correctly() {
        assert_eq!(granularity_to_sql(&RetentionGranularity::Day), "day");
        assert_eq!(granularity_to_sql(&RetentionGranularity::Week), "week");
        assert_eq!(granularity_to_sql(&RetentionGranularity::Month), "month");
    }

    #[test]
    fn clamp_max_periods_enforces_bounds() {
        assert_eq!(clamp_max_periods(&RetentionGranularity::Day, 0), 1);
        assert_eq!(clamp_max_periods(&RetentionGranularity::Day, 99), 30);
        assert_eq!(clamp_max_periods(&RetentionGranularity::Week, 99), 12);
        assert_eq!(clamp_max_periods(&RetentionGranularity::Month, 99), 12);
    }
}
