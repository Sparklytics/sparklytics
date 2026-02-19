use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use sparklytics_core::{
    analytics::{AnalyticsBackend, AnalyticsFilter},
    billing::{BillingGate, BillingOutcome, NullBillingGate},
    event::Event,
};
use sparklytics_duckdb::DuckDbBackend;

fn base_filter(start_date: NaiveDate, end_date: NaiveDate) -> AnalyticsFilter {
    AnalyticsFilter {
        start_date,
        end_date,
        timezone: None,
        filter_country: None,
        filter_page: None,
        filter_referrer: None,
        filter_browser: None,
        filter_os: None,
        filter_device: None,
        filter_language: None,
        filter_utm_source: None,
        filter_utm_medium: None,
        filter_utm_campaign: None,
    }
}

fn sample_event(website_id: &str, session_id: String) -> Event {
    Event {
        id: uuid::Uuid::new_v4().to_string(),
        website_id: website_id.to_string(),
        tenant_id: None,
        session_id,
        visitor_id: "visitor_1".to_string(),
        event_type: "pageview".to_string(),
        url: "/".to_string(),
        referrer_url: Some("https://google.com".to_string()),
        referrer_domain: Some("google.com".to_string()),
        event_name: None,
        event_data: None,
        country: Some("PL".to_string()),
        region: None,
        city: None,
        browser: Some("Chrome".to_string()),
        browser_version: None,
        os: Some("macOS".to_string()),
        os_version: None,
        device_type: Some("desktop".to_string()),
        screen: None,
        language: Some("pl".to_string()),
        utm_source: Some("newsletter".to_string()),
        utm_medium: Some("email".to_string()),
        utm_campaign: Some("launch".to_string()),
        utm_term: None,
        utm_content: None,
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_analytics_backend_dyn_dispatch() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let backend: Arc<dyn AnalyticsBackend> = db.clone();
    let today = Utc::now().date_naive();
    let filter = base_filter(today - chrono::Duration::days(1), today);

    let stats = backend
        .get_stats("site_1", None, &filter)
        .await
        .expect("stats");
    assert_eq!(stats.timezone, "UTC");
}

#[tokio::test]
async fn test_analytics_filter_all_dimensions_accepted() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let today = Utc::now().date_naive();
    let mut filter = base_filter(today - chrono::Duration::days(1), today);
    filter.filter_country = Some("PL".to_string());
    filter.filter_page = Some("/".to_string());
    filter.filter_referrer = Some("google.com".to_string());
    filter.filter_browser = Some("Chrome".to_string());
    filter.filter_os = Some("macOS".to_string());
    filter.filter_device = Some("desktop".to_string());
    filter.filter_language = Some("pl".to_string());
    filter.filter_utm_source = Some("newsletter".to_string());
    filter.filter_utm_medium = Some("email".to_string());
    filter.filter_utm_campaign = Some("launch".to_string());

    backend
        .get_stats("site_1", None, &filter)
        .await
        .expect("stats");
    backend
        .get_timeseries("site_1", None, &filter, None)
        .await
        .expect("timeseries");
    backend
        .get_metrics("site_1", None, "page", 10, 0, &filter)
        .await
        .expect("metrics");
}

#[tokio::test]
async fn test_timeseries_all_filters_accepted() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let today = Utc::now().date_naive();
    let mut filter = base_filter(today - chrono::Duration::days(1), today);
    filter.filter_referrer = Some("google.com".to_string());
    filter.filter_browser = Some("Chrome".to_string());
    filter.filter_os = Some("macOS".to_string());

    backend
        .get_timeseries("site_1", None, &filter, None)
        .await
        .expect("timeseries");
}

#[tokio::test]
async fn test_realtime_includes_pagination() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let backend: Arc<dyn AnalyticsBackend> = db.clone();
    let session_id = backend
        .get_or_create_session("site_1", "visitor_1", None, "/")
        .await
        .expect("session");
    backend
        .insert_events(&[sample_event("site_1", session_id)])
        .await
        .expect("insert");

    let realtime = backend
        .get_realtime("site_1", None)
        .await
        .expect("realtime");
    assert_eq!(realtime.pagination.limit, 100);
    assert!(realtime.pagination.total_in_window >= 0);
}

#[tokio::test]
async fn test_stats_result_includes_timezone() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    {
        let conn = db.conn_for_test().await;
        conn.execute(
            "UPDATE websites SET timezone = ?1 WHERE id = ?2",
            duckdb::params!["Europe/Warsaw", "site_1"],
        )
        .expect("update timezone");
    }

    let backend: Arc<dyn AnalyticsBackend> = db.clone();
    let today = Utc::now().date_naive();
    let filter = base_filter(today - chrono::Duration::days(1), today);

    let stats = backend
        .get_stats("site_1", None, &filter)
        .await
        .expect("stats");
    assert_eq!(stats.timezone, "Europe/Warsaw");
}

#[tokio::test]
async fn test_session_rename_no_infinite_recursion() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let session = backend
        .get_or_create_session("site_1", "visitor_1", None, "/")
        .await
        .expect("session");
    assert!(!session.is_empty());
}

#[tokio::test]
async fn test_billing_gate_moved_to_core() {
    let gate = NullBillingGate;
    assert_eq!(gate.check("org_any").await, BillingOutcome::Allowed);
}
