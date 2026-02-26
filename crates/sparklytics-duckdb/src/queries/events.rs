use std::collections::HashMap;

use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Timelike};

use sparklytics_core::analytics::{
    AnalyticsFilter, EventNameRow, EventNamesResult, EventPropertiesResult, EventPropertyRow,
    TimeseriesPoint, TimeseriesResult,
};

use crate::queries::bot_filters::append_event_bot_filter;
use crate::DuckDbBackend;

use super::timeseries::auto_granularity;

const EVENT_NAMES_LIMIT: i64 = 200;
const EVENT_PROPERTIES_LIMIT: i64 = 500;
const EVENT_PROPERTIES_SAMPLE_LIMIT: i64 = 10_000;

fn append_dimension_filters(
    filter: &AnalyticsFilter,
    column_prefix: &str,
    filter_sql: &mut String,
    params: &mut Vec<Box<dyn duckdb::types::ToSql>>,
    param_idx: &mut usize,
) {
    append_event_bot_filter(filter_sql, filter.include_bots, column_prefix);
    if let Some(ref country) = filter.filter_country {
        filter_sql.push_str(&format!(" AND {column_prefix}country = ?{}", *param_idx));
        params.push(Box::new(country.clone()));
        *param_idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        filter_sql.push_str(&format!(" AND {column_prefix}url LIKE ?{}", *param_idx));
        params.push(Box::new(format!("%{}%", page)));
        *param_idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        filter_sql.push_str(&format!(
            " AND {column_prefix}referrer_domain = ?{}",
            *param_idx
        ));
        params.push(Box::new(referrer.clone()));
        *param_idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        filter_sql.push_str(&format!(" AND {column_prefix}browser = ?{}", *param_idx));
        params.push(Box::new(browser.clone()));
        *param_idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        filter_sql.push_str(&format!(" AND {column_prefix}os = ?{}", *param_idx));
        params.push(Box::new(os.clone()));
        *param_idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        filter_sql.push_str(&format!(
            " AND {column_prefix}device_type = ?{}",
            *param_idx
        ));
        params.push(Box::new(device.clone()));
        *param_idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        filter_sql.push_str(&format!(" AND {column_prefix}language = ?{}", *param_idx));
        params.push(Box::new(language.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        filter_sql.push_str(&format!(" AND {column_prefix}utm_source = ?{}", *param_idx));
        params.push(Box::new(utm_source.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        filter_sql.push_str(&format!(" AND {column_prefix}utm_medium = ?{}", *param_idx));
        params.push(Box::new(utm_medium.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        filter_sql.push_str(&format!(
            " AND {column_prefix}utm_campaign = ?{}",
            *param_idx
        ));
        params.push(Box::new(utm_campaign.clone()));
        *param_idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        filter_sql.push_str(&format!(" AND {column_prefix}region = ?{}", *param_idx));
        params.push(Box::new(region.clone()));
        *param_idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        filter_sql.push_str(&format!(" AND {column_prefix}city = ?{}", *param_idx));
        params.push(Box::new(city.clone()));
        *param_idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        filter_sql.push_str(&format!(
            " AND lower(regexp_extract({column_prefix}url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            *param_idx
        ));
        params.push(Box::new(hostname.clone()));
        *param_idx += 1;
    }
}

pub async fn get_event_names_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
) -> Result<EventNamesResult> {
    let conn = db.conn.lock().await;

    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let period_days = (filter.end_date - filter.start_date).num_days() + 1;
    let prev_start_str = (filter.start_date - Duration::days(period_days))
        .format("%Y-%m-%d")
        .to_string();
    let prev_end_str = filter.start_date.format("%Y-%m-%d").to_string();

    let mut total_filter_sql = String::new();
    let mut total_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str.clone()),
        Box::new(end_str.clone()),
    ];
    let mut total_param_idx = 4;
    append_dimension_filters(
        filter,
        "e.",
        &mut total_filter_sql,
        &mut total_params,
        &mut total_param_idx,
    );

    let total_sql = format!(
        "SELECT COUNT(DISTINCT e.event_name)
         FROM events e
         WHERE e.website_id = ?1
           AND e.event_type = 'event'
           AND e.event_name IS NOT NULL
           AND e.created_at >= ?2
           AND e.created_at < ?3
           {total_filter_sql}"
    );
    let total_refs: Vec<&dyn duckdb::types::ToSql> =
        total_params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .prepare(&total_sql)?
        .query_row(total_refs.as_slice(), |row| row.get(0))?;

    let mut current_filter_sql = String::new();
    let mut previous_filter_sql = String::new();
    let mut rows_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
        Box::new(website_id.to_string()),
        Box::new(prev_start_str),
        Box::new(prev_end_str),
    ];
    let mut rows_param_idx = 7;
    append_dimension_filters(
        filter,
        "e.",
        &mut current_filter_sql,
        &mut rows_params,
        &mut rows_param_idx,
    );
    append_dimension_filters(
        filter,
        "e.",
        &mut previous_filter_sql,
        &mut rows_params,
        &mut rows_param_idx,
    );

    let rows_sql = format!(
        r#"
        WITH current_period AS (
            SELECT
                e.event_name,
                COUNT(*) AS count,
                COUNT(DISTINCT e.visitor_id) AS visitors
            FROM events e
            WHERE e.website_id = ?1
              AND e.event_type = 'event'
              AND e.event_name IS NOT NULL
              AND e.created_at >= ?2
              AND e.created_at < ?3
              {current_filter_sql}
            GROUP BY e.event_name
        ),
        previous_period AS (
            SELECT
                e.event_name,
                COUNT(*) AS prev_count
            FROM events e
            WHERE e.website_id = ?4
              AND e.event_type = 'event'
              AND e.event_name IS NOT NULL
              AND e.created_at >= ?5
              AND e.created_at < ?6
              {previous_filter_sql}
            GROUP BY e.event_name
        )
        SELECT
            c.event_name,
            c.count,
            c.visitors,
            p.prev_count
        FROM current_period c
        LEFT JOIN previous_period p ON p.event_name = c.event_name
        ORDER BY c.count DESC, c.event_name ASC
        LIMIT {EVENT_NAMES_LIMIT}
        "#
    );

    let rows_refs: Vec<&dyn duckdb::types::ToSql> =
        rows_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&rows_sql)?;
    let mapped = stmt.query_map(rows_refs.as_slice(), |row| {
        Ok(EventNameRow {
            event_name: row.get(0)?,
            count: row.get(1)?,
            visitors: row.get(2)?,
            prev_count: row.get(3)?,
        })
    })?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row?);
    }

    Ok(EventNamesResult { rows, total })
}

