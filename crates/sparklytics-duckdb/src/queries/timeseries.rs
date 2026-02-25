use std::collections::HashMap;

use anyhow::Result;
use chrono::{Datelike, NaiveDate, Timelike};

use sparklytics_core::analytics::{
    AnalyticsFilter, ComparisonRange, TimeseriesPoint, TimeseriesResult,
};

use crate::DuckDbBackend;

/// Auto-granularity: ≤2 days -> hour, 3-60 -> day, >60 -> month.
pub fn auto_granularity(start: &NaiveDate, end: &NaiveDate) -> String {
    let days = (*end - *start).num_days() + 1;
    if days <= 2 {
        "hour".to_string()
    } else if days <= 60 {
        "day".to_string()
    } else {
        "month".to_string()
    }
}

pub async fn get_timeseries_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    granularity: Option<&str>,
    comparison: Option<&ComparisonRange>,
) -> Result<TimeseriesResult> {
    let gran = match granularity {
        Some("hour") => "hour".to_string(),
        Some("day") => "day".to_string(),
        Some("month") => "month".to_string(),
        _ => auto_granularity(&filter.start_date, &filter.end_date),
    };

    let conn = db.conn.lock().await;

    if let Some(comparison_range) = comparison {
        let rows = query_period_buckets(&conn, website_id, filter, &gran, Some(comparison_range))?;

        let primary_buckets = generate_buckets(&filter.start_date, &filter.end_date, &gran);
        let bucket_count = primary_buckets.len();
        let mut primary_values = vec![(0_i64, 0_i64); bucket_count];
        let mut compare_values = vec![(0_i64, 0_i64); bucket_count];

        for (period, bucket_index, pageviews, visitors) in rows {
            if bucket_index < 0 {
                continue;
            }
            let idx = bucket_index as usize;
            if idx >= bucket_count {
                continue;
            }

            if period == "primary" {
                primary_values[idx] = (pageviews, visitors);
            } else {
                compare_values[idx] = (pageviews, visitors);
            }
        }

        let series = primary_buckets
            .iter()
            .enumerate()
            .map(|(idx, bucket)| {
                let (pageviews, visitors) = primary_values[idx];
                TimeseriesPoint {
                    date: bucket.clone(),
                    pageviews,
                    visitors,
                }
            })
            .collect::<Vec<_>>();

        let compare_series = primary_buckets
            .iter()
            .enumerate()
            .map(|(idx, bucket)| {
                let (pageviews, visitors) = compare_values[idx];
                TimeseriesPoint {
                    date: bucket.clone(),
                    pageviews,
                    visitors,
                }
            })
            .collect::<Vec<_>>();

        return Ok(TimeseriesResult {
            series,
            granularity: gran,
            compare_series: Some(compare_series),
            compare: Some(comparison_range.to_metadata()),
        });
    }

    let rows = query_period_buckets(&conn, website_id, filter, &gran, None)?;
    let mut data_map: HashMap<i64, (i64, i64)> = HashMap::new();
    for (_, idx, pageviews, visitors) in rows {
        data_map.insert(idx, (pageviews, visitors));
    }

    let all_buckets = generate_buckets(&filter.start_date, &filter.end_date, &gran);
    let series = all_buckets
        .into_iter()
        .enumerate()
        .map(|(idx, bucket)| {
            let (pageviews, visitors) = data_map.get(&(idx as i64)).copied().unwrap_or((0, 0));
            TimeseriesPoint {
                date: bucket,
                pageviews,
                visitors,
            }
        })
        .collect::<Vec<_>>();

    Ok(TimeseriesResult {
        series,
        granularity: gran,
        compare_series: None,
        compare: None,
    })
}

