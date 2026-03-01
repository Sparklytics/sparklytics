use chrono::NaiveDate;

use sparklytics_core::analytics::{
    AnalyticsBackend, AnalyticsFilter, AnchorType, JourneyDirection, JourneyQuery,
};
use sparklytics_duckdb::DuckDbBackend;

fn base_filter(day: NaiveDate) -> AnalyticsFilter {
    AnalyticsFilter {
        start_date: day,
        end_date: day,
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

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    db: &DuckDbBackend,
    website_id: &str,
    event_id: &str,
    session_id: &str,
    event_type: &str,
    url: &str,
    event_name: Option<&str>,
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
            ?1, ?2, NULL, ?3, ?4, ?5, ?6,
            NULL, NULL, ?7, NULL, ?8, NULL, NULL,
            'Chrome', NULL, 'macOS', NULL, 'desktop', NULL, 'en-US',
            NULL, NULL, NULL, NULL, NULL, ?9
        )
        "#,
        duckdb::params![
            event_id,
            website_id,
            session_id,
            format!("visitor_{session_id}"),
            event_type,
            url,
            event_name,
            country,
            created_at
        ],
    )
    .expect("insert event");
}

#[tokio::test]
async fn journey_next_includes_normalized_anchor_and_no_step_branch() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();

    // Session 1: /pricing -> /signup -> signup_clicked
    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_1",
        "pageview",
        "/pricing/?utm=ad#hero",
        None,
        "US",
        &format!("{} 10:00:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_2",
        "sess_1",
        "pageview",
        "/signup",
        None,
        "US",
        &format!("{} 10:01:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_3",
        "sess_1",
        "event",
        "/signup",
        Some("signup_clicked"),
        "US",
        &format!("{} 10:02:00", day),
    )
    .await;

    // Session 2: /pricing with no next step.
    insert_event(
        &db,
        "site_1",
        "evt_4",
        "sess_2",
        "pageview",
        "/pricing",
        None,
        "US",
        &format!("{} 11:00:00", day),
    )
    .await;

    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/Pricing/?a=1#x".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 2,
    };

    let result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey");

    assert_eq!(result.anchor.value, "/pricing");
    assert_eq!(result.total_anchor_sessions, 2);
    assert_eq!(result.branches.len(), 2);
    assert!(result.branches.iter().any(|branch| branch.nodes.is_empty()));
    assert!(result
        .branches
        .iter()
        .any(|branch| branch.nodes == vec!["/signup".to_string(), "signup_clicked".to_string()]));
}

#[tokio::test]
async fn journey_previous_orders_nodes_chronologically() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();

    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_1",
        "pageview",
        "/home",
        None,
        "US",
        &format!("{} 09:00:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_2",
        "sess_1",
        "pageview",
        "/pricing",
        None,
        "US",
        &format!("{} 09:01:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_3",
        "sess_1",
        "event",
        "/pricing",
        Some("signup_clicked"),
        "US",
        &format!("{} 09:02:00", day),
    )
    .await;

    let query = JourneyQuery {
        anchor_type: AnchorType::Event,
        anchor_value: "signup_clicked".to_string(),
        direction: JourneyDirection::Previous,
        max_depth: 2,
    };

    let result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey");

    assert_eq!(result.total_anchor_sessions, 1);
    assert_eq!(result.branches.len(), 1);
    assert_eq!(
        result.branches[0].nodes,
        vec!["/home".to_string(), "/pricing".to_string()]
    );
}

#[tokio::test]
async fn journey_returns_empty_when_anchor_missing() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();
    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_1",
        "pageview",
        "/home",
        None,
        "US",
        &format!("{} 08:00:00", day),
    )
    .await;

    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/pricing".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 3,
    };

    let result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey");

    assert_eq!(result.total_anchor_sessions, 0);
    assert!(result.branches.is_empty());
}

