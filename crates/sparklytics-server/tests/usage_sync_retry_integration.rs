mod common;

use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use tokio::time::Duration;

use sparklytics_core::billing::{BillingAdmission, BillingGate};
use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_core::event::Event;
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::state::AppState;

const RETRY_QUEUE_FILE: &str = "usage-sync/retry-queue.json";

#[derive(Debug, Deserialize)]
struct UsageSyncRetryEntry {
    tenant_id: String,
    event_count: usize,
    retries: u32,
}

struct AlwaysFailBillingGate;

#[async_trait]
impl BillingGate for AlwaysFailBillingGate {
    async fn admit_events(&self, _tenant_id: &str, requested_events: usize) -> BillingAdmission {
        BillingAdmission::allow_all(requested_events)
    }

    async fn record_persisted_events(
        &self,
        _tenant_id: &str,
        _persisted_events: usize,
    ) -> anyhow::Result<()> {
        anyhow::bail!("forced billing sync failure")
    }
}

struct RecordingBillingGate {
    calls: Arc<StdMutex<Vec<(String, usize)>>>,
}

#[async_trait]
impl BillingGate for RecordingBillingGate {
    async fn admit_events(&self, _tenant_id: &str, requested_events: usize) -> BillingAdmission {
        BillingAdmission::allow_all(requested_events)
    }

    async fn record_persisted_events(
        &self,
        tenant_id: &str,
        persisted_events: usize,
    ) -> anyhow::Result<()> {
        self.calls
            .lock()
            .expect("lock calls")
            .push((tenant_id.to_string(), persisted_events));
        Ok(())
    }
}

fn unique_data_dir(tag: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!(
        "/tmp/sparklytics-usage-sync-retry-{tag}-{}-{ts}",
        std::process::id()
    )
}

fn test_config(data_dir: &str) -> Config {
    Config {
        port: 0,
        data_dir: data_dir.to_string(),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        bootstrap_password: None,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5000,
        buffer_max_size: 100,
        mode: AppMode::Cloud,
        argon2_memory_kb: 65536,
        public_url: "http://localhost:3000".to_string(),
        tracking_public_base: "http://localhost:3000".to_string(),
        rate_limit_disable: false,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

fn pageview_event(tenant_id: &str) -> Event {
    Event {
        id: "evt_retry_1".to_string(),
        website_id: "site_test".to_string(),
        tenant_id: Some(tenant_id.to_string()),
        session_id: "session_retry_1".to_string(),
        visitor_id: "visitor_retry_1".to_string(),
        event_type: "pageview".to_string(),
        url: "/".to_string(),
        referrer_url: None,
        referrer_domain: None,
        event_name: None,
        event_data: None,
        country: None,
        region: None,
        city: None,
        browser: None,
        browser_version: None,
        os: None,
        os_version: None,
        device_type: None,
        screen: None,
        language: None,
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
        created_at: Utc::now(),
    }
}

async fn read_retry_entries(data_dir: &str) -> Vec<UsageSyncRetryEntry> {
    let path = std::path::Path::new(data_dir).join(RETRY_QUEUE_FILE);
    let content = tokio::fs::read_to_string(path)
        .await
        .expect("read retry queue file");
    serde_json::from_str::<Vec<UsageSyncRetryEntry>>(&content).expect("parse retry queue file")
}

#[tokio::test]
async fn test_failed_usage_sync_is_persisted_to_retry_file() {
    let data_dir = unique_data_dir("persist");
    std::fs::create_dir_all(&data_dir).expect("create data dir");

    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    db.seed_website("site_test", "example.com")
        .await
        .expect("seed website");

    let mut state = AppState::new(db, test_config(&data_dir));
    state.billing_gate = Arc::new(AlwaysFailBillingGate);
    let state = Arc::new(state);

    state
        .enqueue_ingest_events(vec![pageview_event("org_retry")])
        .await
        .expect("enqueue events");

    common::poll_until(Duration::from_secs(5), Duration::from_millis(25), || {
        let data_dir = data_dir.clone();
        async move {
            let entries =
                std::fs::read_to_string(std::path::Path::new(&data_dir).join(RETRY_QUEUE_FILE))
                    .ok()
                    .and_then(|raw| serde_json::from_str::<Vec<UsageSyncRetryEntry>>(&raw).ok())
                    .unwrap_or_default();
            entries
                .iter()
                .any(|entry| entry.tenant_id == "org_retry" && entry.event_count >= 1)
        }
    })
    .await;

    let entries = read_retry_entries(&data_dir).await;
    let queued = entries
        .iter()
        .find(|entry| entry.tenant_id == "org_retry")
        .expect("queued tenant usage retry entry");
    assert!(queued.event_count >= 1);
    assert!(queued.retries >= 1);

    drop(state);
    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn test_usage_sync_retry_replays_on_startup() {
    let data_dir = unique_data_dir("restore");
    let retry_dir = std::path::Path::new(&data_dir).join("usage-sync");
    std::fs::create_dir_all(&retry_dir).expect("create retry dir");
    std::fs::write(
        retry_dir.join("retry-queue.json"),
        r#"[{"tenant_id":"org_restore","event_count":7,"retries":3}]"#,
    )
    .expect("seed retry queue file");

    let calls = Arc::new(StdMutex::new(Vec::new()));
    let db = DuckDbBackend::open_in_memory().expect("in-memory DuckDB");
    let mut state = AppState::new(db, test_config(&data_dir));
    state.billing_gate = Arc::new(RecordingBillingGate {
        calls: Arc::clone(&calls),
    });
    let state = Arc::new(state);

    state.restore_ingest_queue_from_wal().await;

    common::poll_until(Duration::from_secs(5), Duration::from_millis(25), || {
        let calls = Arc::clone(&calls);
        async move {
            let calls = calls.lock().expect("lock calls");
            calls
                .iter()
                .any(|(tenant_id, count)| tenant_id == "org_restore" && *count == 7)
        }
    })
    .await;

    common::poll_until(Duration::from_secs(5), Duration::from_millis(25), || {
        let data_dir = data_dir.clone();
        async move {
            let entries =
                std::fs::read_to_string(std::path::Path::new(&data_dir).join(RETRY_QUEUE_FILE))
                    .ok()
                    .and_then(|raw| serde_json::from_str::<Vec<UsageSyncRetryEntry>>(&raw).ok())
                    .unwrap_or_default();
            entries.is_empty()
        }
    })
    .await;

    drop(state);
    let _ = std::fs::remove_dir_all(&data_dir);
}