pub async fn get_event_properties_inner(
    db: &DuckDbBackend,
    website_id: &str,
    event_name: &str,
    filter: &AnalyticsFilter,
) -> Result<EventPropertiesResult> {
    let conn = db.conn.lock().await;

    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut total_filter_sql = String::new();
    let mut total_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(event_name.to_string()),
        Box::new(start_str.clone()),
        Box::new(end_str.clone()),
    ];
    let mut total_param_idx = 5;
    append_dimension_filters(
        filter,
        "e.",
        &mut total_filter_sql,
        &mut total_params,
        &mut total_param_idx,
    );

    let total_sql = format!(
        "SELECT COUNT(*)
         FROM events e
         WHERE e.website_id = ?1
           AND e.event_type = 'event'
           AND e.event_name = ?2
           AND e.created_at >= ?3
           AND e.created_at < ?4
           {total_filter_sql}"
    );
    let total_refs: Vec<&dyn duckdb::types::ToSql> =
        total_params.iter().map(|p| p.as_ref()).collect();
    let total_occurrences: i64 = conn
        .prepare(&total_sql)?
        .query_row(total_refs.as_slice(), |row| row.get(0))?;
    let sample_size = total_occurrences.min(EVENT_PROPERTIES_SAMPLE_LIMIT);

    let mut properties_filter_sql = String::new();
    let mut properties_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(event_name.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut properties_param_idx = 5;
    append_dimension_filters(
        filter,
        "e.",
        &mut properties_filter_sql,
        &mut properties_params,
        &mut properties_param_idx,
    );

    let properties_sql = format!(
        r#"
        WITH sampled AS (
            SELECT e.event_data
            FROM events e
            WHERE e.website_id = ?1
              AND e.event_type = 'event'
              AND e.event_name = ?2
              AND e.created_at >= ?3
              AND e.created_at < ?4
              {properties_filter_sql}
            ORDER BY e.created_at DESC
            LIMIT {EVENT_PROPERTIES_SAMPLE_LIMIT}
        ),
        valid_sampled AS (
            SELECT s.event_data
            FROM sampled s
            WHERE s.event_data IS NOT NULL
              AND json_valid(s.event_data)
        ),
        expanded AS (
            SELECT
                t.key AS property_key,
                json_extract_string(v.event_data, '$.' || t.key) AS property_value
            FROM valid_sampled v,
                 UNNEST(json_keys(v.event_data)) AS t(key)
        )
        SELECT
            property_key,
            property_value,
            COUNT(*) AS count
        FROM expanded
        WHERE property_value IS NOT NULL
        GROUP BY property_key, property_value
        ORDER BY property_key ASC, count DESC
        LIMIT {EVENT_PROPERTIES_LIMIT}
        "#
    );
    let properties_refs: Vec<&dyn duckdb::types::ToSql> =
        properties_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&properties_sql)?;
    let mapped = stmt.query_map(properties_refs.as_slice(), |row| {
        Ok(EventPropertyRow {
            property_key: row.get(0)?,
            property_value: row.get(1)?,
            count: row.get(2)?,
        })
    })?;

    let mut properties = Vec::new();
    for row in mapped {
        properties.push(row?);
    }

    Ok(EventPropertiesResult {
        event_name: event_name.to_string(),
        total_occurrences,
        sample_size,
        properties,
    })
}