fn query_period_buckets(
    conn: &duckdb::Connection,
    website_id: &str,
    filter: &AnalyticsFilter,
    granularity: &str,
    comparison: Option<&ComparisonRange>,
) -> Result<Vec<(String, i64, i64, i64)>> {
    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut filter_sql = String::new();
    let mut filter_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];

    let mut param_idx = 4;
    if let Some(compare) = comparison {
        filter_params.push(Box::new(
            compare.comparison_start.format("%Y-%m-%d").to_string(),
        ));
        filter_params.push(Box::new(
            (compare.comparison_end + chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string(),
        ));
        param_idx = 6;
    }

    if let Some(ref country) = filter.filter_country {
        filter_sql.push_str(&format!(" AND e.country = ?{}", param_idx));
        filter_params.push(Box::new(country.clone()));
        param_idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        filter_sql.push_str(&format!(" AND e.url LIKE ?{}", param_idx));
        filter_params.push(Box::new(format!("%{}%", page)));
        param_idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        filter_sql.push_str(&format!(" AND e.referrer_domain = ?{}", param_idx));
        filter_params.push(Box::new(referrer.clone()));
        param_idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        filter_sql.push_str(&format!(" AND e.browser = ?{}", param_idx));
        filter_params.push(Box::new(browser.clone()));
        param_idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        filter_sql.push_str(&format!(" AND e.os = ?{}", param_idx));
        filter_params.push(Box::new(os.clone()));
        param_idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        filter_sql.push_str(&format!(" AND e.device_type = ?{}", param_idx));
        filter_params.push(Box::new(device.clone()));
        param_idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        filter_sql.push_str(&format!(" AND e.language = ?{}", param_idx));
        filter_params.push(Box::new(language.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        filter_sql.push_str(&format!(" AND e.utm_source = ?{}", param_idx));
        filter_params.push(Box::new(utm_source.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        filter_sql.push_str(&format!(" AND e.utm_medium = ?{}", param_idx));
        filter_params.push(Box::new(utm_medium.clone()));
        param_idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        filter_sql.push_str(&format!(" AND e.utm_campaign = ?{}", param_idx));
        filter_params.push(Box::new(utm_campaign.clone()));
        param_idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        filter_sql.push_str(&format!(" AND e.region = ?{}", param_idx));
        filter_params.push(Box::new(region.clone()));
        param_idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        filter_sql.push_str(&format!(" AND e.city = ?{}", param_idx));
        filter_params.push(Box::new(city.clone()));
        param_idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        filter_sql.push_str(&format!(
            " AND lower(regexp_extract(e.url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            param_idx
        ));
        filter_params.push(Box::new(hostname.clone()));
    }

    let bucket_idx_expr = match granularity {
        "hour" => "CAST(DATEDIFF('hour', p.period_start, e.created_at) AS BIGINT)",
        "month" => {
            "CAST(((EXTRACT(year FROM e.created_at) - EXTRACT(year FROM p.period_start)) * 12 + (EXTRACT(month FROM e.created_at) - EXTRACT(month FROM p.period_start))) AS BIGINT)"
        }
        _ => "CAST(DATEDIFF('day', p.period_start, e.created_at) AS BIGINT)",
    };

    let sql = if comparison.is_some() {
        format!(
            r#"
            WITH periods AS (
                SELECT 'primary' AS period_name, CAST(?2 AS TIMESTAMP) AS period_start, CAST(?3 AS TIMESTAMP) AS period_end
                UNION ALL
                SELECT 'comparison' AS period_name, CAST(?4 AS TIMESTAMP) AS period_start, CAST(?5 AS TIMESTAMP) AS period_end
            ),
            filtered AS (
                SELECT
                    p.period_name,
                    {bucket_idx_expr} AS bucket_index,
                    e.visitor_id
                FROM periods p
                JOIN events e
                  ON e.website_id = ?1
                 AND e.created_at >= p.period_start
                 AND e.created_at < p.period_end
                WHERE 1 = 1
                  {filter_sql}
            )
            SELECT
                period_name,
                bucket_index,
                COUNT(*) AS pageviews,
                COUNT(DISTINCT visitor_id) AS visitors
            FROM filtered
            GROUP BY period_name, bucket_index
            ORDER BY period_name, bucket_index
            "#
        )
    } else {
        format!(
            r#"
            WITH periods AS (
                SELECT 'primary' AS period_name, CAST(?2 AS TIMESTAMP) AS period_start, CAST(?3 AS TIMESTAMP) AS period_end
            ),
            filtered AS (
                SELECT
                    p.period_name,
                    {bucket_idx_expr} AS bucket_index,
                    e.visitor_id
                FROM periods p
                JOIN events e
                  ON e.website_id = ?1
                 AND e.created_at >= p.period_start
                 AND e.created_at < p.period_end
                WHERE 1 = 1
                  {filter_sql}
            )
            SELECT
                period_name,
                bucket_index,
                COUNT(*) AS pageviews,
                COUNT(DISTINCT visitor_id) AS visitors
            FROM filtered
            GROUP BY period_name, bucket_index
            ORDER BY period_name, bucket_index
            "#
        )
    };

    let param_refs: Vec<&dyn duckdb::types::ToSql> =
        filter_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let period_name: String = row.get(0)?;
        let bucket_index: i64 = row.get(1)?;
        let pageviews: i64 = row.get(2)?;
        let visitors: i64 = row.get(3)?;
        Ok((period_name, bucket_index, pageviews, visitors))
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

impl DuckDbBackend {
    pub async fn get_timeseries(
        &self,
        website_id: &str,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
        comparison: Option<&ComparisonRange>,
    ) -> Result<TimeseriesResult> {
        get_timeseries_inner(self, website_id, filter, granularity, comparison).await
    }
}

fn generate_buckets(start: &NaiveDate, end: &NaiveDate, gran: &str) -> Vec<String> {
    let mut buckets = Vec::new();
    match gran {
        "hour" => {
            let mut current = start.and_hms_opt(0, 0, 0).unwrap_or_default();
            let end_dt = (*end + chrono::Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default();
            while current < end_dt {
                buckets.push(format!(
                    "{}T{:02}:00:00Z",
                    current.format("%Y-%m-%d"),
                    current.hour()
                ));
                current += chrono::Duration::hours(1);
            }
        }
        "month" => {
            let mut year = start.year();
            let mut month = start.month();
            let end_year = end.year();
            let end_month = end.month();
            loop {
                buckets.push(format!("{:04}-{:02}", year, month));
                if year > end_year || (year == end_year && month >= end_month) {
                    break;
                }
                month += 1;
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
        }
        _ => {
            let mut current = *start;
            while current <= *end {
                buckets.push(current.format("%Y-%m-%d").to_string());
                current += chrono::Duration::days(1);
            }
        }
    }
    buckets
}
