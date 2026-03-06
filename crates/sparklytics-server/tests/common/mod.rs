#![allow(dead_code)]

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration as ChronoDuration, Utc};
use tokio::time::{sleep, Duration, Instant};

pub fn unique_data_dir(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let path = format!("/tmp/sparklytics-{prefix}-{ts}");
    std::fs::create_dir_all(&path).expect("create unique test data dir");
    path
}

pub async fn poll_until<F, Fut>(timeout: Duration, interval: Duration, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if condition().await {
            return;
        }
        assert!(Instant::now() < deadline, "condition timed out");
        sleep(interval).await;
    }
}

pub fn surrounding_date_window() -> (String, String) {
    let today = Utc::now().date_naive();
    (
        (today - ChronoDuration::days(1))
            .format("%Y-%m-%d")
            .to_string(),
        (today + ChronoDuration::days(1))
            .format("%Y-%m-%d")
            .to_string(),
    )
}
