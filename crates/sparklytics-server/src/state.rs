use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

use sparklytics_core::{config::Config, event::Event};
use sparklytics_duckdb::DuckDbBackend;

/// Shared application state injected into every Axum handler via
/// [`axum::extract::State`].
///
/// All fields are safe to clone cheaply — heavy resources are wrapped in
/// `Arc` or `Arc<Mutex<_>>`.
pub struct AppState {
    /// The DuckDB backend. Internally uses `Arc<tokio::sync::Mutex<Connection>>`
    /// so it is already cheap to clone and async-safe.
    pub db: Arc<DuckDbBackend>,

    /// Parsed configuration, loaded once at startup from environment variables.
    pub config: Arc<Config>,

    /// In-memory event buffer.
    ///
    /// Lock, drain, release — then flush to DB. The lock is held only long
    /// enough to `std::mem::take` the buffer so the DB write does not block
    /// incoming collect requests.
    pub buffer: Arc<Mutex<Vec<Event>>>,

    /// Fast in-process cache of known-valid `website_id` values.
    ///
    /// Populated lazily: the first collect request for a site triggers a DB
    /// lookup; subsequent requests hit the cache. The cache is never invalidated
    /// during a server run (websites are not deleted at runtime in Sprint 0).
    pub website_cache: Arc<RwLock<HashSet<String>>>,

    /// Per-IP sliding-window rate limiter for POST /api/collect.
    ///
    /// Key: IP address string. Value: deque of request timestamps within the
    /// last 60 seconds. Limit: 60 requests per IP per 60-second window.
    rate_limiter: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl AppState {
    /// Construct a new `AppState` wrapping the given backend and config.
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check whether `ip` is within the 60 req/min rate limit.
    ///
    /// Returns `true` if the request should proceed, `false` if it should be
    /// rejected with 429. Slides the window on every call.
    pub async fn check_rate_limit(&self, ip: &str) -> bool {
        let mut map = self.rate_limiter.lock().await;
        let window = map.entry(ip.to_string()).or_default();
        let cutoff = Instant::now() - std::time::Duration::from_secs(60);
        // Drop timestamps older than the 60-second window.
        while window.front().is_some_and(|t| *t < cutoff) {
            window.pop_front();
        }
        if window.len() >= 60 {
            return false; // limit reached
        }
        window.push_back(Instant::now());
        true
    }

    /// Background loop: rotate the daily salt at midnight UTC.
    ///
    /// Calculates time until the next UTC midnight, sleeps until then, rotates,
    /// and repeats. A failed rotation is logged as an error but does not crash
    /// the loop — visitor IDs continue working with the current salt.
    pub async fn run_salt_rotation_loop(self: Arc<Self>) {
        loop {
            let now = Utc::now();
            let tomorrow = now.date_naive() + chrono::Duration::days(1);
            let next_midnight = tomorrow
                .and_hms_opt(0, 0, 0)
                .expect("valid time")
                .and_utc();
            let secs_until = (next_midnight - now).num_seconds().max(1) as u64;
            tokio::time::sleep(std::time::Duration::from_secs(secs_until)).await;
            match self.db.rotate_salt().await {
                Ok(()) => info!("Daily salt rotated at midnight UTC"),
                Err(e) => error!(error = %e, "Salt rotation failed — keeping current salt"),
            }
        }
    }

    /// Append `events` to the in-memory buffer.
    ///
    /// If the buffer length reaches or exceeds `config.buffer_max_size` after
    /// the append, an immediate flush is triggered (Sprint 0 requirement:
    /// "100 events flush immediately, not wait for 5-s timer").
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

    /// Drain the buffer and write all pending events to DuckDB.
    ///
    /// The `Mutex` is held only for the `std::mem::take` so the collect
    /// endpoint is not blocked while the DB write runs.
    pub async fn flush_buffer(&self) {
        let batch: Vec<Event> = {
            let mut buf = self.buffer.lock().await;
            std::mem::take(&mut *buf)
        };

        if batch.is_empty() {
            return;
        }

        match self.db.insert_events(&batch).await {
            Ok(()) => {
                info!(count = batch.len(), "Buffer flushed to DuckDB");
            }
            Err(e) => {
                error!(count = batch.len(), error = %e, "Buffer flush failed — events lost");
            }
        }
    }

    /// Background loop: flush the buffer on a fixed interval.
    ///
    /// Spawned as a `tokio::spawn` task in `main.rs`. Runs until the process
    /// exits. Interval is read from `config.buffer_flush_interval_ms`
    /// (default 5 000 ms = 5 seconds per Sprint 0 spec).
    pub async fn run_buffer_flush_loop(self: Arc<Self>) {
        let interval = self.config.buffer_flush_interval();
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            self.flush_buffer().await;
        }
    }

    /// Return `true` if the `website_id` is known to exist.
    ///
    /// Checks the in-process cache first; on a cache miss falls back to a
    /// DuckDB query and populates the cache on success.
    pub async fn is_valid_website(&self, website_id: &str) -> bool {
        // Fast path: cache hit.
        {
            let cache = self.website_cache.read().await;
            if cache.contains(website_id) {
                return true;
            }
        }

        // Slow path: DB lookup.
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
