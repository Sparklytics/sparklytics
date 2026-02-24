use anyhow::Result;
use chrono::NaiveDate;

use sparklytics_core::analytics::{AnalyticsFilter, StatsResult};

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
    pub filter_language: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
    pub filter_region: Option<String>,
    pub filter_city: Option<String>,
    pub filter_hostname: Option<String>,
}

impl StatsParams {
    pub fn from_filter(website_id: &str, filter: &AnalyticsFilter) -> Self {
        Self {
            website_id: website_id.to_string(),
            start_date: filter.start_date,
            end_date: filter.end_date,
            filter_country: filter.filter_country.clone(),
            filter_page: filter.filter_page.clone(),
            filter_referrer: filter.filter_referrer.clone(),
            filter_browser: filter.filter_browser.clone(),
            filter_os: filter.filter_os.clone(),
            filter_device: filter.filter_device.clone(),
            filter_language: filter.filter_language.clone(),
            filter_utm_source: filter.filter_utm_source.clone(),
            filter_utm_medium: filter.filter_utm_medium.clone(),
            filter_utm_campaign: filter.filter_utm_campaign.clone(),
            filter_region: filter.filter_region.clone(),
            filter_city: filter.filter_city.clone(),
            filter_hostname: filter.filter_hostname.clone(),
        }
    }
}

pub async fn get_stats_inner(db: &DuckDbBackend, params: &StatsParams) -> Result<StatsResult> {
    let conn = db.conn.lock().await;

    let timezone: String = conn
        .prepare("SELECT timezone FROM websites WHERE id = ?1")?
        .query_row(duckdb::params![params.website_id], |row| row.get(0))
        .unwrap_or_else(|_| "UTC".to_string());

    let range_days = (params.end_date - params.start_date).num_days() + 1;
    let prev_end = params.start_date - chrono::Duration::days(1);
    let prev_start = prev_end - chrono::Duration::days(range_days - 1);
    let (current, prev) = query_stats_for_ranges(
        &conn,
        &params.website_id,
        &params.start_date,
        &params.end_date,
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
        prev_pageviews: prev.pageviews,
        prev_visitors: prev.visitors,
        prev_sessions: prev.sessions,
        prev_bounce_rate: prev.bounce_rate,
        prev_avg_duration_seconds: prev.avg_duration,
        timezone,
    })
}

impl DuckDbBackend {
    pub async fn get_stats(&self, params: &StatsParams) -> Result<StatsResult> {
        get_stats_inner(self, params).await
    }
}

struct PeriodStats {
    pageviews: i64,
    visitors: i64,
    sessions: i64,
    bounce_rate: f64,
    avg_duration: f64,
}

