//! Analytics backend abstraction.

use chrono::NaiveDate;
use serde::Serialize;

use crate::event::Event;

/// Optional dimension filters applied uniformly to analytics queries.
#[derive(Debug, Clone, Default)]
pub struct AnalyticsFilter {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub timezone: Option<String>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsResult {
    pub pageviews: i64,
    pub visitors: i64,
    pub sessions: i64,
    pub bounce_rate: f64,
    pub avg_duration_seconds: f64,
    pub prev_pageviews: i64,
    pub prev_visitors: i64,
    pub prev_sessions: i64,
    pub prev_bounce_rate: f64,
    pub prev_avg_duration_seconds: f64,
    pub timezone: String,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct MetricRow {
    pub value: String,
    pub visitors: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pageviews: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct MetricsPage {
    pub rows: Vec<MetricRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealtimeEvent {
    pub event_type: String,
    pub url: String,
    pub referrer_domain: Option<String>,
    pub country: Option<String>,
    pub browser: Option<String>,
    pub device_type: Option<String>,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealtimePagination {
    pub limit: i64,
    pub total_in_window: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RealtimeResult {
    pub active_visitors: i64,
    pub recent_events: Vec<RealtimeEvent>,
    pub pagination: RealtimePagination,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportRow {
    pub id: String,
    pub website_id: String,
    pub event_type: String,
    pub url: String,
    pub referrer_domain: Option<String>,
    pub event_name: Option<String>,
    pub country: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device_type: Option<String>,
    pub language: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventNameRow {
    pub event_name: String,
    pub count: i64,
    pub visitors: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPropertyRow {
    pub property_key: String,
    pub property_value: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventNamesResult {
    pub rows: Vec<EventNameRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPropertiesResult {
    pub event_name: String,
    pub total_occurrences: i64,
    pub sample_size: i64,
    pub properties: Vec<EventPropertyRow>,
}

pub const VALID_METRIC_TYPES: &[&str] = &[
    "page",
    "referrer",
    "country",
    "browser",
    "os",
    "device",
    "screen",
    "event_name",
    "language",
    "utm_source",
    "utm_medium",
    "utm_campaign",
];

#[async_trait::async_trait]
pub trait AnalyticsBackend: Send + Sync + 'static {
    async fn insert_events(&self, events: &[Event]) -> anyhow::Result<()>;

    async fn get_or_create_session(
        &self,
        website_id: &str,
        visitor_id: &str,
        referrer_domain: Option<&str>,
        url: &str,
    ) -> anyhow::Result<String>;

    async fn get_stats(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<StatsResult>;

    async fn get_timeseries(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
    ) -> anyhow::Result<TimeseriesResult>;

    async fn get_metrics(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<MetricsPage>;

    async fn get_realtime(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<RealtimeResult>;

    async fn export_events(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<ExportRow>>;

    async fn get_event_names(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<EventNamesResult>;

    async fn get_event_properties(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        event_name: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<EventPropertiesResult>;

    async fn get_event_timeseries(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        event_name: &str,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
    ) -> anyhow::Result<TimeseriesResult>;
}
