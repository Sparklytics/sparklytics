use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use sparklytics_core::{
    analytics::{AnalyticsBackend, AnalyticsFilter},
    billing::{BillingGate, NullBillingGate},
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
        filter_region: None,
        filter_city: None,
        filter_hostname: None,
        include_bots: false,
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
        link_id: None,
        pixel_id: None,
        source_ip: None,
        user_agent: None,
        is_bot: false,
        bot_score: 0,
        bot_reason: None,
        created_at: Utc::now(),
    }
}

fn sample_custom_event(
    website_id: &str,
    session_id: String,
    visitor_id: &str,
    event_name: &str,
    event_data: Option<&str>,
    created_at: chrono::DateTime<Utc>,
) -> Event {
    Event {
        id: uuid::Uuid::new_v4().to_string(),
        website_id: website_id.to_string(),
        tenant_id: None,
        session_id,
        visitor_id: visitor_id.to_string(),
        event_type: "event".to_string(),
        url: "/checkout".to_string(),
        referrer_url: None,
        referrer_domain: None,
        event_name: Some(event_name.to_string()),
        event_data: event_data.map(|v| v.to_string()),
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
        utm_source: None,
        utm_medium: None,
        utm_campaign: None,
        utm_term: None,
        utm_content: None,
        link_id: None,
        pixel_id: None,
        source_ip: None,
        user_agent: None,
        is_bot: false,
        bot_score: 0,
        bot_reason: None,
        created_at,
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
        .get_stats("site_1", None, &filter, None)
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
        .get_stats("site_1", None, &filter, None)
        .await
        .expect("stats");
    backend
        .get_timeseries("site_1", None, &filter, None, None)
        .await
        .expect("timeseries");
    backend
        .get_metrics("site_1", None, "page", 10, 0, &filter, None)
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
        .get_timeseries("site_1", None, &filter, None, None)
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
        .get_realtime("site_1", None, false)
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
        .get_stats("site_1", None, &filter, None)
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
async fn test_custom_event_names_include_previous_period_counts() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let end = Utc::now().date_naive();
    let start = end - chrono::Duration::days(1);
    let prev_day = start - chrono::Duration::days(1);

    let s1 = backend
        .get_or_create_session("site_1", "visitor_1", None, "/")
        .await
        .expect("session");
    let s2 = backend
        .get_or_create_session("site_1", "visitor_2", None, "/")
        .await
        .expect("session");
    let s3 = backend
        .get_or_create_session("site_1", "visitor_3", None, "/")
        .await
        .expect("session");

    let current_ts = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        end.and_hms_opt(12, 0, 0).expect("valid datetime"),
        Utc,
    );
    let prev_ts = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        prev_day.and_hms_opt(12, 0, 0).expect("valid datetime"),
        Utc,
    );

    backend
        .insert_events(&[
            sample_custom_event(
                "site_1",
                s1,
                "visitor_1",
                "purchase",
                Some(r#"{"plan":"pro"}"#),
                current_ts,
            ),
            sample_custom_event(
                "site_1",
                s2.clone(),
                "visitor_2",
                "purchase",
                Some(r#"{"plan":"starter"}"#),
                current_ts,
            ),
            sample_custom_event(
                "site_1",
                s2,
                "visitor_2",
                "signup",
                Some(r#"{"method":"google"}"#),
                current_ts,
            ),
            sample_custom_event(
                "site_1",
                s3,
                "visitor_3",
                "purchase",
                Some(r#"{"plan":"free"}"#),
                prev_ts,
            ),
        ])
        .await
        .expect("insert");

    let result = backend
        .get_event_names("site_1", None, &base_filter(start, end))
        .await
        .expect("event names");

    assert_eq!(result.total, 2);
    let purchase = result
        .rows
        .iter()
        .find(|row| row.event_name == "purchase")
        .expect("purchase row");
    assert_eq!(purchase.count, 2);
    assert_eq!(purchase.visitors, 2);
    assert_eq!(purchase.prev_count, Some(1));

    let signup = result
        .rows
        .iter()
        .find(|row| row.event_name == "signup")
        .expect("signup row");
    assert_eq!(signup.count, 1);
    assert_eq!(signup.prev_count, None);
}

#[tokio::test]
async fn test_custom_event_properties_extract_json_pairs() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let end = Utc::now().date_naive();
    let start = end - chrono::Duration::days(1);
    let ts = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        end.and_hms_opt(12, 0, 0).expect("valid datetime"),
        Utc,
    );

    let s1 = backend
        .get_or_create_session("site_1", "visitor_1", None, "/")
        .await
        .expect("session");
    let s2 = backend
        .get_or_create_session("site_1", "visitor_2", None, "/")
        .await
        .expect("session");

    backend
        .insert_events(&[
            sample_custom_event(
                "site_1",
                s1.clone(),
                "visitor_1",
                "purchase",
                Some(r#"{"plan":"pro","currency":"USD"}"#),
                ts,
            ),
            sample_custom_event(
                "site_1",
                s2,
                "visitor_2",
                "purchase",
                Some(r#"{"plan":"free","currency":"USD"}"#),
                ts,
            ),
            sample_custom_event("site_1", s1, "visitor_1", "purchase", None, ts),
        ])
        .await
        .expect("insert");

    let result = backend
        .get_event_properties("site_1", None, "purchase", &base_filter(start, end))
        .await
        .expect("event properties");

    assert_eq!(result.total_occurrences, 3);
    assert_eq!(result.sample_size, 3);
    assert!(result
        .properties
        .iter()
        .any(|row| row.property_key == "plan" && row.property_value == "pro" && row.count == 1));
    assert!(result.properties.iter().any(|row| {
        row.property_key == "currency" && row.property_value == "USD" && row.count == 2
    }));
}

#[tokio::test]
async fn test_custom_event_timeseries_zero_fills_buckets() {
    let db = Arc::new(DuckDbBackend::open_in_memory().expect("db"));
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");
    let backend: Arc<dyn AnalyticsBackend> = db.clone();

    let end = Utc::now().date_naive();
    let start = end - chrono::Duration::days(2);
    let mid = start + chrono::Duration::days(1);

    let s1 = backend
        .get_or_create_session("site_1", "visitor_1", None, "/")
        .await
        .expect("session");
    let s2 = backend
        .get_or_create_session("site_1", "visitor_2", None, "/")
        .await
        .expect("session");

    let start_ts = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        start.and_hms_opt(12, 0, 0).expect("valid datetime"),
        Utc,
    );
    let end_ts = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        end.and_hms_opt(12, 0, 0).expect("valid datetime"),
        Utc,
    );

    backend
        .insert_events(&[
            sample_custom_event(
                "site_1",
                s1,
                "visitor_1",
                "purchase",
                Some(r#"{"plan":"pro"}"#),
                start_ts,
            ),
            sample_custom_event(
                "site_1",
                s2,
                "visitor_2",
                "purchase",
                Some(r#"{"plan":"pro"}"#),
                end_ts,
            ),
        ])
        .await
        .expect("insert");

    let result = backend
        .get_event_timeseries(
            "site_1",
            None,
            "purchase",
            &base_filter(start, end),
            Some("day"),
        )
        .await
        .expect("event timeseries");

    assert_eq!(result.series.len(), 3);
    let total_occurrences: i64 = result.series.iter().map(|p| p.pageviews).sum();
    assert_eq!(total_occurrences, 2);
    assert!(result
        .series
        .iter()
        .any(|point| point.date == mid.format("%Y-%m-%d").to_string() && point.pageviews == 0));
}

#[tokio::test]
async fn test_billing_gate_moved_to_core() {
    let gate = NullBillingGate;
    let admission = gate.admit_events("org_any", 42).await;
    assert_eq!(admission.allowed_events, 42);
    assert!(admission.reason.is_none());
}