#[tokio::test]
async fn journey_applies_dimension_filters() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();

    // US branch
    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_us",
        "pageview",
        "/pricing",
        None,
        "US",
        &format!("{} 08:00:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_2",
        "sess_us",
        "pageview",
        "/checkout",
        None,
        "US",
        &format!("{} 08:01:00", day),
    )
    .await;

    // DE branch
    insert_event(
        &db,
        "site_1",
        "evt_3",
        "sess_de",
        "pageview",
        "/pricing",
        None,
        "DE",
        &format!("{} 09:00:00", day),
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_4",
        "sess_de",
        "pageview",
        "/features",
        None,
        "DE",
        &format!("{} 09:01:00", day),
    )
    .await;

    let mut filter = base_filter(day);
    filter.filter_country = Some("US".to_string());

    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/pricing".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 1,
    };

    let result = db
        .get_journey("site_1", None, &filter, &query)
        .await
        .expect("journey");

    assert_eq!(result.total_anchor_sessions, 1);
    assert_eq!(result.branches.len(), 1);
    assert_eq!(result.branches[0].nodes, vec!["/checkout".to_string()]);
}

#[tokio::test]
async fn journey_caps_branch_output_to_twenty() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();

    for idx in 0..25 {
        let session = format!("sess_{idx}");
        let anchor_id = format!("evt_anchor_{idx}");
        let next_id = format!("evt_next_{idx}");
        let next_page = format!("/next-{idx}");

        insert_event(
            &db,
            "site_1",
            &anchor_id,
            &session,
            "pageview",
            "/pricing",
            None,
            "US",
            &format!("{} 14:{:02}:00", day, idx % 60),
        )
        .await;
        insert_event(
            &db,
            "site_1",
            &next_id,
            &session,
            "pageview",
            &next_page,
            None,
            "US",
            &format!("{} 15:{:02}:00", day, idx % 60),
        )
        .await;
    }

    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/pricing".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 1,
    };

    let result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey");

    assert_eq!(result.total_anchor_sessions, 25);
    assert_eq!(result.branches.len(), 20);
}

#[tokio::test]
async fn journey_respects_timezone_bounds() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    // UTC timestamp 2026-01-01 23:30 is 2026-01-02 00:30 in Europe/Warsaw.
    insert_event(
        &db,
        "site_1",
        "evt_1",
        "sess_1",
        "pageview",
        "/pricing",
        None,
        "US",
        "2026-01-01 23:30:00",
    )
    .await;
    insert_event(
        &db,
        "site_1",
        "evt_2",
        "sess_1",
        "pageview",
        "/checkout",
        None,
        "US",
        "2026-01-01 23:31:00",
    )
    .await;

    let day = NaiveDate::from_ymd_opt(2026, 1, 2).expect("valid date");
    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/pricing".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 1,
    };

    let utc_result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey UTC");
    assert_eq!(utc_result.total_anchor_sessions, 0);

    let mut warsaw_filter = base_filter(day);
    warsaw_filter.timezone = Some("Europe/Warsaw".to_string());
    let warsaw_result = db
        .get_journey("site_1", None, &warsaw_filter, &query)
        .await
        .expect("journey Europe/Warsaw");
    assert_eq!(warsaw_result.total_anchor_sessions, 1);
    assert_eq!(warsaw_result.branches.len(), 1);
    assert_eq!(
        warsaw_result.branches[0].nodes,
        vec!["/checkout".to_string()]
    );
}

#[tokio::test]
async fn journey_clamps_depth_to_five_steps() {
    let db = DuckDbBackend::open_in_memory().expect("db");
    db.seed_website("site_1", "example.com")
        .await
        .expect("seed");

    let day = chrono::Utc::now().date_naive();
    let pages = ["/pricing", "/a", "/b", "/c", "/d", "/e", "/f"];
    for (idx, page) in pages.iter().enumerate() {
        insert_event(
            &db,
            "site_1",
            &format!("evt_{idx}"),
            "sess_1",
            "pageview",
            page,
            None,
            "US",
            &format!("{} 16:{:02}:00", day, idx),
        )
        .await;
    }

    let query = JourneyQuery {
        anchor_type: AnchorType::Page,
        anchor_value: "/pricing".to_string(),
        direction: JourneyDirection::Next,
        max_depth: 9,
    };

    let result = db
        .get_journey("site_1", None, &base_filter(day), &query)
        .await
        .expect("journey");

    assert_eq!(result.max_depth, 5);
    assert_eq!(result.branches.len(), 1);
    assert_eq!(result.branches[0].nodes.len(), 5);
}
