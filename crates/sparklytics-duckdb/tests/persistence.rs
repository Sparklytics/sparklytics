use std::time::{SystemTime, UNIX_EPOCH};

use sparklytics_duckdb::{website::CreateWebsiteParams, DuckDbBackend};

fn unique_data_dir(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    format!("/tmp/sparklytics-{prefix}-{ts}")
}

#[tokio::test]
async fn file_backed_duckdb_persists_websites_across_reopen() {
    let data_dir = unique_data_dir("persistence");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let db_path = format!("{data_dir}/sparklytics.db");

    let website_id = {
        let db = DuckDbBackend::open(&db_path, "512MB").expect("open first db");
        let website = db
            .create_website(CreateWebsiteParams {
                name: "Persistent Site".to_string(),
                domain: "persistent.example.com".to_string(),
                timezone: Some("Europe/Warsaw".to_string()),
            })
            .await
            .expect("create website");

        assert_eq!(website.tenant_id, None);
        website.id
    };

    let reopened = DuckDbBackend::open(&db_path, "512MB").expect("reopen db");
    let persisted = reopened
        .get_website(&website_id)
        .await
        .expect("query website")
        .expect("website persisted");

    assert_eq!(persisted.name, "Persistent Site");
    assert_eq!(persisted.domain, "persistent.example.com");
    assert_eq!(persisted.timezone, "Europe/Warsaw");
    assert_eq!(persisted.tenant_id, None);

    let _ = std::fs::remove_dir_all(&data_dir);
}
