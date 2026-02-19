use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

use sparklytics_core::{config::Config, event::Event};
use sparklytics_duckdb::DuckDbBackend;

use crate::billing::{BillingGate, NullBillingGate};

#[cfg(feature = "cloud")]
use crate::cloud::{clickhouse::ClickHouseClient, config::CloudConfig};

/// Shared application state injected into every Axum handler via
/// [`axum::extract::State`].
///
/// All fields are safe to clone cheaply — heavy resources are wrapped in
/// `Arc` or `Arc<Mutex<_>>`.
pub struct AppState {
    /// The DuckDB backend (self-hosted mode analytics + metadata).
    pub db: Arc<DuckDbBackend>,

    /// Parsed configuration, loaded once at startup.
    pub config: Arc<Config>,

    /// In-memory event buffer (flushed to DuckDB on a 5-second interval or when
    /// it reaches 100 events — whichever comes first).
    pub buffer: Arc<Mutex<Vec<Event>>>,

    /// Fast in-process cache of known-valid `website_id` values.
    pub website_cache: Arc<RwLock<HashSet<String>>>,

    /// Per-IP sliding-window rate limiter for POST /api/collect.
    rate_limiter: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,

    /// Plan-limit gate. Self-hosted always uses [`NullBillingGate`].
    /// Cloud public binary also defaults to NullBillingGate; the private
    /// `sparklytics-billing` crate injects `StripeBillingGate` (critical fact #13).
    pub billing_gate: Arc<dyn BillingGate>,

    /// Cloud-mode PostgreSQL connection pool (None in self-hosted mode).
    #[cfg(feature = "cloud")]
    pub pg_pool: Option<sqlx::PgPool>,

    /// Cloud-mode ClickHouse HTTP client (None in self-hosted mode).
    #[cfg(feature = "cloud")]
    pub clickhouse: Option<ClickHouseClient>,

    /// Cloud-mode configuration — Clerk, ClickHouse, DB URLs (None in self-hosted mode).
    #[cfg(feature = "cloud")]
    pub cloud_config: Option<Arc<CloudConfig>>,
}

impl AppState {
    /// Constructor for self-hosted mode (no cloud fields).
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            billing_gate: Arc::new(NullBillingGate),
            // Cloud fields are None in self-hosted mode.
            #[cfg(feature = "cloud")]
            pg_pool: None,
            #[cfg(feature = "cloud")]
            clickhouse: None,
            #[cfg(feature = "cloud")]
            cloud_config: None,
        }
    }

    /// Constructor for cloud mode (includes PostgreSQL + ClickHouse).
    #[cfg(feature = "cloud")]
    pub fn new_cloud(
        db: DuckDbBackend,
        config: Config,
        pg_pool: sqlx::PgPool,
        clickhouse: ClickHouseClient,
        cloud_config: CloudConfig,
    ) -> Self {
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            billing_gate: Arc::new(NullBillingGate),
            pg_pool: Some(pg_pool),
            clickhouse: Some(clickhouse),
            cloud_config: Some(Arc::new(cloud_config)),
        }
    }

    /// Return the PostgreSQL pool (cloud mode only).
    #[cfg(feature = "cloud")]
    pub fn cloud_pg(&self) -> anyhow::Result<&sqlx::PgPool> {
        self.pg_pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("PostgreSQL pool not initialized — self-hosted mode"))
    }

    /// Return the ClickHouse client (cloud mode only).
    #[cfg(feature = "cloud")]
    pub fn cloud_ch(&self) -> anyhow::Result<&ClickHouseClient> {
        self.clickhouse
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ClickHouse client not initialized — self-hosted mode"))
    }

    /// Return the cloud configuration (cloud mode only).
    #[cfg(feature = "cloud")]
    pub fn cloud_cfg(&self) -> anyhow::Result<&CloudConfig> {
        self.cloud_config
            .as_ref()
            .map(Arc::as_ref)
            .ok_or_else(|| anyhow::anyhow!("Cloud config not initialized — self-hosted mode"))
    }

    /// Check whether `ip` is within the 60 req/min rate limit.
    pub async fn check_rate_limit(&self, ip: &str) -> bool {
        self.check_rate_limit_with_max(ip, 60).await
    }

    /// Check whether `ip` is within the given `max_per_min` rate limit.
    ///
    /// Keys are namespaced as `"{max}:{ip}"` so different limits don't collide
    /// in the shared HashMap. For example, `/api/collect` uses 60/min and
    /// `/api/share/:id/*` uses 30/min.
    pub async fn check_rate_limit_with_max(&self, ip: &str, max_per_min: usize) -> bool {
        let key = format!("{}:{}", max_per_min, ip);
        let mut map = self.rate_limiter.lock().await;
        let cutoff = Instant::now() - std::time::Duration::from_secs(60);
        // Evict stale timestamps and remove the entry entirely when the window
        // empties, so the HashMap doesn't grow unboundedly for long-tail unique IPs.
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

    /// Construct state with a custom billing gate.
    ///
    /// Used by tests (mock gates) and by the private `sparklytics-billing`
    /// crate to inject `StripeBillingGate` at startup.
    pub fn new_with_billing_gate(
        db: DuckDbBackend,
        config: Config,
        billing_gate: Arc<dyn BillingGate>,
    ) -> Self {
        let mut s = Self::new(db, config);
        s.billing_gate = billing_gate;
        s
    }

    /// Background loop: rotate the daily salt at midnight UTC.
    pub async fn run_salt_rotation_loop(self: Arc<Self>) {
        loop {
            let now = Utc::now();
            let tomorrow = now.date_naive() + chrono::Duration::days(1);
            let next_midnight = match tomorrow.and_hms_opt(0, 0, 0) {
                Some(t) => t.and_utc(),
                None => {
                    error!("Failed to compute next midnight — skipping salt rotation");
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                    continue;
                }
            };
            let secs_until = (next_midnight - now).num_seconds().max(1) as u64;
            tokio::time::sleep(std::time::Duration::from_secs(secs_until)).await;
            match self.db.rotate_salt().await {
                Ok(()) => info!("Daily salt rotated at midnight UTC"),
                Err(e) => error!(error = %e, "Salt rotation failed — keeping current salt"),
            }
        }
    }

    /// Append events to the in-memory buffer, flushing if the threshold is reached.
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
    pub async fn flush_buffer(&self) {
        let batch: Vec<Event> = {
            let mut buf = self.buffer.lock().await;
            std::mem::take(&mut *buf)
        };
        if batch.is_empty() {
            return;
        }
        match self.db.insert_events(&batch).await {
            Ok(()) => info!(count = batch.len(), "Buffer flushed to DuckDB"),
            Err(e) => {
                error!(count = batch.len(), error = %e, "Buffer flush failed — events lost")
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
