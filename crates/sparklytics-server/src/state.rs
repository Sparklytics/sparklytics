use std::collections::HashSet;
use std::sync::Arc;

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
}

impl AppState {
    /// Construct a new `AppState` wrapping the given backend and config.
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
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
    /// (default 1 000 ms = 1 second, but Sprint 0 calls for 5 s timer;
    /// override via environment if needed).
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