fn query_stats_for_ranges(
    conn: &duckdb::Connection,
    website_id: &str,
    current_start_date: &NaiveDate,
    current_end_date: &NaiveDate,
    prev_start_date: &NaiveDate,
    prev_end_date: &NaiveDate,
    params: &StatsParams,
) -> Result<(PeriodStats, PeriodStats)> {
    let current_start_str = current_start_date.format("%Y-%m-%d").to_string();
    let current_end_str = (*current_end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let prev_start_str = prev_start_date.format("%Y-%m-%d").to_string();
    let prev_end_str = (*prev_end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut filter_sql = String::new();
    let mut filter_params: Vec<Box<dyn duckdb::types::ToSql>> = Vec::new();
    filter_params.push(Box::new(website_id.to_string()));
    filter_params.push(Box::new(current_start_str));
    filter_params.push(Box::new(current_end_str));
    filter_params.push(Box::new(prev_start_str));
    filter_params.push(Box::new(prev_end_str));
    let mut param_idx = 6;

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
    if let Some(ref language) = params.filter_language {
        filter_sql.push_str(&format!(" AND e.language = ?{}", param_idx));
        filter_params.push(Box::new(language.clone()));
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
        param_idx += 1;
    }
    if let Some(ref region) = params.filter_region {
        filter_sql.push_str(&format!(" AND e.region = ?{}", param_idx));
        filter_params.push(Box::new(region.clone()));
        param_idx += 1;
    }
    if let Some(ref city) = params.filter_city {
        filter_sql.push_str(&format!(" AND e.city = ?{}", param_idx));
        filter_params.push(Box::new(city.clone()));
        param_idx += 1;
    }
    if let Some(ref hostname) = params.filter_hostname {
        filter_sql.push_str(&format!(
            " AND lower(regexp_extract(e.url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            param_idx
        ));
        filter_params.push(Box::new(hostname.clone()));
    }

    let sql = format!(
        r#"
        WITH periods AS (
            SELECT 'current' AS period_name, CAST(?2 AS TIMESTAMP) AS period_start, CAST(?3 AS TIMESTAMP) AS period_end
            UNION ALL
            SELECT 'previous' AS period_name, CAST(?4 AS TIMESTAMP) AS period_start, CAST(?5 AS TIMESTAMP) AS period_end
        ),
        filtered_events AS (
            SELECT
                p.period_name,
                e.session_id,
                e.visitor_id
            FROM periods p
            JOIN events e
              ON e.website_id = ?1
             AND e.created_at >= p.period_start
             AND e.created_at < p.period_end
            WHERE 1 = 1
              {filter_sql}
        ),
        event_stats AS (
            SELECT
                period_name,
                COUNT(*) AS pageviews,
                COUNT(DISTINCT visitor_id) AS visitors
            FROM filtered_events
            GROUP BY period_name
        ),
        session_ids AS (
            SELECT
                period_name,
                session_id
            FROM filtered_events
            GROUP BY period_name, session_id
        ),
        session_stats AS (
            SELECT
                sid.period_name,
                COUNT(*) AS total_sessions,
                COALESCE(SUM(CASE WHEN s.pageview_count = 1 THEN 1 ELSE 0 END), 0) AS bounced_sessions,
                COALESCE(AVG(EPOCH(s.last_seen - s.first_seen)), 0) AS avg_duration
            FROM session_ids sid
            JOIN sessions s
              ON s.session_id = sid.session_id
            GROUP BY sid.period_name
        ),
        all_stats AS (
            SELECT
                p.period_name,
                COALESCE(es.pageviews, 0) AS pageviews,
                COALESCE(es.visitors, 0) AS visitors,
                COALESCE(ss.total_sessions, 0) AS sessions,
                CASE
                    WHEN COALESCE(ss.total_sessions, 0) = 0 THEN 0.0
                    ELSE ROUND(CAST(COALESCE(ss.bounced_sessions, 0) AS DOUBLE) / ss.total_sessions, 3)
                END AS bounce_rate,
                COALESCE(ss.avg_duration, 0.0) AS avg_duration
            FROM periods p
            LEFT JOIN event_stats es
              ON es.period_name = p.period_name
            LEFT JOIN session_stats ss
              ON ss.period_name = p.period_name
        )
        SELECT
            c.pageviews AS current_pageviews,
            c.visitors AS current_visitors,
            c.sessions AS current_sessions,
            c.bounce_rate AS current_bounce_rate,
            c.avg_duration AS current_avg_duration,
            p.pageviews AS prev_pageviews,
            p.visitors AS prev_visitors,
            p.sessions AS prev_sessions,
            p.bounce_rate AS prev_bounce_rate,
            p.avg_duration AS prev_avg_duration
        FROM all_stats c
        JOIN all_stats p
          ON p.period_name = 'previous'
        WHERE c.period_name = 'current'
        "#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> =
        filter_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let result = stmt.query_row(param_refs.as_slice(), |row| {
        let current = PeriodStats {
            pageviews: row.get::<_, i64>(0)?,
            visitors: row.get::<_, i64>(1)?,
            sessions: row.get::<_, i64>(2)?,
            bounce_rate: row.get::<_, f64>(3)?,
            avg_duration: row.get::<_, f64>(4)?,
        };
        let previous = PeriodStats {
            pageviews: row.get::<_, i64>(5)?,
            visitors: row.get::<_, i64>(6)?,
            sessions: row.get::<_, i64>(7)?,
            bounce_rate: row.get::<_, f64>(8)?,
            avg_duration: row.get::<_, f64>(9)?,
        };
        Ok((current, previous))
    })?;

    Ok(result)
}
