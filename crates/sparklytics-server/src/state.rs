use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{error, info};

use sparklytics_core::{
    analytics::AnalyticsBackend,
    billing::{BillingGate, NullBillingGate},
    config::Config,
    event::Event,
};
use sparklytics_duckdb::DuckDbBackend;

/// Shared application state injected into every Axum handler via
/// [`axum::extract::State`].
pub struct AppState {
    /// DuckDB backend used for self-hosted metadata operations.
    pub db: Arc<DuckDbBackend>,

    /// Analytics backend used by all analytics routes and buffer flush.
    pub analytics: Arc<dyn AnalyticsBackend>,

    /// Parsed configuration, loaded once at startup.
    pub config: Arc<Config>,

    /// In-memory event buffer.
    pub buffer: Arc<Mutex<Vec<Event>>>,

    /// Fast in-process cache of known-valid `website_id` values.
    pub website_cache: Arc<RwLock<HashSet<String>>>,

    /// Per-IP sliding-window rate limiter for POST /api/collect.
    rate_limiter: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,

    /// Plan-limit gate.
    pub billing_gate: Arc<dyn BillingGate>,

    /// Serialize export requests to avoid prolonged DuckDB contention.
    pub export_semaphore: Arc<Semaphore>,
}

impl AppState {
    /// Constructor for self-hosted mode.
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        let db = Arc::new(db);
        let analytics: Arc<dyn AnalyticsBackend> = db.clone();
        Self {
            db,
            analytics,
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            billing_gate: Arc::new(NullBillingGate),
            export_semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Construct state with explicit analytics and billing backends.
    pub fn new_with_backends(
        db: DuckDbBackend,
        config: Config,
        analytics: Arc<dyn AnalyticsBackend>,
        billing_gate: Arc<dyn BillingGate>,
    ) -> Self {
        let mut s = Self::new(db, config);
        s.analytics = analytics;
        s.billing_gate = billing_gate;
        s
    }

    /// Check whether `ip` is within the 60 req/min rate limit.
    pub async fn check_rate_limit(&self, ip: &str) -> bool {
        self.check_rate_limit_with_max(ip, 60).await
    }

    /// Check whether `ip` is within the given `max_per_min` rate limit.
    pub async fn check_rate_limit_with_max(&self, ip: &str, max_per_min: usize) -> bool {
        let key = format!("{}:{}", max_per_min, ip);
        let mut map = self.rate_limiter.lock().await;
        let cutoff = Instant::now() - std::time::Duration::from_secs(60);
        if let Some(window) = map.get_mut(&key) {
            while window.front().is_some_and(|t| *t < cutoff) {
                window.pop_front();
            }
            if window.is_empty() {
                map.remove(&key);
            }
        }
        let window = map.entry(key).or_default();
        if window.len() >= max_per_min {
            return false;
        }
        window.push_back(Instant::now());
        true
    }

    /// Background loop: rotate the daily salt at midnight UTC.
    pub async fn run_salt_rotation_loop(self: Arc<Self>) {
        loop {
            let now = Utc::now();
            let tomorrow = now.date_naive() + chrono::Duration::days(1);
            let next_midnight = match tomorrow.and_hms_opt(0, 0, 0) {
                Some(t) => t.and_utc(),
                None => {
                    error!("Failed to compute next midnight - skipping salt rotation");
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                    continue;
                }
            };
            let secs_until = (next_midnight - now).num_seconds().max(1) as u64;
            tokio::time::sleep(std::time::Duration::from_secs(secs_until)).await;
            match self.db.rotate_salt().await {
                Ok(()) => info!("Daily salt rotated at midnight UTC"),
                Err(e) => error!(error = %e, "Salt rotation failed - keeping current salt"),
            }
        }
    }

    /// Append events to the in-memory buffer, flushing if threshold is reached.
    pub async fn push_events(&self, events: Vec<Event>) {
        let should_flush = {
            let mut buf = self.buffer.lock().await;
            buf.extend(events);
            buf.len() >= self.config.buffer_max_size
        };
        if should_flush {
            self.flush_buffer().await;
        }
    }

    /// Drain the buffer and write all pending events through the analytics backend.
    pub async fn flush_buffer(&self) {
        let batch: Vec<Event> = {
            let mut buf = self.buffer.lock().await;
            std::mem::take(&mut *buf)
        };
        if batch.is_empty() {
            return;
        }
        match self.analytics.insert_events(&batch).await {
            Ok(()) => info!(count = batch.len(), "Buffer flushed"),
            Err(e) => {
                error!(count = batch.len(), error = %e, "Buffer flush failed - events lost")
            }
        }
    }

    /// Background loop: flush the buffer on a fixed interval.
    pub async fn run_buffer_flush_loop(self: Arc<Self>) {
        let interval = self.config.buffer_flush_interval();
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            self.flush_buffer().await;
        }
    }

    /// Return `true` if the `website_id` is known to exist (cache + DB).
    pub async fn is_valid_website(&self, website_id: &str) -> bool {
        {
            let cache = self.website_cache.read().await;
            if cache.contains(website_id) {
                return true;
            }
        }
        match self.db.website_exists(website_id).await {
            Ok(true) => {
                let mut cache = self.website_cache.write().await;
                cache.insert(website_id.to_string());
                true
            }
            Ok(false) => false,
            Err(e) => {
                error!(website_id, error = %e, "website_exists DB lookup failed");
                false
            }
        }
    }
}
