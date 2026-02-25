use async_trait::async_trait;
use chrono::NaiveDate;

use sparklytics_core::analytics::{
    AnalyticsBackend, AnalyticsFilter, AttributionQuery, AttributionResponse, CampaignLink,
    ComparisonRange, CreateCampaignLinkRequest, CreateFunnelRequest, CreateGoalRequest,
    CreateReportRequest, CreateTrackingPixelRequest, EventNamesResult, EventPropertiesResult,
    ExportRow, Funnel, FunnelResults, FunnelSummary, Goal, GoalStats, JourneyQuery,
    JourneyResponse, LinkStatsResponse, MetricRow, MetricsPage, PixelStatsResponse,
    RealtimeEvent, RealtimePagination, RealtimeResult, RetentionQuery, RetentionResponse,
    RevenueSummary, SavedReport, SavedReportSummary, SessionDetailResponse, SessionsQuery,
    SessionsResponse, StatsResult, TimeseriesResult, TrackingPixel, UpdateCampaignLinkRequest,
    UpdateFunnelRequest, UpdateGoalRequest, UpdateReportRequest, UpdateTrackingPixelRequest,
};
use sparklytics_core::event::Event;

use crate::DuckDbBackend;

#[async_trait]
impl AnalyticsBackend for DuckDbBackend {
    async fn insert_events(&self, events: &[Event]) -> anyhow::Result<()> {
        DuckDbBackend::insert_events(self, events).await
    }

    async fn get_or_create_session(
        &self,
        website_id: &str,
        visitor_id: &str,
        _referrer_domain: Option<&str>,
        url: &str,
    ) -> anyhow::Result<String> {
        use chrono::Utc;
        self.get_or_create_session_at(website_id, visitor_id, url, Utc::now())
            .await
    }

    async fn get_stats(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        comparison: Option<&ComparisonRange>,
    ) -> anyhow::Result<StatsResult> {
        let params =
            crate::queries::stats::StatsParams::from_filter(website_id, filter, comparison);
        crate::queries::stats::get_stats_inner(self, &params).await
    }

    async fn get_timeseries(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
        comparison: Option<&ComparisonRange>,
    ) -> anyhow::Result<TimeseriesResult> {
        crate::queries::timeseries::get_timeseries_inner(
            self,
            website_id,
            filter,
            granularity,
            comparison,
        )
        .await
    }

