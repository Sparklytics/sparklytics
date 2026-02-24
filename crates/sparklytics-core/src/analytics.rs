//! Analytics backend abstraction.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

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
    pub filter_region: Option<String>,
    pub filter_city: Option<String>,
    pub filter_hostname: Option<String>,
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
    /// Always populated as of Sprint 12 (was optional for non-page types before).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pageviews: Option<i64>,
    /// Fraction of sessions that had ≤ 1 pageview, 0–100.
    pub bounce_rate: f64,
    /// Mean session duration in seconds (0.0 when all sessions are single-event).
    pub avg_duration_seconds: f64,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SessionSort {
    #[default]
    LastSeenDesc,
}

#[derive(Debug, Clone)]
pub struct SessionsQuery {
    pub limit: u32,
    pub cursor: Option<String>,
    pub sort: SessionSort,
}

impl Default for SessionsQuery {
    fn default() -> Self {
        Self {
            limit: 50,
            cursor: None,
            sort: SessionSort::LastSeenDesc,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionListItem {
    pub session_id: String,
    pub visitor_id: String,
    pub first_seen: String,
    pub last_seen: String,
    pub duration_seconds: i64,
    pub pageview_count: i64,
    pub event_count: i64,
    pub entry_page: Option<String>,
    pub exit_page: Option<String>,
    pub country: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionsPagination {
    pub limit: u32,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct SessionsResponse {
    pub rows: Vec<SessionListItem>,
    pub pagination: SessionsPagination,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionEventItem {
    pub id: String,
    pub event_type: String,
    pub url: String,
    pub event_name: Option<String>,
    pub event_data: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionDetailResponse {
    pub session: SessionListItem,
    pub events: Vec<SessionEventItem>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    PageView,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchOperator {
    #[default]
    Equals,
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub goal_type: GoalType,
    pub match_value: String,
    pub match_operator: MatchOperator,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGoalRequest {
    pub name: String,
    pub goal_type: GoalType,
    pub match_value: String,
    pub match_operator: Option<MatchOperator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGoalRequest {
    pub name: Option<String>,
    pub match_value: Option<String>,
    pub match_operator: Option<MatchOperator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStats {
    pub goal_id: String,
    pub conversions: i64,
    pub converting_sessions: i64,
    pub total_sessions: i64,
    pub conversion_rate: f64,
    pub prev_conversions: Option<i64>,
    pub prev_conversion_rate: Option<f64>,
    pub trend_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    PageView,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelStep {
    pub id: String,
    pub funnel_id: String,
    pub step_order: u32,
    pub step_type: StepType,
    pub match_value: String,
    pub match_operator: MatchOperator,
    pub label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funnel {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub steps: Vec<FunnelStep>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelSummary {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub step_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFunnelStepRequest {
    pub step_type: StepType,
    pub match_value: String,
    pub match_operator: Option<MatchOperator>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFunnelRequest {
    pub name: String,
    pub steps: Vec<CreateFunnelStepRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFunnelRequest {
    pub name: Option<String>,
    pub steps: Option<Vec<CreateFunnelStepRequest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelStepResult {
    pub step_order: u32,
    pub label: String,
    pub sessions_reached: i64,
    pub drop_off_count: i64,
    pub drop_off_rate: f64,
    pub conversion_rate_from_start: f64,
    pub conversion_rate_from_previous: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelResults {
    pub funnel_id: String,
    pub name: String,
    pub total_sessions_entered: i64,
    pub final_conversion_rate: f64,
    pub steps: Vec<FunnelStepResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorType {
    Page,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JourneyDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone)]
pub struct JourneyQuery {
    pub anchor_type: AnchorType,
    pub anchor_value: String,
    pub direction: JourneyDirection,
    pub max_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyNode {
    #[serde(rename = "type")]
    pub node_type: AnchorType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyBranch {
    pub nodes: Vec<String>,
    pub sessions: i64,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyResponse {
    pub anchor: JourneyNode,
    pub direction: JourneyDirection,
    pub max_depth: u32,
    pub total_anchor_sessions: i64,
    pub branches: Vec<JourneyBranch>,
}

pub const VALID_METRIC_TYPES: &[&str] = &[
    "page",
    "referrer",
    "country",
    "region",
    "city",
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

    async fn get_sessions(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &SessionsQuery,
    ) -> anyhow::Result<SessionsResponse>;

    async fn get_session_detail(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        session_id: &str,
    ) -> anyhow::Result<SessionDetailResponse>;

    async fn list_goals(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<Goal>>;

    async fn create_goal(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        req: CreateGoalRequest,
    ) -> anyhow::Result<Goal>;

    async fn update_goal(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        goal_id: &str,
        req: UpdateGoalRequest,
    ) -> anyhow::Result<Goal>;

    async fn delete_goal(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        goal_id: &str,
    ) -> anyhow::Result<()>;

    async fn get_goal_stats(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        goal_id: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<GoalStats>;

    async fn count_goals(&self, website_id: &str, tenant_id: Option<&str>) -> anyhow::Result<i64>;

    async fn goal_name_exists(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        name: &str,
        exclude_goal_id: Option<&str>,
    ) -> anyhow::Result<bool>;

    async fn list_funnels(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<FunnelSummary>>;

    async fn get_funnel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        funnel_id: &str,
    ) -> anyhow::Result<Option<Funnel>>;

    async fn create_funnel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        req: CreateFunnelRequest,
    ) -> anyhow::Result<Funnel>;

    async fn update_funnel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        funnel_id: &str,
        req: UpdateFunnelRequest,
    ) -> anyhow::Result<Option<Funnel>>;

    async fn delete_funnel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        funnel_id: &str,
    ) -> anyhow::Result<bool>;

    async fn get_funnel_results(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        funnel_id: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<FunnelResults>;

    async fn get_journey(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &JourneyQuery,
    ) -> anyhow::Result<JourneyResponse>;
}
