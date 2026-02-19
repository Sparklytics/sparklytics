use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde_json::Value;
use tracing::info;

use sparklytics_duckdb::queries::{
    metrics::{MetricRow, MetricsPagination, MetricsResult},
    realtime::{RealtimeEvent, RealtimePagination, RealtimeResult},
    stats::StatsResult,
    timeseries::{TimeseriesPoint, TimeseriesResult},
};

/// HTTP client wrapper for ClickHouse.
///
/// Uses ClickHouse's HTTP API: SQL is posted as the request body; named
/// parameters are passed as `param_<name>` query-string entries, allowing
/// ClickHouse to substitute them safely (no SQL injection).
#[derive(Clone)]
pub struct ClickHouseClient {
    client: Client,
    url: String,
    user: String,
    password: String,
}

impl ClickHouseClient {
    pub fn new(url: &str, user: &str, password: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.to_string(),
            user: user.to_string(),
            password: password.to_string(),
        }
    }

    /// Execute a ClickHouse SELECT query and return the `data` array.
    ///
    /// `named_params` maps `{name:Type}` placeholder name → value string.
    async fn query(&self, sql: &str, named_params: &[(&str, &str)]) -> Result<Vec<Value>> {
        let base_params: Vec<(String, &str)> = vec![
            ("default_format".to_string(), "JSON"),
            ("database".to_string(), "sparklytics"),
        ];
        let param_pairs: Vec<(String, &str)> = named_params
            .iter()
            .map(|(k, v)| (format!("param_{k}"), *v))
            .collect();

        let mut url = reqwest::Url::parse(&self.url).context("Invalid ClickHouse URL")?;
        {
            let mut qs = url.query_pairs_mut();
            for (k, v) in &base_params {
                qs.append_pair(k, v);
            }
            for (k, v) in &param_pairs {
                qs.append_pair(k, v);
            }
        }

        let resp = self
            .client
            .post(url)
            .basic_auth(&self.user, Some(&self.password))
            .body(sql.to_string())
            .send()
            .await
            .context("ClickHouse HTTP request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ClickHouse error {status}: {body}");
        }

        let json: Value = resp
            .json()
            .await
            .context("ClickHouse response parse failed")?;
        Ok(json
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Execute a SELECT query and return the raw response body as bytes.
    ///
    /// The caller is responsible for appending `FORMAT CSVWithNames` (or any
    /// other format) to the SQL string before calling this method. Unlike
    /// `query()`, this method does NOT add `default_format=JSON` to the URL.
    pub async fn query_raw_bytes(
        &self,
        sql: &str,
        named_params: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        let param_pairs: Vec<(String, &str)> = named_params
            .iter()
            .map(|(k, v)| (format!("param_{k}"), *v))
            .collect();

        let mut url = reqwest::Url::parse(&self.url).context("Invalid ClickHouse URL")?;
        {
            let mut qs = url.query_pairs_mut();
            qs.append_pair("database", "sparklytics");
            for (k, v) in &param_pairs {
                qs.append_pair(k, v);
            }
        }

        let resp = self
            .client
            .post(url)
            .basic_auth(&self.user, Some(&self.password))
            .body(sql.to_string())
            .send()
            .await
            .context("ClickHouse HTTP request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ClickHouse error {status}: {body}");
        }

        let bytes = resp.bytes().await.context("ClickHouse response read failed")?;
        Ok(bytes.to_vec())
    }

    /// Execute a DDL statement (no rows returned).
    async fn execute(&self, sql: &str) -> Result<()> {
        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .body(sql.to_string())
            .send()
            .await
            .context("ClickHouse DDL failed")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ClickHouse DDL error: {body}");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Schema migration (idempotent — uses IF NOT EXISTS)
// ---------------------------------------------------------------------------

/// Apply the ClickHouse schema on startup.
///
/// Creates the `sparklytics` database and both analytics tables.
/// Safe to call on every startup — all statements use `IF NOT EXISTS`.
pub async fn clickhouse_migrate(client: &ClickHouseClient) -> Result<()> {
    info!("Running ClickHouse schema migration");

    client
        .execute("CREATE DATABASE IF NOT EXISTS sparklytics")
        .await?;

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS sparklytics.events (
                id              String,
                website_id      LowCardinality(String),
                tenant_id       LowCardinality(String),
                session_id      String,
                visitor_id      FixedString(16),
                event_type      LowCardinality(String),
                url             String,
                referrer_url    Nullable(String),
                referrer_domain LowCardinality(Nullable(String)),
                event_name      LowCardinality(Nullable(String)),
                event_data      Nullable(String),
                country         LowCardinality(Nullable(FixedString(2))),
                region          LowCardinality(Nullable(String)),
                city            Nullable(String),
                browser         LowCardinality(Nullable(String)),
                browser_version Nullable(String),
                os              LowCardinality(Nullable(String)),
                os_version      Nullable(String),
                device_type     LowCardinality(Nullable(String)),
                screen          Nullable(String),
                language        LowCardinality(Nullable(String)),
                utm_source      LowCardinality(Nullable(String)),
                utm_medium      LowCardinality(Nullable(String)),
                utm_campaign    Nullable(String),
                utm_term        Nullable(String),
                utm_content     Nullable(String),
                created_at      DateTime64(3, 'UTC')
            ) ENGINE = MergeTree()
            PARTITION BY toYYYYMM(created_at)
            ORDER BY (tenant_id, website_id, created_at)
            TTL created_at + INTERVAL 6 MONTH",
        )
        .await?;

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS sparklytics.sessions (
                session_id      String,
                website_id      LowCardinality(String),
                tenant_id       LowCardinality(String),
                visitor_id      FixedString(16),
                first_seen      DateTime64(3, 'UTC'),
                last_seen       DateTime64(3, 'UTC'),
                pageview_count  UInt32 DEFAULT 1,
                entry_page      String,
                country         LowCardinality(Nullable(String)),
                browser         LowCardinality(Nullable(String)),
                device_type     LowCardinality(Nullable(String))
            ) ENGINE = ReplacingMergeTree(last_seen)
            ORDER BY (tenant_id, website_id, session_id)
            PARTITION BY toYYYYMM(first_seen)",
        )
        .await?;

    info!("ClickHouse schema migration complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: format NaiveDate to a string ClickHouse accepts as DateTime64(3)
// ---------------------------------------------------------------------------

fn date_to_ch(date: &NaiveDate) -> String {
    date.format("%Y-%m-%d 00:00:00.000").to_string()
}

fn date_next_to_ch(date: &NaiveDate) -> String {
    (*date + chrono::Duration::days(1))
        .format("%Y-%m-%d 00:00:00.000")
        .to_string()
}

// ---------------------------------------------------------------------------
// Dimension filters
// ---------------------------------------------------------------------------

/// Analytics dimension filters forwarded from HTTP query params to ClickHouse.
///
/// All fields match `sparklytics.events` table columns using exact-value
/// equality. `None` means "no filter on this dimension" — the WHERE clause is
/// not altered and all values for that dimension are included.
#[derive(Debug, Default)]
pub struct ChFilters<'a> {
    pub country: Option<&'a str>,
    pub page: Option<&'a str>,
    pub referrer: Option<&'a str>,
    pub browser: Option<&'a str>,
    pub os: Option<&'a str>,
    pub device: Option<&'a str>,
    pub utm_source: Option<&'a str>,
    pub utm_medium: Option<&'a str>,
    pub utm_campaign: Option<&'a str>,
}

