use async_trait::async_trait;
use chrono::NaiveDate;

use sparklytics_core::analytics::{
    AnalyticsBackend, AnalyticsFilter, ExportRow, MetricRow, MetricsPage, RealtimeEvent,
    RealtimePagination, RealtimeResult, StatsResult, TimeseriesResult,
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
        crate::session::get_or_create_session_inner(self, visitor_id, website_id, url, Utc::now())
            .await
            .map(|r| r.session_id)
    }

    async fn get_stats(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<StatsResult> {
        let params = crate::queries::stats::StatsParams::from_filter(website_id, filter);
        crate::queries::stats::get_stats_inner(self, &params).await
    }

    async fn get_timeseries(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        filter: &AnalyticsFilter,
        granularity: Option<&str>,
    ) -> anyhow::Result<TimeseriesResult> {
        crate::queries::timeseries::get_timeseries_inner(self, website_id, filter, granularity).await
    }

    async fn get_metrics(
        &self,
        website_id: &str,
        _tenant_id: Option<&str>,
        metric_type: &str,
        limit: i64,
        offset: i64,
        filter: &AnalyticsFilter,
    ) -> anyhow::Result<MetricsPage> {
        let (result, pagination) =
            crate::queries::metrics::get_metrics_inner(self, website_id, metric_type, limit, offset, filter)
                .await?;
        Ok(MetricsPage {
            rows: result
                .rows
                .into_iter()
                .map(|r| MetricRow {
                    value: r.value,
                    visitors: r.visitors,
                    pageviews: r.pageviews,
                })
                .collect(),
            total: pagination.total,
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
}
