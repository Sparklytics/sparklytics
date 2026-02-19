use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use serde::Serialize;

use crate::DuckDbBackend;

#[derive(Debug, Clone, Serialize)]
pub struct MetricRow {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pageviews: Option<i64>,
    pub visitors: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsResult {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub rows: Vec<MetricRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsPagination {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

/// Valid metric types for the metrics endpoint.
const VALID_TYPES: &[&str] = &[
    "page",
    "referrer",
    "country",
    "browser",
    "os",
    "device",
    "language",
    "screen",
    "utm_source",
    "utm_medium",
    "utm_campaign",
];

pub fn is_valid_metric_type(t: &str) -> bool {
    VALID_TYPES.contains(&t)
}

impl DuckDbBackend {
    #[allow(clippy::too_many_arguments)]
    pub async fn get_metrics(
        &self,
        website_id: &str,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter_country: Option<&str>,
        filter_page: Option<&str>,
    ) -> Result<(MetricsResult, MetricsPagination)> {
        if !is_valid_metric_type(metric_type) {
            return Err(anyhow!("invalid metric type: {}", metric_type));
        }

        let conn = self.conn.lock().await;
        let start_str = start_date.format("%Y-%m-%d").to_string();
        let end_next = *end_date + chrono::Duration::days(1);
        let end_str = end_next.format("%Y-%m-%d").to_string();

        let mut extra_filter = String::new();
        let mut extra_params: Vec<Box<dyn duckdb::types::ToSql>> = Vec::new();
        // Base: ?1=website_id, ?2=start, ?3=end
        extra_params.push(Box::new(website_id.to_string()));
        extra_params.push(Box::new(start_str));
        extra_params.push(Box::new(end_str));
        let mut idx = 4;

        if let Some(country) = filter_country {
            extra_filter.push_str(&format!(" AND country = ?{}", idx));
            extra_params.push(Box::new(country.to_string()));
            idx += 1;
        }
        if let Some(page) = filter_page {
            extra_filter.push_str(&format!(" AND url LIKE ?{}", idx));
            extra_params.push(Box::new(format!("%{}%", page)));
            idx += 1;
        }

        // Map metric_type to the SQL column and whether to include pageviews.
        let (column_expr, include_pageviews) = match metric_type {
            "page" => ("url", true),
            "referrer" => ("COALESCE(referrer_domain, '(direct)')", false),
            "country" => ("country", false),
            "browser" => ("browser", false),
            "os" => ("os", false),
            "device" => ("device_type", false),
            "language" => ("language", false),
            "screen" => ("screen", false),
            "utm_source" => ("utm_source", false),
            "utm_medium" => ("utm_medium", false),
            "utm_campaign" => ("utm_campaign", false),
            _ => return Err(anyhow!("invalid metric type")),
        };

        let select_extra = if include_pageviews {
            ", COUNT(*) AS pageviews"
        } else {
            ""
        };

        let order_by = if include_pageviews {
            "pageviews DESC"
        } else {
            "visitors DESC"
        };

        // Count total distinct values.
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

        // Add limit and offset params.
        extra_params.push(Box::new(limit));
        extra_params.push(Box::new(offset));

        let data_sql = format!(
            "SELECT {column_expr} AS dim_value, COUNT(DISTINCT visitor_id) AS visitors{select_extra} \
             FROM events \
             WHERE website_id = ?1 AND created_at >= ?2 AND created_at < ?3 \
             AND {column_expr} IS NOT NULL{extra_filter} \
             GROUP BY dim_value \
             ORDER BY {order_by} \
             LIMIT ?{idx} OFFSET ?{}",
            idx + 1,
        );

        let data_refs: Vec<&dyn duckdb::types::ToSql> =
            extra_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&data_sql)?;
        let rows_iter = stmt.query_map(data_refs.as_slice(), |row| {
            let value: String = row.get(0)?;
            let visitors: i64 = row.get(1)?;
            let pageviews: Option<i64> = if include_pageviews {
                Some(row.get(2)?)
            } else {
                None
            };
            Ok(MetricRow {
                value,
                pageviews,
                visitors,
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
}
