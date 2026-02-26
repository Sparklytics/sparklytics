use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sparklytics_core::billing::NullBillingGate;
use sparklytics_core::config::{AppMode, AuthMode, Config};
use sparklytics_duckdb::website::CreateWebsiteParams;
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_server::metadata::duckdb::DuckDbMetadataStore;
use sparklytics_server::state::AppState;

fn test_config() -> Config {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    Config {
        port: 0,
        data_dir: format!("/tmp/sparklytics-test-state-{ts}"),
        geoip_path: "/nonexistent/GeoLite2-City.mmdb".to_string(),
        auth_mode: AuthMode::None,
        https: false,
        retention_days: 365,
        cors_origins: vec![],
        session_days: 7,
        buffer_flush_interval_ms: 5_000,
        buffer_max_size: 100,
        mode: AppMode::Cloud,
        argon2_memory_kb: 4_096,
        public_url: "http://localhost:3000".to_string(),
        rate_limit_disable: true,
        duckdb_memory_limit: "1GB".to_string(),
    }
}

#[tokio::test]
async fn new_with_backends_and_metadata_uses_injected_metadata_store() {
    let primary_db = DuckDbBackend::open_in_memory().expect("primary duckdb");
    let metadata_db = DuckDbBackend::open_in_memory().expect("metadata duckdb");

    let primary_site = primary_db
        .create_website(CreateWebsiteParams {
            name: "Primary Site".to_string(),
            domain: "primary.example.com".to_string(),
            timezone: Some("UTC".to_string()),
        })
        .await
        .expect("create site in primary db");

    let metadata_site = metadata_db
        .create_website(CreateWebsiteParams {
            name: "Metadata Site".to_string(),
            domain: "metadata.example.com".to_string(),
            timezone: Some("UTC".to_string()),
        })
        .await
        .expect("create site in metadata db");

    assert_ne!(
        primary_site.id, metadata_site.id,
        "test site ids should differ"
    );

    let analytics = Arc::new(primary_db.clone());
    let metadata = Arc::new(DuckDbMetadataStore::new(Arc::new(metadata_db.clone())));
    let billing_gate = Arc::new(NullBillingGate);
    let state = AppState::new_with_backends_and_metadata(
        primary_db,
        test_config(),
        analytics,
        metadata,
        billing_gate,
    );

    assert!(
        state.is_valid_website(&metadata_site.id).await,
        "website from injected metadata store should be valid"
    );
    assert!(
        !state.is_valid_website(&primary_site.id).await,
        "website only in primary DB should not be valid when metadata backend is separate"
    );
}