pub async fn get_event_timeseries_inner(
    db: &DuckDbBackend,
    website_id: &str,
    event_name: &str,
    filter: &AnalyticsFilter,
    granularity: Option<&str>,
) -> Result<TimeseriesResult> {
    let gran = match granularity {
        Some("hour") => "hour".to_string(),
        Some("day") => "day".to_string(),
        Some("month") => "month".to_string(),
        _ => auto_granularity(&filter.start_date, &filter.end_date),
    };

    let conn = db.conn.lock().await;

    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut filter_sql = String::new();
    let mut filter_params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(event_name.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut param_idx = 5;
    append_dimension_filters(
        filter,
        "",
        &mut filter_sql,
        &mut filter_params,
        &mut param_idx,
    );

    // Use explicit formatting to keep SQL bucket keys aligned with Rust bucket generation.
    let trunc_fn = match gran.as_str() {
        "hour" => "strftime(created_at, '%Y-%m-%d %H:00:00')",
        "month" => "strftime(created_at, '%Y-%m')",
        _ => "strftime(created_at, '%Y-%m-%d')",
    };

    let sql = format!(
        r#"
        SELECT
            {trunc_fn} AS bucket,
            COUNT(*) AS occurrences,
            COUNT(DISTINCT visitor_id) AS visitors
        FROM events
        WHERE website_id = ?1
          AND event_name = ?2
          AND event_type = 'event'
          AND created_at >= ?3
          AND created_at < ?4
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
        let occurrences: i64 = row.get(1)?;
        let visitors: i64 = row.get(2)?;
        Ok((bucket, occurrences, visitors))
    })?;

    let mut data_map: HashMap<String, (i64, i64)> = HashMap::new();
    for row in rows {
        let (bucket, occurrences, visitors) = row?;
        data_map.insert(bucket, (occurrences, visitors));
    }

    let all_buckets = generate_buckets(&filter.start_date, &filter.end_date, &gran);
    let series = all_buckets
        .into_iter()
        .map(|bucket_key| {
            let (occurrences, visitors) = find_bucket_match(&data_map, &bucket_key);
            TimeseriesPoint {
                date: bucket_key,
                pageviews: occurrences,
                visitors,
            }
        })
        .collect();

    Ok(TimeseriesResult {
        series,
        granularity: gran,
        compare_series: None,
        compare: None,
    })
}

fn find_bucket_match(data_map: &HashMap<String, (i64, i64)>, bucket_key: &str) -> (i64, i64) {
    data_map.get(bucket_key).copied().unwrap_or((0, 0))
}

fn generate_buckets(start: &NaiveDate, end: &NaiveDate, gran: &str) -> Vec<String> {
    let mut buckets = Vec::new();
    match gran {
        "hour" => {
            let mut current = start.and_hms_opt(0, 0, 0).unwrap_or_default();
            let end_dt = (*end + Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap_or_default();
            while current < end_dt {
                buckets.push(format!(
                    "{} {:02}:00:00",
                    current.format("%Y-%m-%d"),
                    current.hour()
                ));
                current += Duration::hours(1);
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
                current += Duration::days(1);
            }
        }
    }
    buckets
}
