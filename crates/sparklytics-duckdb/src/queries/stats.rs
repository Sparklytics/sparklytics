use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::DuckDbBackend;

#[derive(Debug, Clone)]
pub struct StatsParams {
    pub website_id: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub filter_country: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsResult {
    pub pageviews: i64,
    pub visitors: i64,
    pub sessions: i64,
    pub bounce_rate: f64,
    pub avg_duration_seconds: f64,
    pub timezone: String,
    pub prev_pageviews: i64,
    pub prev_visitors: i64,
    pub prev_sessions: i64,
    pub prev_bounce_rate: f64,
    pub prev_avg_duration_seconds: f64,
}

impl DuckDbBackend {
    pub async fn get_stats(&self, params: &StatsParams) -> Result<StatsResult> {
        let conn = self.conn.lock().await;

        // Get website timezone.
        let timezone: String = conn
            .prepare("SELECT timezone FROM websites WHERE id = ?1")?
            .query_row(duckdb::params![params.website_id], |row| row.get(0))
            .unwrap_or_else(|_| "UTC".to_string());

        // Calculate current period.
        let current = query_stats_for_period(
            &conn,
            &params.website_id,
            &params.start_date,
            &params.end_date,
            params,
        )?;

        // Calculate previous period (same duration, immediately before).
        let range_days = (params.end_date - params.start_date).num_days() + 1;
        let prev_end = params.start_date - chrono::Duration::days(1);
        let prev_start = prev_end - chrono::Duration::days(range_days - 1);

        let prev = query_stats_for_period(
            &conn,
            &params.website_id,
            &prev_start,
            &prev_end,
            params,
        )?;

        Ok(StatsResult {
            pageviews: current.pageviews,
            visitors: current.visitors,
            sessions: current.sessions,
            bounce_rate: current.bounce_rate,
            avg_duration_seconds: current.avg_duration,
            timezone,
            prev_pageviews: prev.pageviews,
            prev_visitors: prev.visitors,
            prev_sessions: prev.sessions,
            prev_bounce_rate: prev.bounce_rate,
            prev_avg_duration_seconds: prev.avg_duration,
        })
    }
}

struct PeriodStats {
    pageviews: i64,
    visitors: i64,
    sessions: i64,
    bounce_rate: f64,
    avg_duration: f64,
}

fn query_stats_for_period(
    conn: &duckdb::Connection,
    website_id: &str,
    start_date: &NaiveDate,
    end_date: &NaiveDate,
    params: &StatsParams,
) -> Result<PeriodStats> {
    let start_str = start_date.format("%Y-%m-%d").to_string();
    // end_date is inclusive, so we add 1 day for the < comparison.
    let end_next = *end_date + chrono::Duration::days(1);
    let end_str = end_next.format("%Y-%m-%d").to_string();

    // Build dynamic filter clauses. All use parameterized queries.
    let mut filter_sql = String::new();
    let mut filter_params: Vec<Box<dyn duckdb::types::ToSql>> = Vec::new();
    // Base params: ?1 = website_id, ?2 = start, ?3 = end
    filter_params.push(Box::new(website_id.to_string()));
    filter_params.push(Box::new(start_str.clone()));
    filter_params.push(Box::new(end_str.clone()));
    let mut param_idx = 4;

    if let Some(ref country) = params.filter_country {
        filter_sql.push_str(&format!(" AND e.country = ?{}", param_idx));
        filter_params.push(Box::new(country.clone()));
        param_idx += 1;
    }
    if let Some(ref page) = params.filter_page {
        filter_sql.push_str(&format!(" AND e.url LIKE ?{}", param_idx));
        filter_params.push(Box::new(format!("%{}%", page)));
        param_idx += 1;
    }
    if let Some(ref referrer) = params.filter_referrer {
        filter_sql.push_str(&format!(" AND e.referrer_domain = ?{}", param_idx));
        filter_params.push(Box::new(referrer.clone()));
        param_idx += 1;
    }
    if let Some(ref browser) = params.filter_browser {
        filter_sql.push_str(&format!(" AND e.browser = ?{}", param_idx));
        filter_params.push(Box::new(browser.clone()));
        param_idx += 1;
    }
    if let Some(ref os) = params.filter_os {
        filter_sql.push_str(&format!(" AND e.os = ?{}", param_idx));
        filter_params.push(Box::new(os.clone()));
        param_idx += 1;
    }
    if let Some(ref device) = params.filter_device {
        filter_sql.push_str(&format!(" AND e.device_type = ?{}", param_idx));
        filter_params.push(Box::new(device.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_source) = params.filter_utm_source {
        filter_sql.push_str(&format!(" AND e.utm_source = ?{}", param_idx));
        filter_params.push(Box::new(utm_source.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_medium) = params.filter_utm_medium {
        filter_sql.push_str(&format!(" AND e.utm_medium = ?{}", param_idx));
        filter_params.push(Box::new(utm_medium.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_campaign) = params.filter_utm_campaign {
        filter_sql.push_str(&format!(" AND e.utm_campaign = ?{}", param_idx));
        filter_params.push(Box::new(utm_campaign.clone()));
        let _ = param_idx;
    }

    // CLAUDE.md critical fact #4: Bounce rate MUST use CTEs (no correlated subqueries in DuckDB).
    let sql = format!(
        r#"
        WITH filtered_events AS (
            SELECT e.session_id, e.visitor_id
            FROM events e
            WHERE e.website_id = ?1
              AND e.created_at >= ?2
              AND e.created_at < ?3
              {filter_sql}
        ),
        session_counts AS (
            SELECT
                s.session_id,
                s.pageview_count,
                s.first_seen,
                s.last_seen
            FROM sessions s
            WHERE s.session_id IN (SELECT DISTINCT session_id FROM filtered_events)
        ),
        stats AS (
            SELECT
                COUNT(*) AS total_sessions,
                COALESCE(SUM(CASE WHEN pageview_count = 1 THEN 1 ELSE 0 END), 0) AS bounced_sessions,
                COALESCE(AVG(
                    EPOCH(last_seen - first_seen)
                ), 0) AS avg_duration
            FROM session_counts
        )
        SELECT
            (SELECT COUNT(*) FROM filtered_events) AS pageviews,
            (SELECT COUNT(DISTINCT visitor_id) FROM filtered_events) AS visitors,
            (SELECT total_sessions FROM stats) AS sessions,
            CASE
                WHEN (SELECT total_sessions FROM stats) = 0 THEN 0.0
                ELSE ROUND(CAST((SELECT bounced_sessions FROM stats) AS DOUBLE) / (SELECT total_sessions FROM stats), 3)
            END AS bounce_rate,
            (SELECT avg_duration FROM stats) AS avg_duration
        "#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> = filter_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let result = stmt.query_row(param_refs.as_slice(), |row| {
        Ok(PeriodStats {
            pageviews: row.get::<_, i64>(0)?,
            visitors: row.get::<_, i64>(1)?,
            sessions: row.get::<_, i64>(2)?,
            bounce_rate: row.get::<_, f64>(3)?,
            avg_duration: row.get::<_, f64>(4)?,
        })
    })?;

    Ok(result)
}