impl<'a> ChFilters<'a> {
    /// Returns `(sql_fragment, owned_params)`.
    ///
    /// `sql_fragment` is zero or more `AND col = {param:String}` lines
    /// suitable for appending to an existing ClickHouse WHERE clause.
    /// `owned_params` are `(name, value)` pairs to merge into the named-param
    /// list before converting to `&[(&str, &str)]` for `ClickHouseClient::query`.
    pub fn filter_clause(&self) -> (String, Vec<(String, String)>) {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<(String, String)> = Vec::new();

        macro_rules! push_filter {
            ($opt:expr, $param:literal, $col:literal) => {
                if let Some(val) = $opt {
                    clauses.push(format!("AND {} = {{{}:String}}", $col, $param));
                    params.push(($param.to_string(), val.to_string()));
                }
            };
        }

        push_filter!(self.country, "filter_country", "country");
        push_filter!(self.page, "filter_page", "url");
        push_filter!(self.referrer, "filter_referrer", "referrer_domain");
        push_filter!(self.browser, "filter_browser", "browser");
        push_filter!(self.os, "filter_os", "os");
        push_filter!(self.device, "filter_device", "device_type");
        push_filter!(self.utm_source, "filter_utm_source", "utm_source");
        push_filter!(self.utm_medium, "filter_utm_medium", "utm_medium");
        push_filter!(self.utm_campaign, "filter_utm_campaign", "utm_campaign");

        (clauses.join("\n          "), params)
    }
}

