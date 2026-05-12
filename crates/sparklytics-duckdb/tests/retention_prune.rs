use chrono::{Duration, Utc};
use sparklytics_duckdb::{DuckDbBackend, RetentionPruneStats};

#[tokio::test]
async fn retention_prune_deletes_only_rows_older_than_horizon() {
    let db = DuckDbBackend::open_in_memory().expect("open duckdb");
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

    let stats = db
        .prune_analytics_retention(7)
        .await
        .expect("prune retention");
    assert_eq!(
        stats,
        RetentionPruneStats {
            events_deleted: 1,
            sessions_deleted: 1
        }
    );

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
