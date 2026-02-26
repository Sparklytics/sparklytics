use anyhow::{anyhow, Result};

use sparklytics_core::analytics::{
    AnalyticsFilter, ComparisonRange, MetricRow, MetricsPage, VALID_METRIC_TYPES,
};

use crate::queries::bot_filters::append_event_bot_filter;
use crate::DuckDbBackend;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsResult {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub rows: Vec<MetricRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<sparklytics_core::analytics::ComparisonMetadata>,
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
    comparison: Option<&ComparisonRange>,
) -> Result<(MetricsResult, MetricsPagination)> {
    if !is_valid_metric_type(metric_type) {
        return Err(anyhow!("invalid metric type: {}", metric_type));
    }

    let conn = db.conn.lock().await;
    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];

    let mut idx = 4;
    if let Some(compare) = comparison {
        params.push(Box::new(
            compare.comparison_start.format("%Y-%m-%d").to_string(),
        ));
        params.push(Box::new(
            (compare.comparison_end + chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string(),
        ));
        idx = 6;
    }

    let mut extra_filter = String::new();
    append_event_bot_filter(&mut extra_filter, filter.include_bots, "e.");
    if let Some(ref country) = filter.filter_country {
        extra_filter.push_str(&format!(" AND e.country = ?{}", idx));
        params.push(Box::new(country.clone()));
        idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        extra_filter.push_str(&format!(" AND e.url LIKE ?{}", idx));
        params.push(Box::new(format!("%{}%", page)));
        idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        extra_filter.push_str(&format!(" AND e.referrer_domain = ?{}", idx));
        params.push(Box::new(referrer.clone()));
        idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        extra_filter.push_str(&format!(" AND e.browser = ?{}", idx));
        params.push(Box::new(browser.clone()));
        idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        extra_filter.push_str(&format!(" AND e.os = ?{}", idx));
        params.push(Box::new(os.clone()));
        idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        extra_filter.push_str(&format!(" AND e.device_type = ?{}", idx));
        params.push(Box::new(device.clone()));
        idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        extra_filter.push_str(&format!(" AND e.language = ?{}", idx));
        params.push(Box::new(language.clone()));
        idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        extra_filter.push_str(&format!(" AND e.utm_source = ?{}", idx));
        params.push(Box::new(utm_source.clone()));
        idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        extra_filter.push_str(&format!(" AND e.utm_medium = ?{}", idx));
        params.push(Box::new(utm_medium.clone()));
        idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        extra_filter.push_str(&format!(" AND e.utm_campaign = ?{}", idx));
        params.push(Box::new(utm_campaign.clone()));
        idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        extra_filter.push_str(&format!(" AND e.region = ?{}", idx));
        params.push(Box::new(region.clone()));
        idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        extra_filter.push_str(&format!(" AND e.city = ?{}", idx));
        params.push(Box::new(city.clone()));
        idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        extra_filter.push_str(&format!(
            " AND lower(regexp_extract(e.url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            idx
        ));
        params.push(Box::new(hostname.clone()));
        idx += 1;
    }

    let column_expr = match metric_type {
        "page" => "e.url",
        "referrer" => "COALESCE(e.referrer_domain, '(direct)')",
        "country" => "e.country",
        "region" => "e.region",
        "city" => "e.city",
        "browser" => "e.browser",
        "os" => "e.os",
        "device" => "e.device_type",
        "language" => "e.language",
        "screen" => "e.screen",
        "event_name" => "e.event_name",
        "utm_source" => "e.utm_source",
        "utm_medium" => "e.utm_medium",
        "utm_campaign" => "e.utm_campaign",
        _ => return Err(anyhow!("invalid metric type")),
    };

    let order_by = match metric_type {
        "page" | "event_name" => "p.pageviews DESC",
        _ => "p.visitors DESC",
    };

    let compare_bind_padding = if comparison.is_some() {
        " AND (?4 IS NULL OR ?5 IS NULL OR 1=1)"
    } else {
        ""
    };

    let count_sql = format!(
        "SELECT COUNT(DISTINCT {column_expr}) FROM events e \
         WHERE e.website_id = ?1 AND e.created_at >= ?2 AND e.created_at < ?3 \
         AND {column_expr} IS NOT NULL{compare_bind_padding}{extra_filter}"
    );

    let count_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .prepare(&count_sql)?
        .query_row(count_refs.as_slice(), |row| row.get(0))?;

    params.push(Box::new(limit));
    params.push(Box::new(offset));

    let data_sql = if comparison.is_some() {
        format!(
            "WITH periods AS ( \
               SELECT 'primary' AS period_name, CAST(?2 AS TIMESTAMP) AS period_start, CAST(?3 AS TIMESTAMP) AS period_end \
               UNION ALL \
               SELECT 'comparison' AS period_name, CAST(?4 AS TIMESTAMP) AS period_start, CAST(?5 AS TIMESTAMP) AS period_end \
             ), \
             sess AS ( \
               SELECT p.period_name, \
                      {column_expr} AS dim_value, \
                      e.session_id, \
                      e.visitor_id, \
                      SUM(CASE WHEN e.event_type = 'pageview' THEN 1 ELSE 0 END) AS pv_count, \
                      CAST(DATEDIFF('second', MIN(e.created_at), MAX(e.created_at)) AS DOUBLE) AS dur_s \
               FROM periods p \
               JOIN events e \
                 ON e.website_id = ?1 \
                AND e.created_at >= p.period_start \
                AND e.created_at < p.period_end \
               WHERE {column_expr} IS NOT NULL{extra_filter} \
               GROUP BY p.period_name, dim_value, e.session_id, e.visitor_id \
             ), \
             agg AS ( \
               SELECT period_name, \
                      dim_value, \
                      COUNT(DISTINCT visitor_id) AS visitors, \
                      COALESCE(SUM(pv_count), 0) AS pageviews, \
                      COALESCE(ROUND( \
                        100.0 * SUM(CASE WHEN pv_count <= 1 THEN 1.0 ELSE 0.0 END) \
                              / NULLIF(COUNT(*), 0), 1), 0.0) AS bounce_rate, \
                      COALESCE(ROUND(AVG(CASE WHEN dur_s > 0 THEN dur_s END), 1), 0.0) \
                        AS avg_duration_seconds \
               FROM sess \
               GROUP BY period_name, dim_value \
             ), \
             p AS (SELECT * FROM agg WHERE period_name = 'primary'), \
             c AS (SELECT * FROM agg WHERE period_name = 'comparison') \
             SELECT p.dim_value, \
                    p.visitors, \
                    p.pageviews, \
                    p.bounce_rate, \
                    p.avg_duration_seconds, \
                    COALESCE(c.visitors, 0) AS prev_visitors, \
                    COALESCE(c.pageviews, 0) AS prev_pageviews \
             FROM p \
             LEFT JOIN c ON c.dim_value = p.dim_value \
             ORDER BY {order_by} \
             LIMIT ?{idx} OFFSET ?{}",
            idx + 1,
        )
    } else {
        let non_compare_order_by = match metric_type {
            "page" | "event_name" => "pageviews DESC",
            _ => "visitors DESC",
        };

        format!(
            "WITH sess AS ( \
               SELECT {column_expr} AS dim_value, \
                      e.session_id, \
                      e.visitor_id, \
                      SUM(CASE WHEN e.event_type = 'pageview' THEN 1 ELSE 0 END) AS pv_count, \
                      CAST(DATEDIFF('second', MIN(e.created_at), MAX(e.created_at)) AS DOUBLE) AS dur_s \
               FROM events e \
               WHERE e.website_id = ?1 AND e.created_at >= ?2 AND e.created_at < ?3 \
               AND {column_expr} IS NOT NULL{extra_filter} \
               GROUP BY dim_value, e.session_id, e.visitor_id \
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
             ORDER BY {non_compare_order_by} \
             LIMIT ?{idx} OFFSET ?{}",
            idx + 1,
        )
    };

    let data_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&data_sql)?;

    let mut rows = Vec::new();
    if comparison.is_some() {
        let rows_iter = stmt.query_map(data_refs.as_slice(), |row| {
            let visitors: i64 = row.get(1)?;
            let prev_visitors: i64 = row.get(5)?;
            let delta_visitors_abs = visitors - prev_visitors;
            let delta_visitors_pct = if prev_visitors > 0 {
                Some((delta_visitors_abs as f64) / (prev_visitors as f64))
            } else {
                Some(0.0)
            };

            Ok(MetricRow {
                value: row.get(0)?,
                visitors,
                pageviews: Some(row.get(2)?),
                prev_visitors: Some(prev_visitors),
                prev_pageviews: Some(row.get(6)?),
                delta_visitors_abs: Some(delta_visitors_abs),
                delta_visitors_pct,
                bounce_rate: row.get(3)?,
                avg_duration_seconds: row.get(4)?,
            })
        })?;
        for row in rows_iter {
            rows.push(row?);
        }
    } else {
        let rows_iter = stmt.query_map(data_refs.as_slice(), |row| {
            Ok(MetricRow {
                value: row.get(0)?,
                visitors: row.get(1)?,
                pageviews: Some(row.get(2)?),
                prev_visitors: None,
                prev_pageviews: None,
                delta_visitors_abs: None,
                delta_visitors_pct: None,
                bounce_rate: row.get(3)?,
                avg_duration_seconds: row.get(4)?,
            })
        })?;
        for row in rows_iter {
            rows.push(row?);
        }
    }

    Ok((
        MetricsResult {
            metric_type: metric_type.to_string(),
            rows,
            compare: comparison.map(ComparisonRange::to_metadata),
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
        comparison: Option<&ComparisonRange>,
    ) -> Result<(MetricsResult, MetricsPagination)> {
        get_metrics_inner(
            self,
            website_id,
            metric_type,
            limit,
            offset,
            filter,
            comparison,
        )
        .await
    }

    pub async fn get_metrics_page(
        &self,
        website_id: &str,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
        comparison: Option<&ComparisonRange>,
    ) -> Result<MetricsPage> {
        let (result, pagination) = get_metrics_inner(
            self,
            website_id,
            metric_type,
            limit,
            offset,
            filter,
            comparison,
        )
        .await?;
        Ok(MetricsPage {
            rows: result.rows,
            total: pagination.total,
            compare: result.compare,
        })
    }
}
