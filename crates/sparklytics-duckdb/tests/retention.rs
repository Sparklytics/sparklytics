use chrono::NaiveDate;

use sparklytics_core::analytics::{
    AnalyticsBackend, AnalyticsFilter, RetentionGranularity, RetentionQuery,
};
use sparklytics_duckdb::DuckDbBackend;

fn base_filter(start: NaiveDate, end: NaiveDate) -> AnalyticsFilter {
    AnalyticsFilter {
        start_date: start,
        end_date: end,
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

async fn insert_session(
    db: &DuckDbBackend,
    website_id: &str,
    session_id: &str,
    visitor_id: &str,
    first_seen: &str,
) {
    let conn = db.conn_for_test().await;
    conn.execute(
        r#"
        INSERT INTO sessions (
            session_id, website_id, tenant_id, visitor_id, first_seen, last_seen, pageview_count, entry_page
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, ?4, 1, '/'
        )
        "#,
        duckdb::params![session_id, website_id, visitor_id, first_seen],
    )
    .expect("insert session");
}

async fn insert_event(
    db: &DuckDbBackend,
    website_id: &str,
    event_id: &str,
    session_id: &str,
    visitor_id: &str,
    country: &str,
    created_at: &str,
) {
    let conn = db.conn_for_test().await;
    conn.execute(
        r#"
        INSERT INTO events (
            id, website_id, tenant_id, session_id, visitor_id, event_type, url,
            referrer_url, referrer_domain, event_name, event_data, country, region, city,
            browser, browser_version, os, os_version, device_type, screen, language,
            utm_source, utm_medium, utm_campaign, utm_term, utm_content, created_at
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, 'pageview', '/pricing',
            NULL, NULL, NULL, NULL, ?5, NULL, NULL,
            'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
            NULL, NULL, NULL, NULL, NULL, ?6
        )
        "#,
        duckdb::params![event_id, website_id, session_id, visitor_id, country, created_at],
    )
    .expect("insert event");
}

#[tokio::test]
async fn retention_week0_is_100_and_missing_offsets_are_zero_filled() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(&db, "site_1", "sess_a0", "visitor_a", "2026-01-01 10:00:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_a0",
        "sess_a0",
        "visitor_a",
        "PL",
        "2026-01-01 10:00:00",
    )
    .await;
    insert_session(&db, "site_1", "sess_a1", "visitor_a", "2026-01-08 10:00:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_a1",
        "sess_a1",
        "visitor_a",
        "PL",
        "2026-01-08 10:00:00",
    )
    .await;

    insert_session(&db, "site_1", "sess_b0", "visitor_b", "2026-01-01 12:00:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_b0",
        "sess_b0",
        "visitor_b",
        "US",
        "2026-01-01 12:00:00",
    )
    .await;

    let result = db
        .get_retention(
            "site_1",
            None,
            &base_filter(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 1, 31).expect("valid"),
            ),
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 4,
            },
        )
        .await
        .expect("retention");

    assert_eq!(result.max_periods, 4);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].cohort_size, 2);
    assert_eq!(result.rows[0].periods.len(), 4);
    assert_eq!(result.rows[0].periods[0].retained, 2);
    assert!((result.rows[0].periods[0].rate - 1.0).abs() < f64::EPSILON);
    assert_eq!(result.rows[0].periods[1].retained, 1);
    assert_eq!(result.rows[0].periods[2].retained, 0);
    assert_eq!(result.rows[0].periods[3].retained, 0);
}

#[tokio::test]
async fn retention_respects_country_filter_for_cohort_population() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(
        &db,
        "site_1",
        "sess_pl",
        "visitor_pl",
        "2026-01-03 10:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_pl",
        "sess_pl",
        "visitor_pl",
        "PL",
        "2026-01-03 10:00:00",
    )
    .await;

    insert_session(
        &db,
        "site_1",
        "sess_us",
        "visitor_us",
        "2026-01-03 11:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_us",
        "sess_us",
        "visitor_us",
        "US",
        "2026-01-03 11:00:00",
    )
    .await;

    let mut filter = base_filter(
        NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid"),
        NaiveDate::from_ymd_opt(2026, 1, 10).expect("valid"),
    );
    filter.filter_country = Some("PL".to_string());

    let result = db
        .get_retention(
            "site_1",
            None,
            &filter,
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 4,
            },
        )
        .await
        .expect("retention");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].cohort_size, 1);
    assert_eq!(result.rows[0].periods[0].retained, 1);
}

#[tokio::test]
async fn retention_uses_events_fallback_when_session_row_is_missing() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(
        &db,
        "site_1",
        "sess_with_session",
        "visitor_with_session",
        "2026-01-01 10:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_with_session",
        "sess_with_session",
        "visitor_with_session",
        "PL",
        "2026-01-01 10:00:00",
    )
    .await;

    // No session row for this visitor: retention should still include them via events fallback.
    insert_event(
        &db,
        "site_1",
        "evt_event_only",
        "sess_missing",
        "visitor_event_only",
        "PL",
        "2026-01-02 11:00:00",
    )
    .await;

    let result = db
        .get_retention(
            "site_1",
            None,
            &base_filter(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 1, 31).expect("valid"),
            ),
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 2,
            },
        )
        .await
        .expect("retention");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].cohort_size, 2);
    assert_eq!(result.rows[0].periods[0].retained, 2);
}