// ---------------------------------------------------------------------------
// Analytics queries
// All WHERE clauses include tenant_id = {tenant_id:String} (CLAUDE.md fact #8)
// Date params use :DateTime64(3) so ClickHouse can apply partition pruning.
// ---------------------------------------------------------------------------

/// Summary stats for the date range, plus comparison to the previous period.
///
/// Returns the same [`StatsResult`] type that the DuckDB backend returns,
/// so the existing handler serialises it identically.
pub async fn ch_stats(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start_date: &NaiveDate,
    end_date: &NaiveDate,
    timezone: &str,
    filters: &ChFilters<'_>,
) -> Result<StatsResult> {
    let start = date_to_ch(start_date);
    let end = date_next_to_ch(end_date);

    let range_days = (*end_date - *start_date).num_days() + 1;
    let prev_end_date = *start_date - chrono::Duration::days(1);
    let prev_start_date = prev_end_date - chrono::Duration::days(range_days - 1);
    let prev_start = date_to_ch(&prev_start_date);
    let prev_end = date_next_to_ch(&prev_end_date);

    let (pv, vis, sess) =
        query_period_basics(client, tenant_id, website_id, &start, &end, filters).await?;
    let bounce_rate =
        query_bounce_rate(client, tenant_id, website_id, &start, &end, filters).await?;
    let avg_dur = query_avg_duration(client, tenant_id, website_id, &start, &end).await?;

    let (prev_pv, prev_vis, prev_sess) =
        query_period_basics(client, tenant_id, website_id, &prev_start, &prev_end, filters)
            .await?;
    let prev_bounce =
        query_bounce_rate(client, tenant_id, website_id, &prev_start, &prev_end, filters).await?;
    let prev_dur =
        query_avg_duration(client, tenant_id, website_id, &prev_start, &prev_end).await?;

    Ok(StatsResult {
        pageviews: pv,
        visitors: vis,
        sessions: sess,
        bounce_rate,
        avg_duration_seconds: avg_dur,
        timezone: timezone.to_string(),
        prev_pageviews: prev_pv,
        prev_visitors: prev_vis,
        prev_sessions: prev_sess,
        prev_bounce_rate: prev_bounce,
        prev_avg_duration_seconds: prev_dur,
    })
}

async fn query_period_basics(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start: &str,
    end: &str,
    filters: &ChFilters<'_>,
) -> Result<(i64, i64, i64)> {
    let (filter_clause, filter_params) = filters.filter_clause();
    // :DateTime64(3) enables partition pruning via toYYYYMM(created_at).
    let sql = format!(
        "SELECT
            countIf(event_type = 'pageview') AS pageviews,
            uniqIf(visitor_id, event_type = 'pageview') AS visitors,
            uniqIf(session_id, event_type = 'pageview') AS sessions
         FROM sparklytics.events
         WHERE tenant_id = {{tenant_id:String}}
           AND website_id = {{website_id:String}}
           AND created_at >= {{start:DateTime64(3)}}
           AND created_at < {{end:DateTime64(3)}}
           {filter_clause}"
    );

    let mut all: Vec<(String, String)> = vec![
        ("tenant_id".into(), tenant_id.into()),
        ("website_id".into(), website_id.into()),
        ("start".into(), start.into()),
        ("end".into(), end.into()),
    ];
    all.extend(filter_params);
    let named: Vec<(&str, &str)> = all.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let rows = client.query(&sql, &named).await?;

    let row = rows.first().cloned().unwrap_or_default();
    let pv = row.get("pageviews").and_then(|v| v.as_i64()).unwrap_or(0);
    let vis = row.get("visitors").and_then(|v| v.as_i64()).unwrap_or(0);
    let sess = row.get("sessions").and_then(|v| v.as_i64()).unwrap_or(0);
    Ok((pv, vis, sess))
}