    async fn get_metrics(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
        comparison: Option<&ComparisonRange>,
    ) -> anyhow::Result<MetricsPage> {
        let (result, pagination) = crate::queries::metrics::get_metrics_inner(
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
            rows: result
                .rows
                .into_iter()
                .map(|r| MetricRow {
                    value: r.value,
                    visitors: r.visitors,
                    pageviews: r.pageviews,
                    prev_visitors: r.prev_visitors,
                    prev_pageviews: r.prev_pageviews,
                    delta_visitors_abs: r.delta_visitors_abs,
                    delta_visitors_pct: r.delta_visitors_pct,
                    bounce_rate: r.bounce_rate,
                    avg_duration_seconds: r.avg_duration_seconds,
                })
                .collect(),
            total: pagination.total,
            compare: result.compare,
        })
    }

    async fn get_realtime(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<RealtimeResult> {
        let r = crate::queries::realtime::get_realtime_inner(self, website_id).await?;
        Ok(RealtimeResult {
            active_visitors: r.active_visitors,
            recent_events: r
                .recent_events
                .into_iter()
                .map(|e| RealtimeEvent {
                    event_type: e.event_type,
                    url: e.url,
                    referrer_domain: e.referrer_domain,
                    country: e.country,
                    browser: e.browser,
                    device_type: e.device_type,
                    ts: e.ts,
                })
                .collect(),
            pagination: RealtimePagination {
                limit: r.pagination.limit,
                total_in_window: r.pagination.total_in_window,
            },
        })
    }

    async fn export_events(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<ExportRow>> {
        let rows = self.export_events_raw(website_id, start, end).await?;
        Ok(rows
            .into_iter()
            .map(|r| ExportRow {
                id: r.id,
                website_id: r.website_id,
                event_type: r.event_type,
                url: r.url,
                referrer_domain: r.referrer_domain,
                event_name: r.event_name,
                country: r.country,
                browser: r.browser,
                os: r.os,
                device_type: r.device_type,
                language: r.language,
                utm_source: r.utm_source,
                utm_medium: r.utm_medium,
                utm_campaign: r.utm_campaign,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn get_event_names(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<EventNamesResult> {
        crate::queries::events::get_event_names_inner(self, website_id, filter).await
    }

    async fn get_event_properties(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        event_name: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<EventPropertiesResult> {
        crate::queries::events::get_event_properties_inner(self, website_id, event_name, filter)
            .await
    }

    async fn get_event_timeseries(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        event_name: &str,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
    ) -> anyhow::Result<TimeseriesResult> {
        crate::queries::events::get_event_timeseries_inner(
            self,
            website_id,
            event_name,
            filter,
            granularity,
        )
        .await
    }

    async fn get_sessions(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &SessionsQuery,
    ) -> anyhow::Result<SessionsResponse> {
        crate::queries::sessions::get_sessions_inner(self, website_id, filter, query).await
    }

    async fn get_session_detail(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        session_id: &str,
    ) -> anyhow::Result<SessionDetailResponse> {
        crate::queries::session_detail::get_session_detail_inner(self, website_id, session_id).await
    }

    async fn list_goals(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<Goal>> {
        crate::queries::goals::list_goals_inner(self, website_id).await
    }

    async fn create_goal(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        req: CreateGoalRequest,
    ) -> anyhow::Result<Goal> {
        crate::queries::goals::create_goal_inner(self, website_id, req).await
    }

    async fn update_goal(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        goal_id: &str,
        req: UpdateGoalRequest,
    ) -> anyhow::Result<Goal> {
        crate::queries::goals::update_goal_inner(self, website_id, goal_id, req).await
    }

    async fn delete_goal(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        goal_id: &str,
    ) -> anyhow::Result<()> {
        crate::queries::goals::delete_goal_inner(self, website_id, goal_id).await
    }

    async fn get_goal_stats(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        goal_id: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<GoalStats> {
        crate::queries::goals::get_goal_stats_inner(self, website_id, goal_id, filter).await
    }

    async fn get_attribution(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &AttributionQuery,
    ) -> anyhow::Result<AttributionResponse> {
        crate::queries::attribution::get_attribution_inner(self, website_id, filter, query).await
    }

    async fn get_revenue_summary(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &AttributionQuery,
    ) -> anyhow::Result<RevenueSummary> {
        crate::queries::attribution::get_revenue_summary_inner(self, website_id, filter, query)
            .await
    }

    async fn count_goals(&self, website_id: &str, _tenant_id: Option<&str>) -> anyhow::Result<i64> {
        crate::queries::goals::count_goals_inner(self, website_id).await
    }

    async fn goal_name_exists(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        name: &str,
        exclude_goal_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        crate::queries::goals::goal_name_exists_inner(self, website_id, name, exclude_goal_id).await
    }

    async fn list_funnels(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<FunnelSummary>> {
        crate::queries::funnels::list_funnels_inner(self, website_id).await
    }

    async fn get_funnel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        funnel_id: &str,
    ) -> anyhow::Result<Option<Funnel>> {
        crate::queries::funnels::get_funnel_inner(self, website_id, funnel_id).await
    }

    async fn create_funnel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        req: CreateFunnelRequest,
    ) -> anyhow::Result<Funnel> {
        crate::queries::funnels::create_funnel_inner(self, website_id, req).await
    }

    async fn update_funnel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        funnel_id: &str,
        req: UpdateFunnelRequest,
    ) -> anyhow::Result<Option<Funnel>> {
        crate::queries::funnels::update_funnel_inner(self, website_id, funnel_id, req).await
    }

    async fn delete_funnel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        funnel_id: &str,
    ) -> anyhow::Result<bool> {
        crate::queries::funnels::delete_funnel_inner(self, website_id, funnel_id).await
    }

    async fn get_funnel_results(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        funnel_id: &str,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<FunnelResults> {
        crate::queries::funnel_results::get_funnel_results_inner(
            self, website_id, funnel_id, filter,
        )
        .await
    }

    async fn get_journey(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &JourneyQuery,
    ) -> anyhow::Result<JourneyResponse> {
        crate::queries::journey::get_journey_inner(self, website_id, filter, query).await
    }

    async fn get_retention(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        query: &RetentionQuery,
    ) -> anyhow::Result<RetentionResponse> {
        crate::queries::retention::get_retention_inner(self, website_id, filter, query).await
    }

    async fn list_campaign_links(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<CampaignLink>> {
        self.list_campaign_links_with_stats(website_id).await
    }

    async fn create_campaign_link(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        req: CreateCampaignLinkRequest,
    ) -> anyhow::Result<CampaignLink> {
        DuckDbBackend::create_campaign_link(self, website_id, req).await
    }

    async fn update_campaign_link(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        link_id: &str,
        req: UpdateCampaignLinkRequest,
    ) -> anyhow::Result<Option<CampaignLink>> {
        DuckDbBackend::update_campaign_link(self, website_id, link_id, req).await
    }

    async fn delete_campaign_link(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        link_id: &str,
    ) -> anyhow::Result<bool> {
        DuckDbBackend::delete_campaign_link(self, website_id, link_id).await
    }

    async fn get_campaign_link_stats(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        link_id: &str,
    ) -> anyhow::Result<LinkStatsResponse> {
        DuckDbBackend::get_campaign_link_stats(self, website_id, link_id).await
    }

    async fn get_campaign_link_by_slug(&self, slug: &str) -> anyhow::Result<Option<CampaignLink>> {
        DuckDbBackend::get_campaign_link_by_slug(self, slug).await
    }

    async fn list_tracking_pixels(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<TrackingPixel>> {
        DuckDbBackend::list_tracking_pixels_with_stats(self, website_id).await
    }

    async fn create_tracking_pixel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        req: CreateTrackingPixelRequest,
    ) -> anyhow::Result<TrackingPixel> {
        DuckDbBackend::create_tracking_pixel(self, website_id, req).await
    }

    async fn update_tracking_pixel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        pixel_id: &str,
        req: UpdateTrackingPixelRequest,
    ) -> anyhow::Result<Option<TrackingPixel>> {
        DuckDbBackend::update_tracking_pixel(self, website_id, pixel_id, req).await
    }

    async fn delete_tracking_pixel(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        pixel_id: &str,
    ) -> anyhow::Result<bool> {
        DuckDbBackend::delete_tracking_pixel(self, website_id, pixel_id).await
    }

    async fn get_tracking_pixel_stats(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        pixel_id: &str,
    ) -> anyhow::Result<PixelStatsResponse> {
        DuckDbBackend::get_tracking_pixel_stats(self, website_id, pixel_id).await
    }

    async fn get_tracking_pixel_by_key(
        &self,
        pixel_key: &str,
    ) -> anyhow::Result<Option<TrackingPixel>> {
        DuckDbBackend::get_tracking_pixel_by_key(self, pixel_key).await
    }

    async fn list_reports(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<Vec<SavedReportSummary>> {
        crate::queries::reports::list_reports_inner(self, website_id).await
    }

    async fn get_report(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<Option<SavedReport>> {
        crate::queries::reports::get_report_inner(self, website_id, report_id).await
    }

    async fn create_report(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        req: CreateReportRequest,
    ) -> anyhow::Result<SavedReport> {
        crate::queries::reports::create_report_inner(self, website_id, req).await
    }

    async fn update_report(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        report_id: &str,
        req: UpdateReportRequest,
    ) -> anyhow::Result<Option<SavedReport>> {
        crate::queries::reports::update_report_inner(self, website_id, report_id, req).await
    }

    async fn delete_report(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<bool> {
        crate::queries::reports::delete_report_inner(self, website_id, report_id).await
    }

    async fn count_reports(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
    ) -> anyhow::Result<i64> {
        crate::queries::reports::count_reports_inner(self, website_id).await
    }

    async fn report_name_exists(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        name: &str,
        exclude_report_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        crate::queries::reports::report_name_exists_inner(self, website_id, name, exclude_report_id)
            .await
    }

    async fn touch_report_last_run(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        report_id: &str,
    ) -> anyhow::Result<()> {
        crate::queries::reports::touch_report_last_run_inner(self, website_id, report_id).await
    }
}