#[tokio::test]
async fn retention_timezone_changes_cohort_day_bucket() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(&db, "site_1", "sess_1", "visitor_1", "2026-01-05 23:30:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_1",
        "visitor_1",
        "PL",
        "2026-01-05 23:30:00",
    )
    .await;

    let mut warsaw = base_filter(
        NaiveDate::from_ymd_opt(2026, 1, 6).expect("valid"),
        NaiveDate::from_ymd_opt(2026, 1, 6).expect("valid"),
    );
    warsaw.timezone = Some("Europe/Warsaw".to_string());

    let warsaw_result = db
        .get_retention(
            "site_1",
            None,
            &warsaw,
            &RetentionQuery {
                granularity: RetentionGranularity::Day,
                max_periods: 2,
            },
        )
        .await
        .expect("warsaw retention");

    assert_eq!(warsaw_result.rows.len(), 1);
    assert!(
        warsaw_result.rows[0].cohort_start.contains("2026-01-06"),
        "unexpected warsaw cohort_start: {}",
        warsaw_result.rows[0].cohort_start
    );

    let mut utc = base_filter(
        NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid"),
        NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid"),
    );
    utc.timezone = Some("UTC".to_string());

    let utc_result = db
        .get_retention(
            "site_1",
            None,
            &utc,
            &RetentionQuery {
                granularity: RetentionGranularity::Day,
                max_periods: 2,
            },
        )
        .await
        .expect("utc retention");

    assert_eq!(utc_result.rows.len(), 1);
    assert!(utc_result.rows[0].cohort_start.contains("2026-01-05"));
}

#[tokio::test]
async fn retention_empty_range_returns_empty_rows_and_zero_summary() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    let result = db
        .get_retention(
            "site_1",
            None,
            &base_filter(
                NaiveDate::from_ymd_opt(2026, 2, 1).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 2, 2).expect("valid"),
            ),
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 50,
            },
        )
        .await
        .expect("retention");

    assert!(result.rows.is_empty());
    assert_eq!(result.max_periods, 12);
    assert_eq!(result.summary.avg_period1_rate, 0.0);
    assert_eq!(result.summary.avg_period4_rate, Some(0.0));
}

#[tokio::test]
async fn retention_summary_ignores_not_elapsed_periods() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(
        &db,
        "site_1",
        "sess_old_0",
        "visitor_old",
        "2026-01-01 10:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_old_0",
        "sess_old_0",
        "visitor_old",
        "PL",
        "2026-01-01 10:00:00",
    )
    .await;
    insert_session(
        &db,
        "site_1",
        "sess_old_1",
        "visitor_old",
        "2026-01-08 10:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_old_1",
        "sess_old_1",
        "visitor_old",
        "PL",
        "2026-01-08 10:00:00",
    )
    .await;

    insert_session(
        &db,
        "site_1",
        "sess_new_0",
        "visitor_new",
        "2026-01-29 10:00:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_new_0",
        "sess_new_0",
        "visitor_new",
        "PL",
        "2026-01-29 10:00:00",
    )
    .await;

    let result = db
        .get_retention(
            "site_1",
            None,
            &base_filter(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 1, 31).expect("valid"),
            ),
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 5,
            },
        )
        .await
        .expect("retention");

    assert_eq!(result.rows.len(), 2);
    assert!(
        (result.summary.avg_period1_rate - 1.0).abs() < f64::EPSILON,
        "unexpected avg_period1_rate: {}",
        result.summary.avg_period1_rate
    );
    assert_eq!(result.summary.avg_period4_rate, Some(0.0));
}

#[tokio::test]
async fn retention_counts_activity_beyond_cohort_end_within_extended_window() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed website");

    insert_session(&db, "site_1", "sess_0", "visitor_1", "2026-01-01 09:00:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_0",
        "sess_0",
        "visitor_1",
        "PL",
        "2026-01-01 09:00:00",
    )
    .await;

    insert_session(&db, "site_1", "sess_2", "visitor_1", "2026-01-15 09:00:00").await;
    insert_event(
        &db,
        "site_1",
        "evt_2",
        "sess_2",
        "visitor_1",
        "PL",
        "2026-01-15 09:00:00",
    )
    .await;

    let result = db
        .get_retention(
            "site_1",
            None,
            &base_filter(
                NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 1, 7).expect("valid"),
            ),
            &RetentionQuery {
                granularity: RetentionGranularity::Week,
                max_periods: 4,
            },
        )
        .await
        .expect("retention");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].periods[2].retained, 1);
}