async fn query_bounce_rate(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start: &str,
    end: &str,
    filters: &ChFilters<'_>,
) -> Result<f64> {
    let (filter_clause, filter_params) = filters.filter_clause();
    // Denominator uses countIf(pv_count >= 1) to exclude sessions with zero
    // pageviews (e.g. custom events only) from the bounce rate calculation.
    let sql = format!(
        "SELECT
            countIf(pv_count = 1)    AS bounced,
            countIf(pv_count >= 1)   AS total
         FROM (
            SELECT session_id, countIf(event_type = 'pageview') AS pv_count
            FROM sparklytics.events
            WHERE tenant_id = {{tenant_id:String}}
              AND website_id = {{website_id:String}}
              AND created_at >= {{start:DateTime64(3)}}
              AND created_at < {{end:DateTime64(3)}}
              {filter_clause}
            GROUP BY session_id
         )"
    );

    let mut all: Vec<(String, String)> = vec![
        ("tenant_id".into(), tenant_id.into()),
        ("website_id".into(), website_id.into()),
        ("start".into(), start.into()),
        ("end".into(), end.into()),
    ];
    all.extend(filter_params);
    let named: Vec<(&str, &str)> = all.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let rows = client.query(&sql, &named).await?;

    let row = rows.first().cloned().unwrap_or_default();
    let bounced = row.get("bounced").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let total = row.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
    Ok(if total == 0.0 { 0.0 } else { bounced / total })
}

