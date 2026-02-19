use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

use crate::DuckDbBackend;

#[derive(Debug, Clone, Serialize)]
pub struct TimeseriesPoint {
    pub date: String,
    pub pageviews: i64,
    pub visitors: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeseriesResult {
    pub series: Vec<TimeseriesPoint>,
    pub granularity: String,
}

/// Auto-granularity: ≤2 days → hour, 3–60 → day, >60 → month.
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

impl DuckDbBackend {
    pub async fn get_timeseries(
        &self,
        website_id: &str,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
        granularity: Option<&str>,
        filter_country: Option<&str>,
        filter_page: Option<&str>,
    ) -> Result<TimeseriesResult> {
        let gran = match granularity {
            Some("hour") => "hour".to_string(),
            Some("day") => "day".to_string(),
            Some("month") => "month".to_string(),
            _ => auto_granularity(start_date, end_date),
        };

        let conn = self.conn.lock().await;

        let start_str = start_date.format("%Y-%m-%d").to_string();
        let end_next = *end_date + chrono::Duration::days(1);
        let end_str = end_next.format("%Y-%m-%d").to_string();

        let mut filter_sql = String::new();
        let mut filter_params: Vec<Box<dyn duckdb::types::ToSql>> = Vec::new();
        filter_params.push(Box::new(website_id.to_string()));
        filter_params.push(Box::new(start_str.clone()));
        filter_params.push(Box::new(end_str.clone()));
        let mut param_idx = 4;

        if let Some(country) = filter_country {
            filter_sql.push_str(&format!(" AND country = ?{}", param_idx));
            filter_params.push(Box::new(country.to_string()));
            param_idx += 1;
        }
        if let Some(page) = filter_page {
            filter_sql.push_str(&format!(" AND url LIKE ?{}", param_idx));
            filter_params.push(Box::new(format!("%{}%", page)));
            let _ = param_idx;
        }

        let trunc_fn = match gran.as_str() {
            "hour" => "CAST(date_trunc('hour', created_at) AS VARCHAR)",
            "month" => "CAST(date_trunc('month', created_at) AS VARCHAR)",
            _ => "CAST(date_trunc('day', created_at) AS VARCHAR)",
        };

        let sql = format!(
            r#"
            SELECT
                {trunc_fn} AS bucket,
                COUNT(*) AS pageviews,
                COUNT(DISTINCT visitor_id) AS visitors
            FROM events
            WHERE website_id = ?1
              AND created_at >= ?2
              AND created_at < ?3
              {filter_sql}
            GROUP BY bucket
            ORDER BY bucket
            "#
        );

        let param_refs: Vec<&dyn duckdb::types::ToSql> =
            filter_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let bucket: String = row.get(0)?;
            let pageviews: i64 = row.get(1)?;
            let visitors: i64 = row.get(2)?;
            Ok((bucket, pageviews, visitors))
        })?;

        let mut data_map: std::collections::HashMap<String, (i64, i64)> =
            std::collections::HashMap::new();
        for row in rows {
            let (bucket, pv, vis) = row?;
            data_map.insert(bucket, (pv, vis));
        }

        // Zero-fill: generate all expected buckets and fill missing ones with 0.
        let all_buckets = generate_buckets(start_date, end_date, &gran);

        let series: Vec<TimeseriesPoint> = all_buckets
            .into_iter()
            .map(|bucket_key| {
                // Try to find the bucket in data_map by matching the formatted key.
                let (pageviews, visitors) = find_bucket_match(&data_map, &bucket_key);
                TimeseriesPoint {
                    date: bucket_key,
                    pageviews,
                    visitors,
                }
            })
            .collect();

        Ok(TimeseriesResult {
            series,
            granularity: gran,
        })
    }
}

/// Find a bucket match in the data map. DuckDB returns timestamps in various formats,
/// so we do a prefix match.
fn find_bucket_match(
    data_map: &std::collections::HashMap<String, (i64, i64)>,
    bucket_key: &str,
) -> (i64, i64) {
    // Try exact match first.
    if let Some(&(pv, vis)) = data_map.get(bucket_key) {
        return (pv, vis);
    }
    // Try matching by prefix (DuckDB may return "2026-02-10 00:00:00" for date "2026-02-10").
    for (key, &(pv, vis)) in data_map {
        if key.starts_with(bucket_key) || bucket_key.starts_with(key) {
            return (pv, vis);
        }
    }
    (0, 0)
}

/// Generate all time buckets for the given range and granularity.
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
            // day
            let mut current = *start;
            while current <= *end {
                buckets.push(current.format("%Y-%m-%d").to_string());
                current += chrono::Duration::days(1);
            }
        }
    }
    buckets
}

use chrono::{Datelike, Timelike};
