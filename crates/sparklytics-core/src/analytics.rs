//! Analytics backend abstraction.

use anyhow::{anyhow, Result};
use chrono::{Months, NaiveDate};
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompareMode {
    #[default]
    None,
    PreviousPeriod,
    PreviousYear,
    Custom,
}

impl CompareMode {
    pub fn parse(raw: Option<&str>) -> Result<Self> {
        match raw.map(str::trim) {
            None | Some("") | Some("none") => Ok(Self::None),
            Some("previous_period") => Ok(Self::PreviousPeriod),
            Some("previous_year") => Ok(Self::PreviousYear),
            Some("custom") => Ok(Self::Custom),
            Some(_) => Err(anyhow!(
                "compare_mode must be one of: none, previous_period, previous_year, custom"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonRange {
    pub mode: CompareMode,
    pub primary_start: NaiveDate,
    pub primary_end: NaiveDate,
    pub comparison_start: NaiveDate,
    pub comparison_end: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonMetadata {
    pub mode: CompareMode,
    pub primary_range: [String; 2],
    pub comparison_range: [String; 2],
}

impl ComparisonRange {
    pub fn to_metadata(&self) -> ComparisonMetadata {
        ComparisonMetadata {
            mode: self.mode.clone(),
            primary_range: [self.primary_start.to_string(), self.primary_end.to_string()],
            comparison_range: [
                self.comparison_start.to_string(),
                self.comparison_end.to_string(),
            ],
        }
    }
}

pub fn resolve_comparison_range(
    primary_start: NaiveDate,
    primary_end: NaiveDate,
    mode: CompareMode,
    compare_start: Option<NaiveDate>,
    compare_end: Option<NaiveDate>,
) -> Result<Option<ComparisonRange>> {
    if primary_end < primary_start {
        return Err(anyhow!("end_date must be on or after start_date"));
    }

    if matches!(mode, CompareMode::None) {
        return Ok(None);
    }

    let primary_days = (primary_end - primary_start).num_days() + 1;
    let (comparison_start, comparison_end) = match mode {
        CompareMode::PreviousPeriod => {
            let end = primary_start - chrono::Duration::days(1);
            let start = end - chrono::Duration::days(primary_days - 1);
            (start, end)
        }
        CompareMode::PreviousYear => {
            let start = primary_start
                .checked_sub_months(Months::new(12))
                .ok_or_else(|| anyhow!("previous_year comparison_start out of range"))?;
            let end = primary_end
                .checked_sub_months(Months::new(12))
                .ok_or_else(|| anyhow!("previous_year comparison_end out of range"))?;
            (start, end)
        }
        CompareMode::Custom => {
            let start = compare_start
                .ok_or_else(|| anyhow!("compare_start_date is required for custom compare"))?;
            let end = compare_end
                .ok_or_else(|| anyhow!("compare_end_date is required for custom compare"))?;
            if end < start {
                return Err(anyhow!(
                    "compare_end_date must be on or after compare_start_date"
                ));
            }
            (start, end)
        }
        CompareMode::None => unreachable!(),
    };

    let compare_days = (comparison_end - comparison_start).num_days() + 1;
    if compare_days > primary_days * 2 {
        return Err(anyhow!(
            "comparison range cannot exceed primary range duration x 2"
        ));
    }

    Ok(Some(ComparisonRange {
        mode,
        primary_start,
        primary_end,
        comparison_start,
        comparison_end,
    }))
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<ComparisonMetadata>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare_series: Option<Vec<TimeseriesPoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<ComparisonMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricRow {
    pub value: String,
    pub visitors: i64,
    /// Always populated as of Sprint 12 (was optional for non-page types before).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pageviews: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_visitors: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_pageviews: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_visitors_abs: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_visitors_pct: Option<f64>,
    /// Fraction of sessions that had ≤ 1 pageview, 0–100.
    pub bounce_rate: f64,
    /// Mean session duration in seconds (0.0 when all sessions are single-event).
    pub avg_duration_seconds: f64,
}

#[derive(Debug, Clone)]
pub struct MetricsPage {
    pub rows: Vec<MetricRow>,
    pub total: i64,
    pub compare: Option<ComparisonMetadata>,
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
pub enum GoalValueMode {
    #[default]
    None,
    Fixed,
    EventProperty,
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
    pub value_mode: GoalValueMode,
    pub fixed_value: Option<f64>,
    pub value_property_key: Option<String>,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGoalRequest {
    pub name: String,
    pub goal_type: GoalType,
    pub match_value: String,
    pub match_operator: Option<MatchOperator>,
    pub value_mode: Option<GoalValueMode>,
    pub fixed_value: Option<f64>,
    pub value_property_key: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGoalRequest {
    pub name: Option<String>,
    pub match_value: Option<String>,
    pub match_operator: Option<MatchOperator>,
    pub value_mode: Option<GoalValueMode>,
    pub fixed_value: Option<f64>,
    pub value_property_key: Option<String>,
    pub currency: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttributionModel {
    FirstTouch,
    #[default]
    LastTouch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionQuery {
    pub goal_id: String,
    pub model: AttributionModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionRow {
    pub channel: String,
    pub conversions: i64,
    pub revenue: f64,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionTotals {
    pub conversions: i64,
    pub revenue: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionResponse {
    pub goal_id: String,
    pub model: AttributionModel,
    pub rows: Vec<AttributionRow>,
    pub totals: AttributionTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSummary {
    pub goal_id: String,
    pub model: AttributionModel,
    pub conversions: i64,
    pub revenue: f64,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RetentionGranularity {
    Day,
    #[default]
    Week,
    Month,
}

#[derive(Debug, Clone)]
pub struct RetentionQuery {
    pub granularity: RetentionGranularity,
    pub max_periods: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPeriod {
    pub offset: u32,
    pub retained: i64,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionCohortRow {
    pub cohort_start: String,
    pub cohort_size: i64,
    pub periods: Vec<RetentionPeriod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionSummary {
    pub avg_period1_rate: f64,
    pub avg_period4_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionResponse {
    pub granularity: RetentionGranularity,
    pub max_periods: u32,
    pub rows: Vec<RetentionCohortRow>,
    pub summary: RetentionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignLink {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub slug: String,
    pub destination_url: String,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clicks: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_visitors: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversions: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revenue: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignLinkRequest {
    pub name: String,
    pub destination_url: String,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCampaignLinkRequest {
    pub name: Option<String>,
    pub destination_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub utm_source: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub utm_medium: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub utm_campaign: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub utm_term: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub utm_content: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkStatsResponse {
    pub link_id: String,
    pub clicks: i64,
    pub unique_visitors: i64,
    pub conversions: i64,
    pub revenue: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingPixel {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub pixel_key: String,
    pub default_url: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub views: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unique_visitors: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrackingPixelRequest {
    pub name: String,
    pub default_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTrackingPixelRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub default_url: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelStatsResponse {
    pub pixel_id: String,
    pub views: i64,
    pub unique_visitors: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionSchedule {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Email,
    Webhook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertMetric {
    Pageviews,
    Visitors,
    Conversions,
    ConversionRate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertConditionType {
    Spike,
    Drop,
    ThresholdAbove,
    ThresholdBelow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSourceType {
    Subscription,
    Alert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationDeliveryStatus {
    Sent,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSubscription {
    pub id: String,
    pub website_id: String,
    pub report_id: String,
    pub schedule: SubscriptionSchedule,
    pub timezone: String,
    pub channel: NotificationChannel,
    pub target: String,
    pub is_active: bool,
    pub last_run_at: Option<String>,
    pub next_run_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReportSubscriptionRequest {
    pub report_id: String,
    pub schedule: SubscriptionSchedule,
    pub timezone: Option<String>,
    pub channel: NotificationChannel,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateReportSubscriptionRequest {
    pub report_id: Option<String>,
    pub schedule: Option<SubscriptionSchedule>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub timezone: Option<Option<String>>,
    pub channel: Option<NotificationChannel>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub target: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub metric: AlertMetric,
    pub condition_type: AlertConditionType,
    pub threshold_value: f64,
    pub lookback_days: i64,
    pub channel: NotificationChannel,
    pub target: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlertRuleRequest {
    pub name: String,
    pub metric: AlertMetric,
    pub condition_type: AlertConditionType,
    pub threshold_value: f64,
    pub lookback_days: Option<i64>,
    pub channel: NotificationChannel,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAlertRuleRequest {
    pub name: Option<String>,
    pub metric: Option<AlertMetric>,
    pub condition_type: Option<AlertConditionType>,
    pub threshold_value: Option<f64>,
    pub lookback_days: Option<i64>,
    pub channel: Option<NotificationChannel>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub target: Option<Option<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvaluationResult {
    pub alert_id: String,
    pub triggered: bool,
    pub metric_value: f64,
    pub baseline_mean: Option<f64>,
    pub baseline_stddev: Option<f64>,
    pub z_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationDelivery {
    pub id: String,
    pub source_type: NotificationSourceType,
    pub source_id: String,
    pub idempotency_key: String,
    pub status: NotificationDeliveryStatus,
    pub error_message: Option<String>,
    pub delivered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPayload {
    pub website_id: String,
    pub report_id: String,
    pub generated_at: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReportType {
    #[default]
    Stats,
    Pageviews,
    Metrics,
    Events,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DateRangeType {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    pub version: u32,
    pub report_type: ReportType,
    pub date_range_type: DateRangeType,
    pub relative_days: Option<u32>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub compare_mode: Option<CompareMode>,
    pub compare_start_date: Option<String>,
    pub compare_end_date: Option<String>,
    pub timezone: Option<String>,
    pub metric_type: Option<String>,
    pub filter_country: Option<String>,
    pub filter_browser: Option<String>,
    pub filter_os: Option<String>,
    pub filter_device: Option<String>,
    pub filter_page: Option<String>,
    pub filter_referrer: Option<String>,
    pub filter_utm_source: Option<String>,
    pub filter_utm_medium: Option<String>,
    pub filter_utm_campaign: Option<String>,
    pub filter_region: Option<String>,
    pub filter_city: Option<String>,
    pub filter_hostname: Option<String>,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            version: 1,
            report_type: ReportType::Stats,
            date_range_type: DateRangeType::Relative,
            relative_days: Some(30),
            start_date: None,
            end_date: None,
            compare_mode: None,
            compare_start_date: None,
            compare_end_date: None,
            timezone: Some("UTC".to_string()),
            metric_type: None,
            filter_country: None,
            filter_browser: None,
            filter_os: None,
            filter_device: None,
            filter_page: None,
            filter_referrer: None,
            filter_utm_source: None,
            filter_utm_medium: None,
            filter_utm_campaign: None,
            filter_region: None,
            filter_city: None,
            filter_hostname: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedReport {
    pub id: String,
    pub website_id: String,
    pub name: String,
    pub description: Option<String>,
    pub config: ReportConfig,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedReportSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub report_type: ReportType,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReportRequest {
    pub name: String,
    pub description: Option<String>,
    pub config: ReportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReportRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub description: Option<Option<String>>,
    pub config: Option<ReportConfig>,
}

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRunResult {
    pub report_id: Option<String>,
    pub config: ReportConfig,
    pub ran_at: String,
    pub data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_year_uses_calendar_alignment() {
        let start = NaiveDate::from_ymd_opt(2025, 3, 1).expect("valid date");
        let end = NaiveDate::from_ymd_opt(2025, 3, 31).expect("valid date");

        let range = resolve_comparison_range(start, end, CompareMode::PreviousYear, None, None)
            .expect("comparison should resolve")
            .expect("comparison range should exist");

        assert_eq!(
            range.comparison_start,
            NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid date")
        );
        assert_eq!(
            range.comparison_end,
            NaiveDate::from_ymd_opt(2024, 3, 31).expect("valid date")
        );
    }

    #[test]
    fn previous_year_clamps_leap_day_to_feb_28() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).expect("valid date");

        let range = resolve_comparison_range(date, date, CompareMode::PreviousYear, None, None)
            .expect("comparison should resolve")
            .expect("comparison range should exist");

        assert_eq!(
            range.comparison_start,
            NaiveDate::from_ymd_opt(2023, 2, 28).expect("valid date")
        );
        assert_eq!(
            range.comparison_end,
            NaiveDate::from_ymd_opt(2023, 2, 28).expect("valid date")
        );
    }
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
#[allow(clippy::too_many_arguments)]
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
        comparison: Option<&ComparisonRange>,
    ) -> anyhow::Result<StatsResult>;

    async fn get_timeseries(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
        comparison: Option<&ComparisonRange>,
    ) -> anyhow::Result<TimeseriesResult>;

    async fn get_metrics(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
        comparison: Option<&ComparisonRange>,
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

    async fn get_attribution(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &AttributionQuery,
    ) -> anyhow::Result<AttributionResponse>;

    async fn get_revenue_summary(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &AttributionQuery,
    ) -> anyhow::Result<RevenueSummary>;

    async fn evaluate_alert_rules(
        &self,
        tenant_id: Option<&str>,
        website_id: &str,
    ) -> anyhow::Result<Vec<AlertEvaluationResult>>;

    async fn render_report_payload(
        &self,
        tenant_id: Option<&str>,
        website_id: &str,
        report_id: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<ReportPayload>;

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

    async fn get_retention(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &RetentionQuery,
    ) -> anyhow::Result<RetentionResponse>;

    async fn list_campaign_links(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<CampaignLink>>;

    async fn create_campaign_link(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        req: CreateCampaignLinkRequest,
    ) -> anyhow::Result<CampaignLink>;

    async fn update_campaign_link(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        link_id: &str,
        req: UpdateCampaignLinkRequest,
    ) -> anyhow::Result<Option<CampaignLink>>;

    async fn delete_campaign_link(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        link_id: &str,
    ) -> anyhow::Result<bool>;

    async fn get_campaign_link_stats(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        link_id: &str,
    ) -> anyhow::Result<LinkStatsResponse>;

    async fn get_campaign_link_by_slug(&self, slug: &str) -> anyhow::Result<Option<CampaignLink>>;

    async fn list_tracking_pixels(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<TrackingPixel>>;

    async fn create_tracking_pixel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        req: CreateTrackingPixelRequest,
    ) -> anyhow::Result<TrackingPixel>;

    async fn update_tracking_pixel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        pixel_id: &str,
        req: UpdateTrackingPixelRequest,
    ) -> anyhow::Result<Option<TrackingPixel>>;

    async fn delete_tracking_pixel(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        pixel_id: &str,
    ) -> anyhow::Result<bool>;

    async fn get_tracking_pixel_stats(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        pixel_id: &str,
    ) -> anyhow::Result<PixelStatsResponse>;

    async fn get_tracking_pixel_by_key(
        &self,
        pixel_key: &str,
    ) -> anyhow::Result<Option<TrackingPixel>>;

    async fn list_reports(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<SavedReportSummary>>;

    async fn get_report(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<Option<SavedReport>>;

    async fn create_report(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        req: CreateReportRequest,
    ) -> anyhow::Result<SavedReport>;

    async fn update_report(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        report_id: &str,
        req: UpdateReportRequest,
    ) -> anyhow::Result<Option<SavedReport>>;

    async fn delete_report(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<bool>;

    async fn count_reports(&self, website_id: &str, tenant_id: Option<&str>)
        -> anyhow::Result<i64>;

    async fn report_name_exists(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        name: &str,
        exclude_report_id: Option<&str>,
    ) -> anyhow::Result<bool>;

    async fn touch_report_last_run(
        &self,
        website_id: &str,
        tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<()>;
}