async fn query_avg_duration(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start: &str,
    end: &str,
) -> Result<f64> {
    let sql = "
        SELECT avg(
            toUnixTimestamp64Milli(last_seen) - toUnixTimestamp64Milli(first_seen)
        ) / 1000.0 AS avg_secs
        FROM sparklytics.sessions
        WHERE tenant_id = {tenant_id:String}
          AND website_id = {website_id:String}
          AND first_seen >= {start:DateTime64(3)}
          AND first_seen < {end:DateTime64(3)}
    ";

    let rows = client
        .query(
            sql,
            &[
                ("tenant_id", tenant_id),
                ("website_id", website_id),
                ("start", start),
                ("end", end),
            ],
        )
        .await?;

    Ok(rows
        .first()
        .and_then(|r| r.get("avg_secs"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0))
}

/// Time-series pageviews + unique visitors.
pub async fn ch_pageviews(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start_date: &NaiveDate,
    end_date: &NaiveDate,
    granularity: Option<&str>,
    filters: &ChFilters<'_>,
) -> Result<TimeseriesResult> {
    let start = date_to_ch(start_date);
    let end = date_next_to_ch(end_date);

    let gran = match granularity {
        Some("hour") => "hour",
        Some("month") => "month",
        _ => "day",
    };

    let trunc_fn = match gran {
        "hour" => "toStartOfHour",
        "month" => "toStartOfMonth",
        _ => "toStartOfDay",
    };

    let (filter_clause, filter_params) = filters.filter_clause();
    // :DateTime64(3) enables partition pruning.
    let sql = format!(
        "SELECT
            formatDateTime({trunc_fn}(created_at), '%Y-%m-%d %H:%i:%S') AS date,
            countIf(event_type = 'pageview') AS pageviews,
            uniqIf(visitor_id, event_type = 'pageview') AS visitors
         FROM sparklytics.events
         WHERE tenant_id = {{tenant_id:String}}
           AND website_id = {{website_id:String}}
           AND created_at >= {{start:DateTime64(3)}}
           AND created_at < {{end:DateTime64(3)}}
           {filter_clause}
         GROUP BY date
         ORDER BY date ASC"
    );

    let mut all: Vec<(String, String)> = vec![
        ("tenant_id".into(), tenant_id.into()),
        ("website_id".into(), website_id.into()),
        ("start".into(), start.clone()),
        ("end".into(), end.clone()),
    ];
    all.extend(filter_params);
    let named: Vec<(&str, &str)> = all.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let rows = client.query(&sql, &named).await?;

    let series = rows
        .iter()
        .map(|r| TimeseriesPoint {
            date: r
                .get("date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            pageviews: r.get("pageviews").and_then(|v| v.as_i64()).unwrap_or(0),
            visitors: r.get("visitors").and_then(|v| v.as_i64()).unwrap_or(0),
        })
        .collect();

    Ok(TimeseriesResult {
        series,
        granularity: gran.to_string(),
    })
}

/// Dimension breakdown (page, referrer, country, browser, …).
#[allow(clippy::too_many_arguments)]
pub async fn ch_metrics(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
    start_date: &NaiveDate,
    end_date: &NaiveDate,
    metric_type: &str,
    limit: i64,
    offset: i64,
    filters: &ChFilters<'_>,
) -> Result<(MetricsResult, MetricsPagination)> {
    let start = date_to_ch(start_date);
    let end = date_next_to_ch(end_date);
    let limit_s = limit.to_string();
    let offset_s = offset.to_string();

    let (value_expr, include_pageviews) = match metric_type {
        "page" => ("url", true),
        "referrer" => (
            "if(referrer_domain = '' OR isNull(referrer_domain), '(direct)', referrer_domain)",
            false,
        ),
        "country" => ("if(isNull(country), '(unknown)', country)", false),
        "browser" => ("if(isNull(browser), '(unknown)', browser)", false),
        "os" => ("if(isNull(os), '(unknown)', os)", false),
        "device" => ("if(isNull(device_type), '(unknown)', device_type)", false),
        "language" => ("if(isNull(language), '(unknown)', language)", false),
        "screen" => ("if(isNull(screen), '(unknown)', screen)", false),
        "utm_source" => ("if(isNull(utm_source), '(none)', utm_source)", false),
        "utm_medium" => ("if(isNull(utm_medium), '(none)', utm_medium)", false),
        "utm_campaign" => ("if(isNull(utm_campaign), '(none)', utm_campaign)", false),
        _ => ("url", true),
    };

    let pv_col = if include_pageviews {
        ", countIf(event_type = 'pageview') AS pageviews"
    } else {
        ""
    };
    let order_col = if include_pageviews {
        "pageviews"
    } else {
        "visitors"
    };

    let (filter_clause, filter_params) = filters.filter_clause();
    // :DateTime64(3) enables partition pruning.
    let data_sql = format!(
        "SELECT {value_expr} AS value, uniq(visitor_id) AS visitors{pv_col}
         FROM sparklytics.events
         WHERE tenant_id = {{tenant_id:String}}
           AND website_id = {{website_id:String}}
           AND created_at >= {{start:DateTime64(3)}}
           AND created_at < {{end:DateTime64(3)}}
           {filter_clause}
         GROUP BY value
         ORDER BY {order_col} DESC
         LIMIT {{limit:UInt32}} OFFSET {{offset:UInt32}}"
    );

    let count_sql = format!(
        "SELECT uniq({value_expr}) AS total
         FROM sparklytics.events
         WHERE tenant_id = {{tenant_id:String}}
           AND website_id = {{website_id:String}}
           AND created_at >= {{start:DateTime64(3)}}
           AND created_at < {{end:DateTime64(3)}}
           {filter_clause}"
    );

    let base: Vec<(String, String)> = vec![
        ("tenant_id".into(), tenant_id.into()),
        ("website_id".into(), website_id.into()),
        ("start".into(), start.clone()),
        ("end".into(), end.clone()),
    ];

    let mut data_params = base.clone();
    data_params.extend(filter_params.clone());
    data_params.push(("limit".into(), limit_s.clone()));
    data_params.push(("offset".into(), offset_s.clone()));
    let data_named: Vec<(&str, &str)> = data_params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let mut count_params = base;
    count_params.extend(filter_params);
    let count_named: Vec<(&str, &str)> = count_params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let data_rows = client.query(&data_sql, &data_named).await?;
    let count_rows = client.query(&count_sql, &count_named).await?;

    let total = count_rows
        .first()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let rows: Vec<MetricRow> = data_rows
        .iter()
        .map(|r| MetricRow {
            value: r
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            pageviews: r.get("pageviews").and_then(|v| v.as_i64()),
            visitors: r.get("visitors").and_then(|v| v.as_i64()).unwrap_or(0),
        })
        .collect();

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

/// Active visitors + recent events from the last 30 minutes.
pub async fn ch_realtime(
    client: &ClickHouseClient,
    tenant_id: &str,
    website_id: &str,
) -> Result<RealtimeResult> {
    // Active visitors: unique visitor_ids in the last 30 minutes (matches spec).
    let active_sql = "
        SELECT uniq(visitor_id) AS active_visitors
        FROM sparklytics.events
        WHERE tenant_id = {tenant_id:String}
          AND website_id = {website_id:String}
          AND created_at >= now() - INTERVAL 30 MINUTE
    ";

    let active_rows = client
        .query(
            active_sql,
            &[("tenant_id", tenant_id), ("website_id", website_id)],
        )
        .await?;
    let active_visitors = active_rows
        .first()
        .and_then(|r| r.get("active_visitors"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let events_sql = "
        SELECT
            url,
            referrer_domain,
            country,
            browser,
            device_type,
            event_type,
            formatDateTime(created_at, '%Y-%m-%dT%H:%i:%SZ') AS ts
        FROM sparklytics.events
        WHERE tenant_id = {tenant_id:String}
          AND website_id = {website_id:String}
          AND created_at >= now() - INTERVAL 30 MINUTE
        ORDER BY created_at DESC
        LIMIT 100
    ";

    let event_rows = client
        .query(
            events_sql,
            &[("tenant_id", tenant_id), ("website_id", website_id)],
        )
        .await?;

    let total_sql = "
        SELECT count() AS total
        FROM sparklytics.events
        WHERE tenant_id = {tenant_id:String}
          AND website_id = {website_id:String}
          AND created_at >= now() - INTERVAL 30 MINUTE
    ";
    let total_rows = client
        .query(
            total_sql,
            &[("tenant_id", tenant_id), ("website_id", website_id)],
        )
        .await?;
    let total_in_window = total_rows
        .first()
        .and_then(|r| r.get("total"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let recent_events: Vec<RealtimeEvent> = event_rows
        .iter()
        .map(|r| RealtimeEvent {
            url: r
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            referrer_domain: r
                .get("referrer_domain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            country: r
                .get("country")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            browser: r
                .get("browser")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            device_type: r
                .get("device_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            event_type: r
                .get("event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("pageview")
                .to_string(),
            ts: r
                .get("ts")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(RealtimeResult {
        active_visitors,
        recent_events,
        pagination: RealtimePagination {
            limit: 100,
            total_in_window,
        },
    })
}

/// Verify that `website_id` belongs to `tenant_id` in PostgreSQL.
///
/// Returns `true` if the website exists, belongs to the tenant, and the tenant
/// has not been soft-deleted. All cloud analytics endpoints call this to
/// enforce tenant isolation.
pub async fn website_belongs_to_tenant(
    pool: &sqlx::PgPool,
    website_id: &str,
    tenant_id: &str,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM websites
         WHERE id = $1 AND tenant_id = $2
         AND tenant_id IN (SELECT id FROM tenants WHERE deleted_at IS NULL)",
    )
    .bind(website_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}
