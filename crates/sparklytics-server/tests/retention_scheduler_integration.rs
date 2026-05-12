mod common;

use std::sync::Arc;

use chrono::{Duration, Utc};

use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::{scheduler, state::AppState};

fn test_config() -> Config {
    Config {
        port: 0,
        data_dir: common::unique_data_dir("retention-scheduler"),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        bootstrap_password: None,
        https: false,
        retention_days: 7,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::SelfHosted,
        argon2_memory_kb: 65536,
        public_url: "http://localhost:3000".to_string(),
        tracking_public_base: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

#[tokio::test]
async fn scheduler_retention_prune_uses_configured_retention_days() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_retention", "example.com")
        .await
        .expect("seed website");

    let old = (Utc::now() - Duration::days(10)).to_rfc3339();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    {
        let conn = db.conn_for_test().await;
        conn.execute(
            r#"INSERT INTO events (
                id, website_id, session_id, visitor_id, event_type, url, created_at
            ) VALUES
                ('event_old', 'site_retention', 'session_old', 'visitor_old', 'pageview', 'https://example.com/old', CAST(?1 AS TIMESTAMP)),
                ('event_recent', 'site_retention', 'session_recent', 'visitor_recent', 'pageview', 'https://example.com/recent', CAST(?2 AS TIMESTAMP))"#,
            sparklytics_duckdb::duckdb::params![old, recent],
        )
        .expect("insert events");
        conn.execute(
            r#"INSERT INTO sessions (
                session_id, website_id, visitor_id, first_seen, last_seen, pageview_count, entry_page
            ) VALUES
                ('session_old', 'site_retention', 'visitor_old', CAST(?1 AS TIMESTAMP), CAST(?1 AS TIMESTAMP), 1, '/old'),
                ('session_recent', 'site_retention', 'visitor_recent', CAST(?2 AS TIMESTAMP), CAST(?2 AS TIMESTAMP), 1, '/recent')"#,
            sparklytics_duckdb::duckdb::params![old, recent],
        )
        .expect("insert sessions");
    }

    let state = Arc::new(AppState::new(db, test_config()));
    let stats = scheduler::prune_retention_once(&state)
        .await
        .expect("retention prune");

    assert_eq!(stats.events_deleted, 1);
    assert_eq!(stats.sessions_deleted, 1);

    let conn = state.db.conn_for_test().await;
    let remaining_events: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .expect("count events");
    let remaining_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("count sessions");

    assert_eq!(remaining_events, 1);
    assert_eq!(remaining_sessions, 1);
}

#[tokio::test]
async fn startup_maintenance_prunes_retention_immediately() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_startup_retention", "example.com")
        .await
        .expect("seed website");

    let old = (Utc::now() - Duration::days(10)).to_rfc3339();
    {
        let conn = db.conn_for_test().await;
        conn.execute(
            r#"INSERT INTO events (
                id, website_id, session_id, visitor_id, event_type, url, created_at
            ) VALUES (
                'event_startup_old', 'site_startup_retention', 'session_startup_old', 'visitor_startup_old', 'pageview', 'https://example.com/old', CAST(?1 AS TIMESTAMP)
            )"#,
            sparklytics_duckdb::duckdb::params![old],
        )
        .expect("insert event");
        conn.execute(
            r#"INSERT INTO sessions (
                session_id, website_id, visitor_id, first_seen, last_seen, pageview_count, entry_page
            ) VALUES (
                'session_startup_old', 'site_startup_retention', 'visitor_startup_old', CAST(?1 AS TIMESTAMP), CAST(?1 AS TIMESTAMP), 1, '/old'
            )"#,
            sparklytics_duckdb::duckdb::params![old],
        )
        .expect("insert session");
    }

    let state = Arc::new(AppState::new(db, test_config()));
    let (_login_attempts, retention) = scheduler::run_startup_maintenance(&state).await;
    let stats = retention.expect("startup retention prune");

    assert_eq!(stats.events_deleted, 1);
    assert_eq!(stats.sessions_deleted, 1);
}

#[tokio::test]
async fn retention_prune_treats_zero_days_as_one_day() {
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_retention_zero", "example.com")
        .await
        .expect("seed website");

    let old = (Utc::now() - Duration::days(2)).to_rfc3339();
    let recent = (Utc::now() - Duration::hours(12)).to_rfc3339();
    {
        let conn = db.conn_for_test().await;
        conn.execute(
            r#"INSERT INTO events (
                id, website_id, session_id, visitor_id, event_type, url, created_at
            ) VALUES
                ('event_zero_old', 'site_retention_zero', 'session_zero_old', 'visitor_zero_old', 'pageview', 'https://example.com/old', CAST(?1 AS TIMESTAMP)),
                ('event_zero_recent', 'site_retention_zero', 'session_zero_recent', 'visitor_zero_recent', 'pageview', 'https://example.com/recent', CAST(?2 AS TIMESTAMP))"#,
            sparklytics_duckdb::duckdb::params![old, recent],
        )
        .expect("insert events");
        conn.execute(
            r#"INSERT INTO sessions (
                session_id, website_id, visitor_id, first_seen, last_seen, pageview_count, entry_page
            ) VALUES
                ('session_zero_old', 'site_retention_zero', 'visitor_zero_old', CAST(?1 AS TIMESTAMP), CAST(?1 AS TIMESTAMP), 1, '/old'),
                ('session_zero_recent', 'site_retention_zero', 'visitor_zero_recent', CAST(?2 AS TIMESTAMP), CAST(?2 AS TIMESTAMP), 1, '/recent')"#,
            sparklytics_duckdb::duckdb::params![old, recent],
        )
        .expect("insert sessions");
    }

    let stats = db
        .prune_analytics_retention(0)
        .await
        .expect("retention prune");

    assert_eq!(stats.events_deleted, 1);
    assert_eq!(stats.sessions_deleted, 1);

    let conn = db.conn_for_test().await;
    let remaining_events: i64 = conn
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .expect("count events");
    let remaining_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("count sessions");

    assert_eq!(remaining_events, 1);
    assert_eq!(remaining_sessions, 1);
}
