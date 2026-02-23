use anyhow::{anyhow, Result};

use sparklytics_core::analytics::{AnalyticsFilter, MetricRow, MetricsPage, VALID_METRIC_TYPES};

use crate::DuckDbBackend;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsResult {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub rows: Vec<MetricRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsPagination {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

pub fn is_valid_metric_type(t: &str) -> bool {
    VALID_METRIC_TYPES.contains(&t)
}

pub async fn get_metrics_inner(
    db: &DuckDbBackend,
    website_id: &str,
    metric_type: &str,
    limit: i64,
    offset: i64,
    filter: &AnalyticsFilter,
) -> Result<(MetricsResult, MetricsPagination)> {
    if !is_valid_metric_type(metric_type) {
        return Err(anyhow!("invalid metric type: {}", metric_type));
    }

    let conn = db.conn.lock().await;
    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_next = filter.end_date + chrono::Duration::days(1);
    let end_str = end_next.format("%Y-%m-%d").to_string();

    let mut extra_filter = String::new();
    let mut extra_params: Vec<Box<dyn duckdb::types::ToSql>> = Vec::new();
    extra_params.push(Box::new(website_id.to_string()));
    extra_params.push(Box::new(start_str));
    extra_params.push(Box::new(end_str));
    let mut idx = 4;

    if let Some(ref country) = filter.filter_country {
        extra_filter.push_str(&format!(" AND country = ?{}", idx));
        extra_params.push(Box::new(country.clone()));
        idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        extra_filter.push_str(&format!(" AND url LIKE ?{}", idx));
        extra_params.push(Box::new(format!("%{}%", page)));
        idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        extra_filter.push_str(&format!(" AND referrer_domain = ?{}", idx));
        extra_params.push(Box::new(referrer.clone()));
        idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        extra_filter.push_str(&format!(" AND browser = ?{}", idx));
        extra_params.push(Box::new(browser.clone()));
        idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        extra_filter.push_str(&format!(" AND os = ?{}", idx));
        extra_params.push(Box::new(os.clone()));
        idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        extra_filter.push_str(&format!(" AND device_type = ?{}", idx));
        extra_params.push(Box::new(device.clone()));
        idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        extra_filter.push_str(&format!(" AND language = ?{}", idx));
        extra_params.push(Box::new(language.clone()));
        idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        extra_filter.push_str(&format!(" AND utm_source = ?{}", idx));
        extra_params.push(Box::new(utm_source.clone()));
        idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        extra_filter.push_str(&format!(" AND utm_medium = ?{}", idx));
        extra_params.push(Box::new(utm_medium.clone()));
        idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        extra_filter.push_str(&format!(" AND utm_campaign = ?{}", idx));
        extra_params.push(Box::new(utm_campaign.clone()));
        idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        extra_filter.push_str(&format!(" AND region = ?{}", idx));
        extra_params.push(Box::new(region.clone()));
        idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        extra_filter.push_str(&format!(" AND city = ?{}", idx));
        extra_params.push(Box::new(city.clone()));
        idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        extra_filter.push_str(&format!(
            " AND lower(regexp_extract(url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            idx
        ));
        extra_params.push(Box::new(hostname.clone()));
        idx += 1;
    }

    let column_expr = match metric_type {
        "page" => "url",
        "referrer" => "COALESCE(referrer_domain, '(direct)')",
        "country" => "country",
        "region" => "region",
        "city" => "city",
        "browser" => "browser",
        "os" => "os",
        "device" => "device_type",
        "language" => "language",
        "screen" => "screen",
        "event_name" => "event_name",
        "utm_source" => "utm_source",
        "utm_medium" => "utm_medium",
        "utm_campaign" => "utm_campaign",
        _ => return Err(anyhow!("invalid metric type")),
    };

    // Pages and event names are still ordered by total pageviews; everything else by unique visitors.
    let order_by = match metric_type {
        "page" | "event_name" => "pageviews DESC",
        _ => "visitors DESC",
    };

    // Count distinct dimension values for pagination (cheap flat query, no CTE needed).
    let count_sql = format!(
        "SELECT COUNT(DISTINCT {column_expr}) FROM events \
         WHERE website_id = ?1 AND created_at >= ?2 AND created_at < ?3 \
         AND {column_expr} IS NOT NULL{extra_filter}"
    );

    let count_refs: Vec<&dyn duckdb::types::ToSql> =
        extra_params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .prepare(&count_sql)?
        .query_row(count_refs.as_slice(), |row| row.get(0))?;

    extra_params.push(Box::new(limit));
    extra_params.push(Box::new(offset));

    // CTE groups events → per-session stats, then aggregates per dimension value.
    // This gives us visitors, pageviews, bounce_rate, and avg_duration in one pass.
    //
    // Bounce = session with ≤ 1 pageview (pv_count FILTER WHERE type = 'pageview').
    // Duration = epoch difference between first and last event in the session.
    //   Sessions where all events occur at the same timestamp get dur_s = 0 and are
    //   excluded from the AVG to avoid pulling the mean down artificially.
    let data_sql = format!(
        "WITH sess AS ( \
           SELECT {column_expr} AS dim_value, \
                  session_id, \
                  visitor_id, \
                  SUM(CASE WHEN event_type = 'pageview' THEN 1 ELSE 0 END) AS pv_count, \
                  CAST(DATEDIFF('second', MIN(created_at), MAX(created_at)) AS DOUBLE) AS dur_s \
           FROM events \
           WHERE website_id = ?1 AND created_at >= ?2 AND created_at < ?3 \
           AND {column_expr} IS NOT NULL{extra_filter} \
           GROUP BY dim_value, session_id, visitor_id \
         ) \
         SELECT dim_value, \
                COUNT(DISTINCT visitor_id) AS visitors, \
                COALESCE(SUM(pv_count), 0) AS pageviews, \
                COALESCE(ROUND( \
                  100.0 * SUM(CASE WHEN pv_count <= 1 THEN 1.0 ELSE 0.0 END) \
                        / NULLIF(COUNT(*), 0), 1), 0.0) AS bounce_rate, \
                COALESCE(ROUND(AVG(CASE WHEN dur_s > 0 THEN dur_s END), 1), 0.0) \
                  AS avg_duration_seconds \
         FROM sess \
         GROUP BY dim_value \
         ORDER BY {order_by} \
         LIMIT ?{idx} OFFSET ?{}",
        idx + 1,
    );

    let data_refs: Vec<&dyn duckdb::types::ToSql> =
        extra_params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&data_sql)?;
    let rows_iter = stmt.query_map(data_refs.as_slice(), |row| {
        Ok(MetricRow {
            value: row.get(0)?,
            visitors: row.get(1)?,
            pageviews: Some(row.get(2)?),
            bounce_rate: row.get(3)?,
            avg_duration_seconds: row.get(4)?,
        })
    })?;

    let mut rows = Vec::new();
    for row in rows_iter {
        rows.push(row?);
    }

    Ok((
        MetricsResult {
            metric_type: metric_type.to_string(),
            rows,
        },
        MetricsPagination {
            total,
            limit,
            offset,
            has_more: offset + limit < total,
        },
    ))
}

impl DuckDbBackend {
    pub async fn get_metrics(
        &self,
        website_id: &str,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
    ) -> Result<(MetricsResult, MetricsPagination)> {
        get_metrics_inner(self, website_id, metric_type, limit, offset, filter).await
    }

    pub async fn get_metrics_page(
        &self,
        website_id: &str,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
    ) -> Result<MetricsPage> {
        let (result, pagination) =
            get_metrics_inner(self, website_id, metric_type, limit, offset, filter).await?;
        Ok(MetricsPage {
            rows: result.rows,
            total: pagination.total,
        })
    }
}
