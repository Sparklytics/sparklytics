#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration as ChronoDuration, Utc};
use tokio::time::{sleep, Duration, Instant};

static UNIQUE_DATA_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns a process-scoped temporary data directory for an integration suite.
pub fn unique_data_dir(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let pid = std::process::id();
    let seq = UNIQUE_DATA_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("sparklytics-{prefix}-{pid}-{ts}-{seq}"));
    std::fs::create_dir(&path).expect("create unique test data dir");
    path.to_string_lossy().into_owned()
}

/// Polls until the provided async condition succeeds or the timeout elapses.
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

/// Returns a query window that safely spans yesterday through tomorrow in UTC.
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
