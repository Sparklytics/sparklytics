use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use chrono::{DateTime, Utc};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{error, info, warn};

use sparklytics_core::{
    analytics::{AnalyticsBackend, CampaignLink, TrackingPixel},
    billing::{BillingGate, NullBillingGate},
    config::Config,
    event::Event,
};
use sparklytics_duckdb::{website::Website, DuckDbBackend};

use crate::bot_detection::{BotOverrideDecision, BotPolicyInput};
use crate::error::AppError;
use crate::metadata::{duckdb::DuckDbMetadataStore, MetadataStore};

const SESSION_ID_PENDING: &str = "__pending__";
const DEFAULT_INGEST_QUEUE_MAX_EVENTS: usize = 100_000;
const DEFAULT_INGEST_DRAIN_MAX_EVENTS: usize = 5_000;
const DEFAULT_INGEST_DRAIN_MAX_BATCHES: usize = 128;
const DEFAULT_INGEST_RETRY_AFTER_SECONDS: u64 = 5;
const DEFAULT_INGEST_RETRY_BASE_MS: u64 = 200;
const DEFAULT_INGEST_RETRY_MAX_MS: u64 = 5_000;
const DEFAULT_SESSION_CACHE_MAX_ENTRIES: usize = 50_000;
const DEFAULT_SESSION_CACHE_TTL_SECONDS: i64 = 1_800;
const DEFAULT_RATE_LIMIT_MAX_KEYS: usize = 100_000;
const DEFAULT_ACQUISITION_CACHE_MAX_ENTRIES: usize = 10_000;
const DEFAULT_ACQUISITION_CACHE_TTL_SECONDS: u64 = 60;
const DEFAULT_COLLECT_CACHE_MAX_ENTRIES: usize = 100_000;
const DEFAULT_COLLECT_CACHE_TTL_SECONDS: u64 = 120;
const DEFAULT_EXPORT_CACHE_MAX_ENTRIES: usize = 2;
const DEFAULT_EXPORT_CACHE_TTL_SECONDS: u64 = 2;
const DEFAULT_EXPORT_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const INGEST_WAL_LOG_FILE: &str = "segment.log";
const INGEST_WAL_CURSOR_FILE: &str = "segment.cursor";

#[derive(Debug, Clone)]
struct IngestBatch {
    wal_end_offset: u64,
    events: Vec<Event>,
    retries: u8,
}

#[derive(Debug, Clone)]
struct CachedSession {
    session_id: String,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct CachedCampaignLink {
    value: CampaignLink,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedTrackingPixel {
    value: TrackingPixel,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedWebsiteMetadata {
    website: Option<Website>,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedBotPolicy {
    value: BotPolicyInput,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedBotOverride {
    value: Option<BotOverrideDecision>,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CachedExportResponse {
    value: Bytes,
    expires_at: Instant,
}

/// Shared application state injected into every Axum handler via
/// [`axum::extract::State`].
pub struct AppState {
    /// DuckDB backend used for self-hosted metadata operations.
    pub db: Arc<DuckDbBackend>,

    /// Scheduler DuckDB backend for background jobs. Can be dedicated via
    /// `SPARKLYTICS_SCHEDULER_DEDICATED_DUCKDB=1`, otherwise reuses `db`.
    pub scheduler_db: Arc<DuckDbBackend>,

    /// Analytics backend used by all analytics routes and buffer flush.
    pub analytics: Arc<dyn AnalyticsBackend>,

    /// Metadata backend used by website/auth/share metadata routes.
    pub metadata: Arc<dyn MetadataStore>,

    /// Parsed configuration, loaded once at startup.
    pub config: Arc<Config>,

    /// In-memory event buffer.
    pub buffer: Arc<Mutex<Vec<Event>>>,

    /// Fast in-process cache of known-valid `website_id` values.
    pub website_cache: Arc<RwLock<HashSet<String>>>,

    /// Per-IP sliding-window rate limiter for POST /api/collect.
    rate_limiter: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    rate_limiter_max_entries: usize,
    campaign_link_cache: Arc<Mutex<HashMap<String, CachedCampaignLink>>>,
    tracking_pixel_cache: Arc<Mutex<HashMap<String, CachedTrackingPixel>>>,
    acquisition_cache_max_entries: usize,
    acquisition_cache_ttl: Duration,
    website_metadata_cache: Arc<Mutex<HashMap<String, CachedWebsiteMetadata>>>,
    bot_policy_cache: Arc<Mutex<HashMap<String, CachedBotPolicy>>>,
    bot_override_cache: Arc<Mutex<HashMap<(String, String, String), CachedBotOverride>>>,
    collect_cache_max_entries: usize,
    collect_cache_ttl: Duration,
    export_cache: Arc<Mutex<HashMap<String, CachedExportResponse>>>,
    export_cache_compute_lock: Arc<Mutex<()>>,
    export_cache_max_entries: usize,
    export_cache_ttl: Duration,
    export_cache_max_bytes: usize,

    /// Plan-limit gate.
    pub billing_gate: Arc<dyn BillingGate>,

    /// Serialize export requests to avoid prolonged DuckDB contention.
    pub export_semaphore: Arc<Semaphore>,

    /// Bound concurrent funnel-results execution to avoid queue buildup.
    pub funnel_results_semaphore: Arc<Semaphore>,

    /// Bound concurrent journey execution to avoid queue buildup.
    pub journey_semaphore: Arc<Semaphore>,

    /// Bound concurrent retention execution to avoid queue buildup.
    pub retention_semaphore: Arc<Semaphore>,

    /// Guard to avoid scheduling overlapping background flush tasks.
    flush_in_progress: Arc<AtomicBool>,

    /// Durable ingest WAL queue.
    ingest_queue: Arc<Mutex<VecDeque<IngestBatch>>>,
    ingest_queue_events: Arc<AtomicUsize>,
    ingest_queue_max_events: usize,
    ingest_drain_max_events: usize,
    ingest_drain_max_batches: usize,
    ingest_retry_after_seconds: u64,
    ingest_retry_base_ms: u64,
    ingest_retry_max_ms: u64,
    ingest_wal_log_path: PathBuf,
    ingest_wal_cursor_path: PathBuf,
    ingest_wal_append_lock: Arc<Mutex<()>>,
    ingest_wal_next_offset: Arc<AtomicU64>,
    ingest_wal_cursor_offset: Arc<AtomicU64>,
    session_cache: Arc<Mutex<HashMap<(String, String), CachedSession>>>,
    session_cache_max_entries: usize,
    session_cache_ttl: chrono::Duration,
    ingest_worker_running: Arc<AtomicBool>,
    ingest_drain_lock: Arc<Mutex<()>>,
}

impl AppState {
    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default)
    }

    fn env_u64(name: &str, default: u64) -> u64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default)
    }

    fn env_i64(name: &str, default: i64) -> i64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default)
    }

    fn env_bool(name: &str, default: bool) -> bool {
        std::env::var(name)
            .ok()
            .map(|v| {
                let trimmed = v.trim();
                trimmed.eq_ignore_ascii_case("1")
                    || trimmed.eq_ignore_ascii_case("true")
                    || trimmed.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(default)
    }

    /// Constructor for self-hosted mode.
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        let ingest_wal_dir = PathBuf::from(&config.data_dir).join("ingest-wal");
        if let Err(e) = std::fs::create_dir_all(&ingest_wal_dir) {
            warn!(error = %e, path = %ingest_wal_dir.display(), "Failed to create ingest WAL directory");
        }
        let ingest_queue_max_events = Self::env_usize(
            "SPARKLYTICS_INGEST_QUEUE_MAX_EVENTS",
            DEFAULT_INGEST_QUEUE_MAX_EVENTS,
        );
        let ingest_drain_max_events = Self::env_usize(
            "SPARKLYTICS_INGEST_DRAIN_MAX_EVENTS",
            DEFAULT_INGEST_DRAIN_MAX_EVENTS,
        );
        let ingest_drain_max_batches = Self::env_usize(
            "SPARKLYTICS_INGEST_DRAIN_MAX_BATCHES",
            DEFAULT_INGEST_DRAIN_MAX_BATCHES,
        );
        let ingest_retry_after_seconds = Self::env_u64(
            "SPARKLYTICS_INGEST_RETRY_AFTER_SECONDS",
            DEFAULT_INGEST_RETRY_AFTER_SECONDS,
        );
        let ingest_retry_base_ms = Self::env_u64(
            "SPARKLYTICS_INGEST_RETRY_BASE_MS",
            DEFAULT_INGEST_RETRY_BASE_MS,
        );
        let ingest_retry_max_ms = Self::env_u64(
            "SPARKLYTICS_INGEST_RETRY_MAX_MS",
            DEFAULT_INGEST_RETRY_MAX_MS,
        );
        let session_cache_max_entries = Self::env_usize(
            "SPARKLYTICS_SESSION_CACHE_MAX_ENTRIES",
            DEFAULT_SESSION_CACHE_MAX_ENTRIES,
        );
        let session_cache_ttl_seconds = Self::env_i64(
            "SPARKLYTICS_SESSION_CACHE_TTL_SECONDS",
            DEFAULT_SESSION_CACHE_TTL_SECONDS,
        );
        let rate_limiter_max_entries = Self::env_usize(
            "SPARKLYTICS_RATE_LIMIT_MAX_KEYS",
            DEFAULT_RATE_LIMIT_MAX_KEYS,
        );
        let acquisition_cache_max_entries = Self::env_usize(
            "SPARKLYTICS_ACQUISITION_CACHE_MAX_ENTRIES",
            DEFAULT_ACQUISITION_CACHE_MAX_ENTRIES,
        );
        let acquisition_cache_ttl_seconds = Self::env_u64(
            "SPARKLYTICS_ACQUISITION_CACHE_TTL_SECONDS",
            DEFAULT_ACQUISITION_CACHE_TTL_SECONDS,
        );
        let collect_cache_max_entries = Self::env_usize(
            "SPARKLYTICS_COLLECT_CACHE_MAX_ENTRIES",
            DEFAULT_COLLECT_CACHE_MAX_ENTRIES,
        );
        let collect_cache_ttl_seconds = Self::env_u64(
            "SPARKLYTICS_COLLECT_CACHE_TTL_SECONDS",
            DEFAULT_COLLECT_CACHE_TTL_SECONDS,
        );
        let export_cache_max_entries = Self::env_usize(
            "SPARKLYTICS_EXPORT_CACHE_MAX_ENTRIES",
            DEFAULT_EXPORT_CACHE_MAX_ENTRIES,
        );
        let export_cache_ttl_seconds = Self::env_u64(
            "SPARKLYTICS_EXPORT_CACHE_TTL_SECONDS",
            DEFAULT_EXPORT_CACHE_TTL_SECONDS,
        );
        let export_cache_max_bytes = Self::env_usize(
            "SPARKLYTICS_EXPORT_CACHE_MAX_BYTES",
            DEFAULT_EXPORT_CACHE_MAX_BYTES,
        );
        let ingest_wal_log_path = ingest_wal_dir.join(INGEST_WAL_LOG_FILE);
        let ingest_wal_cursor_path = ingest_wal_dir.join(INGEST_WAL_CURSOR_FILE);

        if let Err(e) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ingest_wal_log_path)
        {
            warn!(
                error = %e,
                path = %ingest_wal_log_path.display(),
                "Failed to initialize ingest WAL log file"
            );
        }
        if !ingest_wal_cursor_path.exists() {
            if let Err(e) = std::fs::write(&ingest_wal_cursor_path, "0") {
                warn!(
                    error = %e,
                    path = %ingest_wal_cursor_path.display(),
                    "Failed to initialize ingest WAL cursor file"
                );
            }
        }
        let ingest_wal_next_offset = std::fs::metadata(&ingest_wal_log_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let ingest_wal_cursor_offset = std::fs::read_to_string(&ingest_wal_cursor_path)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
            .min(ingest_wal_next_offset);

        let db_path = format!("{}/sparklytics.db", config.data_dir);
        let db = Arc::new(db);
        // Default to a dedicated scheduler connection in normal builds to reduce
        // contention with ingest/query traffic. Tests default to shared DB for
        // deterministic visibility unless explicitly overridden.
        let scheduler_db = if Self::env_bool("SPARKLYTICS_SCHEDULER_DEDICATED_DUCKDB", !cfg!(test))
        {
            match DuckDbBackend::open(&db_path, &config.duckdb_memory_limit) {
                Ok(backend) => Arc::new(backend),
                Err(err) => {
                    warn!(
                        error = %err,
                        db_path = %db_path,
                        "Failed to open dedicated scheduler DB connection; falling back to primary"
                    );
                    Arc::clone(&db)
                }
            }
        } else {
            Arc::clone(&db)
        };
        let analytics: Arc<dyn AnalyticsBackend> = db.clone();
        let metadata: Arc<dyn MetadataStore> = Arc::new(DuckDbMetadataStore::new(Arc::clone(&db)));
        Self {
            db,
            scheduler_db,
            analytics,
            metadata,
            config: Arc::new(config),
            buffer: Arc::new(Mutex::new(Vec::new())),
            website_cache: Arc::new(RwLock::new(HashSet::new())),
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            rate_limiter_max_entries,
            campaign_link_cache: Arc::new(Mutex::new(HashMap::new())),
            tracking_pixel_cache: Arc::new(Mutex::new(HashMap::new())),
            acquisition_cache_max_entries,
            acquisition_cache_ttl: Duration::from_secs(acquisition_cache_ttl_seconds),
            website_metadata_cache: Arc::new(Mutex::new(HashMap::new())),
            bot_policy_cache: Arc::new(Mutex::new(HashMap::new())),
            bot_override_cache: Arc::new(Mutex::new(HashMap::new())),
            collect_cache_max_entries,
            collect_cache_ttl: Duration::from_secs(collect_cache_ttl_seconds),
            export_cache: Arc::new(Mutex::new(HashMap::new())),
            export_cache_compute_lock: Arc::new(Mutex::new(())),
            export_cache_max_entries,
            export_cache_ttl: Duration::from_secs(export_cache_ttl_seconds),
            export_cache_max_bytes,
            billing_gate: Arc::new(NullBillingGate),
            export_semaphore: Arc::new(Semaphore::new(1)),
            funnel_results_semaphore: Arc::new(Semaphore::new(1)),
            journey_semaphore: Arc::new(Semaphore::new(2)),
            retention_semaphore: Arc::new(Semaphore::new(2)),
            flush_in_progress: Arc::new(AtomicBool::new(false)),
            ingest_queue: Arc::new(Mutex::new(VecDeque::new())),
            ingest_queue_events: Arc::new(AtomicUsize::new(0)),
            ingest_queue_max_events,
            ingest_drain_max_events,
            ingest_drain_max_batches,
            ingest_retry_after_seconds,
            ingest_retry_base_ms,
            ingest_retry_max_ms,
            ingest_wal_log_path,
            ingest_wal_cursor_path,
            ingest_wal_append_lock: Arc::new(Mutex::new(())),
            ingest_wal_next_offset: Arc::new(AtomicU64::new(ingest_wal_next_offset)),
            ingest_wal_cursor_offset: Arc::new(AtomicU64::new(ingest_wal_cursor_offset)),
            session_cache: Arc::new(Mutex::new(HashMap::new())),
            session_cache_max_entries,
            session_cache_ttl: chrono::Duration::seconds(session_cache_ttl_seconds),
            ingest_worker_running: Arc::new(AtomicBool::new(false)),
            ingest_drain_lock: Arc::new(Mutex::new(())),
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
        // Custom-backend mode (cloud) may use a non-default DB filename.
        // Reuse the already-opened primary DB handle for scheduler reads/writes
        // so background jobs operate on the same data file.
        s.scheduler_db = Arc::clone(&s.db);
        s.analytics = analytics;
        s.billing_gate = billing_gate;
        s
    }

    /// Construct state with explicit analytics, metadata and billing backends.
    pub fn new_with_backends_and_metadata(
        db: DuckDbBackend,
        config: Config,
        analytics: Arc<dyn AnalyticsBackend>,
        metadata: Arc<dyn MetadataStore>,
        billing_gate: Arc<dyn BillingGate>,
    ) -> Self {
        let mut s = Self::new(db, config);
        s.scheduler_db = Arc::clone(&s.db);
        s.analytics = analytics;
        s.metadata = metadata;
        s.billing_gate = billing_gate;
        s
    }

    /// Replay persisted ingest batches from `data_dir/ingest-wal`.
    pub async fn restore_ingest_queue_from_wal(self: &Arc<Self>) {
        let mut wal_bytes = match tokio::fs::read(&self.ingest_wal_log_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!(
                    error = %e,
                    path = %self.ingest_wal_log_path.display(),
                    "Could not read ingest WAL log"
                );
                self.ingest_wal_next_offset.store(0, Ordering::Release);
                self.persist_ingest_wal_cursor(0).await;
                return;
            }
        };

        let wal_len = wal_bytes.len() as u64;
        let mut cursor = self
            .ingest_wal_cursor_offset
            .load(Ordering::Acquire)
            .min(wal_len);
        if cursor != self.ingest_wal_cursor_offset.load(Ordering::Acquire) {
            warn!(
                cursor,
                wal_len, "Ingest WAL cursor exceeded log length; clamping cursor"
            );
            self.persist_ingest_wal_cursor(cursor).await;
        }

        let mut restored_batches = 0usize;
        let mut restored_events = 0usize;
        let mut offset = cursor as usize;
        let mut truncate_tail = false;

        while offset.saturating_add(4) <= wal_bytes.len() {
            let record_len =
                u32::from_le_bytes(wal_bytes[offset..offset + 4].try_into().unwrap()) as usize;
            let payload_start = offset + 4;
            let payload_end = payload_start.saturating_add(record_len);
            if payload_end > wal_bytes.len() {
                warn!(
                    offset,
                    record_len,
                    wal_len = wal_bytes.len(),
                    "Ingest WAL contains a partial trailing record; truncating tail"
                );
                truncate_tail = true;
                break;
            }

            let events: Vec<Event> =
                match serde_json::from_slice(&wal_bytes[payload_start..payload_end]) {
                    Ok(events) => events,
                    Err(e) => {
                        warn!(
                            error = %e,
                            offset,
                            record_len,
                            "Invalid ingest WAL record payload; skipping"
                        );
                        offset = payload_end;
                        cursor = offset as u64;
                        self.persist_ingest_wal_cursor(cursor).await;
                        continue;
                    }
                };

            if events.is_empty() {
                offset = payload_end;
                cursor = offset as u64;
                self.persist_ingest_wal_cursor(cursor).await;
                continue;
            }

            let event_count = events.len();
            let current = self.ingest_queue_events.load(Ordering::Acquire);
            if current.saturating_add(event_count) > self.ingest_queue_max_events {
                warn!(
                    event_count,
                    queue_capacity = self.ingest_queue_max_events,
                    "Skipping WAL replay batch because in-memory ingest queue cap would be exceeded"
                );
                break;
            }

            {
                let mut queue = self.ingest_queue.lock().await;
                queue.push_back(IngestBatch {
                    wal_end_offset: payload_end as u64,
                    events,
                    retries: 0,
                });
            }
            self.ingest_queue_events
                .fetch_add(event_count, Ordering::AcqRel);
            restored_batches += 1;
            restored_events += event_count;
            offset = payload_end;
        }

        if truncate_tail && offset < wal_bytes.len() {
            wal_bytes.truncate(offset);
            match tokio::fs::OpenOptions::new()
                .write(true)
                .open(&self.ingest_wal_log_path)
                .await
            {
                Ok(file) => {
                    if let Err(e) = file.set_len(offset as u64).await {
                        warn!(error = %e, "Failed to truncate ingest WAL log tail");
                    }
                }
                Err(e) => warn!(error = %e, "Failed to open ingest WAL log for truncation"),
            }
        }
        let effective_wal_len = if truncate_tail {
            offset as u64
        } else {
            wal_len
        };
        self.ingest_wal_next_offset
            .store(effective_wal_len, Ordering::Release);

        if restored_batches > 0 {
            info!(
                restored_batches,
                restored_events, "Restored ingest batches from WAL"
            );
            self.schedule_ingest_worker();
        }
    }

    pub fn pending_session_marker() -> &'static str {
        SESSION_ID_PENDING
    }

    pub fn ingest_queue_capacity(&self) -> usize {
        self.ingest_queue_max_events
    }

    async fn persist_ingest_wal_cursor(&self, offset: u64) {
        self.ingest_wal_cursor_offset
            .store(offset, Ordering::Release);
        if let Err(e) = tokio::fs::write(&self.ingest_wal_cursor_path, offset.to_string()).await {
            warn!(
                error = %e,
                path = %self.ingest_wal_cursor_path.display(),
                "Failed to persist ingest WAL cursor"
            );
        }
    }

    async fn append_ingest_wal_record(&self, events: &[Event]) -> Result<u64, AppError> {
        let payload = serde_json::to_vec(events).map_err(|e| AppError::Internal(e.into()))?;
        if payload.len() > u32::MAX as usize {
            return Err(AppError::Internal(anyhow::anyhow!(
                "ingest wal record too large: {} bytes",
                payload.len()
            )));
        }

        let mut record = Vec::with_capacity(4 + payload.len());
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);

        let _append_guard = self.ingest_wal_append_lock.lock().await;
        let start_offset = self.ingest_wal_next_offset.load(Ordering::Acquire);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ingest_wal_log_path)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("failed to open ingest wal log: {e}"))
            })?;
        file.write_all(&record).await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("failed to append ingest wal record: {e}"))
        })?;
        let end_offset = start_offset.saturating_add(record.len() as u64);
        self.ingest_wal_next_offset
            .store(end_offset, Ordering::Release);
        Ok(end_offset)
    }

    fn try_reserve_ingest_capacity(&self, event_count: usize) -> bool {
        self.ingest_queue_events
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                let next = current.saturating_add(event_count);
                if next > self.ingest_queue_max_events {
                    None
                } else {
                    Some(next)
                }
            })
            .is_ok()
    }

    fn release_ingest_capacity(&self, event_count: usize) {
        self.ingest_queue_events
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                Some(v.saturating_sub(event_count))
            })
            .ok();
    }

    async fn maybe_compact_ingest_wal(&self) {
        if self.ingest_queue_events.load(Ordering::Acquire) > 0 {
            return;
        }
        let _append_guard = self.ingest_wal_append_lock.lock().await;
        if self.ingest_queue_events.load(Ordering::Acquire) > 0 {
            return;
        }
        let cursor = self.ingest_wal_cursor_offset.load(Ordering::Acquire);
        let next = self.ingest_wal_next_offset.load(Ordering::Acquire);
        if cursor < next {
            return;
        }

        match tokio::fs::OpenOptions::new()
            .write(true)
            .open(&self.ingest_wal_log_path)
            .await
        {
            Ok(file) => {
                if let Err(e) = file.set_len(0).await {
                    warn!(
                        error = %e,
                        path = %self.ingest_wal_log_path.display(),
                        "Failed to compact ingest WAL log"
                    );
                    return;
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    path = %self.ingest_wal_log_path.display(),
                    "Failed to open ingest WAL log for compaction"
                );
                return;
            }
        }

        self.ingest_wal_next_offset.store(0, Ordering::Release);
        self.persist_ingest_wal_cursor(0).await;
    }

    async fn get_cached_session(
        &self,
        website_id: &str,
        visitor_id: &str,
        at: DateTime<Utc>,
    ) -> Option<String> {
        let key = (website_id.to_string(), visitor_id.to_string());
        let mut cache = self.session_cache.lock().await;
        let entry = cache.get_mut(&key)?;

        if at
            .signed_duration_since(entry.last_seen_at)
            .gt(&self.session_cache_ttl)
        {
            cache.remove(&key);
            return None;
        }

        if at > entry.last_seen_at {
            entry.last_seen_at = at;
        }
        Some(entry.session_id.clone())
    }

    async fn put_cached_session(
        &self,
        website_id: String,
        visitor_id: String,
        session_id: String,
        last_seen_at: DateTime<Utc>,
    ) {
        let key = (website_id, visitor_id);
        let mut cache = self.session_cache.lock().await;

        if !cache.contains_key(&key) && cache.len() >= self.session_cache_max_entries {
            cache.retain(|_, v| {
                last_seen_at
                    .signed_duration_since(v.last_seen_at)
                    .le(&self.session_cache_ttl)
            });
            while cache.len() >= self.session_cache_max_entries {
                let Some(evict_key) = cache.keys().next().cloned() else {
                    break;
                };
                cache.remove(&evict_key);
            }
        }

        cache.insert(
            key,
            CachedSession {
                session_id,
                last_seen_at,
            },
        );
    }

    fn event_needs_session_resolution(event: &Event) -> bool {
        event.session_id.is_empty() || event.session_id == SESSION_ID_PENDING
    }

    async fn assign_pending_sessions(&self, events: &mut [Event]) -> anyhow::Result<()> {
        #[derive(Debug)]
        struct SessionAccumulator {
            session_id: String,
            count: u32,
            last_seen_at: DateTime<Utc>,
            base_count_already_recorded: bool,
            website_id: String,
            visitor_id: String,
            is_bot: bool,
            bot_score: i32,
            bot_reason: Option<String>,
        }

        let mut session_cache: HashMap<(String, String), SessionAccumulator> = HashMap::new();

        for event in events.iter_mut() {
            if !Self::event_needs_session_resolution(event) {
                continue;
            }

            let key = (event.website_id.clone(), event.visitor_id.clone());
            if let Some(existing) = session_cache.get_mut(&key) {
                existing.count = existing.count.saturating_add(1);
                if event.created_at > existing.last_seen_at {
                    existing.last_seen_at = event.created_at;
                }
                if event.bot_score >= existing.bot_score {
                    existing.bot_score = event.bot_score;
                    existing.bot_reason = event.bot_reason.clone();
                }
                existing.is_bot = existing.is_bot || event.is_bot;
                event.session_id = existing.session_id.clone();
                continue;
            }

            let mut base_count_already_recorded = false;
            let session_id = if let Some(cached_session_id) = self
                .get_cached_session(&event.website_id, &event.visitor_id, event.created_at)
                .await
            {
                cached_session_id
            } else {
                base_count_already_recorded = true;
                self.analytics
                    .get_or_create_session_at(
                        &event.website_id,
                        &event.visitor_id,
                        event.referrer_domain.as_deref(),
                        &event.url,
                        event.created_at,
                    )
                    .await?
            };
            session_cache.insert(
                key,
                SessionAccumulator {
                    session_id: session_id.clone(),
                    count: 1,
                    last_seen_at: event.created_at,
                    base_count_already_recorded,
                    website_id: event.website_id.clone(),
                    visitor_id: event.visitor_id.clone(),
                    is_bot: event.is_bot,
                    bot_score: event.bot_score,
                    bot_reason: event.bot_reason.clone(),
                },
            );
            event.session_id = session_id;
        }

        for entry in session_cache.into_values() {
            let additional_pageviews = if entry.base_count_already_recorded {
                entry.count.saturating_sub(1)
            } else {
                entry.count
            };

            if additional_pageviews > 0 {
                self.analytics
                    .increment_session_pageviews(
                        &entry.session_id,
                        additional_pageviews,
                        entry.last_seen_at,
                    )
                    .await?;
            }
            if entry.is_bot || entry.bot_score > 0 || entry.bot_reason.is_some() {
                self.analytics
                    .set_session_bot_classification(
                        &entry.session_id,
                        entry.is_bot,
                        entry.bot_score,
                        entry.bot_reason.as_deref(),
                    )
                    .await?;
            }

            self.put_cached_session(
                entry.website_id,
                entry.visitor_id,
                entry.session_id,
                entry.last_seen_at,
            )
            .await;
        }

        Ok(())
    }

    /// Check whether `ip` is within the 60 req/min rate limit.
    pub async fn check_rate_limit(&self, ip: &str) -> bool {
        self.check_rate_limit_with_max(ip, 60).await
    }

    pub async fn get_website_metadata_cached(
        &self,
        website_id: &str,
    ) -> anyhow::Result<Option<Website>> {
        let now = Instant::now();
        {
            let mut cache = self.website_metadata_cache.lock().await;
            if let Some(entry) = cache.get(website_id) {
                if entry.expires_at > now {
                    return Ok(entry.website.clone());
                }
                cache.remove(website_id);
            }
        }

        let website = self.metadata.get_website(website_id).await?;
        {
            let mut cache = self.website_metadata_cache.lock().await;
            if cache.len() >= self.collect_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                website_id.to_string(),
                CachedWebsiteMetadata {
                    website: website.clone(),
                    expires_at: now + self.collect_cache_ttl,
                },
            );
        }
        {
            let mut cache = self.website_cache.write().await;
            if website.is_some() {
                cache.insert(website_id.to_string());
            } else {
                cache.remove(website_id);
            }
        }
        Ok(website)
    }

    pub async fn cache_website_metadata(&self, website: Website) {
        let now = Instant::now();
        {
            let mut cache = self.website_metadata_cache.lock().await;
            if cache.len() >= self.collect_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                website.id.clone(),
                CachedWebsiteMetadata {
                    website: Some(website.clone()),
                    expires_at: now + self.collect_cache_ttl,
                },
            );
        }
        self.website_cache.write().await.insert(website.id);
    }

    pub async fn invalidate_website_metadata_cache(&self, website_id: &str) {
        self.website_cache.write().await.remove(website_id);
        self.website_metadata_cache.lock().await.remove(website_id);
        self.bot_policy_cache.lock().await.remove(website_id);
        self.bot_override_cache
            .lock()
            .await
            .retain(|(cached_website_id, _, _), _| cached_website_id != website_id);
    }

    pub async fn get_bot_policy_cached(&self, website_id: &str) -> anyhow::Result<BotPolicyInput> {
        let now = Instant::now();
        {
            let mut cache = self.bot_policy_cache.lock().await;
            if let Some(entry) = cache.get(website_id) {
                if entry.expires_at > now {
                    return Ok(entry.value.clone());
                }
                cache.remove(website_id);
            }
        }

        let policy = self.db.get_bot_policy(website_id).await?;
        let mode = policy.mode;
        let threshold_score = match mode {
            sparklytics_core::analytics::BotPolicyMode::Strict if policy.threshold_score <= 0 => 60,
            sparklytics_core::analytics::BotPolicyMode::Balanced
            | sparklytics_core::analytics::BotPolicyMode::Off
                if policy.threshold_score <= 0 =>
            {
                70
            }
            _ => policy.threshold_score,
        };
        let input = BotPolicyInput {
            mode,
            threshold_score,
        };
        {
            let mut cache = self.bot_policy_cache.lock().await;
            if cache.len() >= self.collect_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                website_id.to_string(),
                CachedBotPolicy {
                    value: input.clone(),
                    expires_at: now + self.collect_cache_ttl,
                },
            );
        }
        Ok(input)
    }

    pub async fn classify_override_for_request_cached(
        &self,
        website_id: &str,
        client_ip: &str,
        user_agent: &str,
    ) -> anyhow::Result<Option<BotOverrideDecision>> {
        let now = Instant::now();
        let key = (
            website_id.to_string(),
            client_ip.to_string(),
            user_agent.to_string(),
        );
        {
            let mut cache = self.bot_override_cache.lock().await;
            if let Some(entry) = cache.get(&key) {
                if entry.expires_at > now {
                    return Ok(entry.value.clone());
                }
                cache.remove(&key);
            }
        }

        let decision = self
            .db
            .classify_override_for_request(website_id, client_ip, user_agent)
            .await?
            .map(|is_bot| {
                if is_bot {
                    BotOverrideDecision::ForceBot
                } else {
                    BotOverrideDecision::ForceHuman
                }
            });

        {
            let mut cache = self.bot_override_cache.lock().await;
            if cache.len() >= self.collect_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                key,
                CachedBotOverride {
                    value: decision.clone(),
                    expires_at: now + self.collect_cache_ttl,
                },
            );
        }

        Ok(decision)
    }

    /// Returns the default query behavior for `include_bots`.
    /// When bot policy mode is `off`, queries include bot traffic by default.
    pub async fn default_include_bots(&self, website_id: &str) -> bool {
        match self.get_bot_policy_cached(website_id).await {
            Ok(policy) => matches!(policy.mode, sparklytics_core::analytics::BotPolicyMode::Off),
            Err(err) => {
                warn!(
                    website_id,
                    error = %err,
                    "failed to resolve bot policy default include_bots"
                );
                false
            }
        }
    }

    fn export_cache_enabled(&self) -> bool {
        self.export_cache_max_entries > 0
            && self.export_cache_max_bytes > 0
            && !self.export_cache_ttl.is_zero()
    }

    pub fn export_cache_key(&self, website_id: &str, start_date: &str, end_date: &str) -> String {
        format!("{website_id}|{start_date}|{end_date}|csv")
    }

    pub async fn lock_export_cache_compute(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.export_cache_compute_lock.lock().await
    }

    pub async fn get_cached_export_csv(&self, key: &str) -> Option<Bytes> {
        if !self.export_cache_enabled() {
            return None;
        }

        let now = Instant::now();
        let mut cache = self.export_cache.lock().await;
        if let Some(entry) = cache.get(key) {
            if entry.expires_at > now {
                return Some(entry.value.clone());
            }
            cache.remove(key);
        }
        None
    }

    pub async fn put_cached_export_csv(&self, key: String, value: Bytes) {
        if !self.export_cache_enabled() || value.len() > self.export_cache_max_bytes {
            return;
        }

        let now = Instant::now();
        let mut cache = self.export_cache.lock().await;
        if cache.len() >= self.export_cache_max_entries {
            cache.clear();
        }
        cache.insert(
            key,
            CachedExportResponse {
                value,
                expires_at: now + self.export_cache_ttl,
            },
        );
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
        if !map.contains_key(&key) && map.len() >= self.rate_limiter_max_entries {
            map.retain(|_, window| {
                while window.front().is_some_and(|t| *t < cutoff) {
                    window.pop_front();
                }
                !window.is_empty()
            });
            if !map.contains_key(&key) && map.len() >= self.rate_limiter_max_entries {
                return false;
            }
        }
        let window = map.entry(key).or_default();
        if window.len() >= max_per_min {
            return false;
        }
        window.push_back(Instant::now());
        true
    }

    pub async fn get_campaign_link_by_slug_cached(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<CampaignLink>> {
        let now = Instant::now();
        {
            let mut cache = self.campaign_link_cache.lock().await;
            if let Some(entry) = cache.get(slug) {
                if entry.expires_at > now {
                    return Ok(Some(entry.value.clone()));
                }
                cache.remove(slug);
            }
        }

        let link = self.analytics.get_campaign_link_by_slug(slug).await?;
        if let Some(link_value) = link.as_ref() {
            let mut cache = self.campaign_link_cache.lock().await;
            if cache.len() >= self.acquisition_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                slug.to_string(),
                CachedCampaignLink {
                    value: link_value.clone(),
                    expires_at: now + self.acquisition_cache_ttl,
                },
            );
        }
        Ok(link)
    }

    pub async fn get_tracking_pixel_by_key_cached(
        &self,
        pixel_key: &str,
    ) -> anyhow::Result<Option<TrackingPixel>> {
        let now = Instant::now();
        {
            let mut cache = self.tracking_pixel_cache.lock().await;
            if let Some(entry) = cache.get(pixel_key) {
                if entry.expires_at > now {
                    return Ok(Some(entry.value.clone()));
                }
                cache.remove(pixel_key);
            }
        }

        let pixel = self.analytics.get_tracking_pixel_by_key(pixel_key).await?;
        if let Some(pixel_value) = pixel.as_ref() {
            let mut cache = self.tracking_pixel_cache.lock().await;
            if cache.len() >= self.acquisition_cache_max_entries {
                cache.clear();
            }
            cache.insert(
                pixel_key.to_string(),
                CachedTrackingPixel {
                    value: pixel_value.clone(),
                    expires_at: now + self.acquisition_cache_ttl,
                },
            );
        }
        Ok(pixel)
    }

    pub async fn invalidate_acquisition_cache(&self) {
        self.campaign_link_cache.lock().await.clear();
        self.tracking_pixel_cache.lock().await.clear();
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
            self.schedule_flush();
        }
    }

    /// Persist events to WAL and enqueue them for async ingestion.
    pub async fn enqueue_ingest_events(
        self: &Arc<Self>,
        events: Vec<Event>,
    ) -> Result<(), AppError> {
        if events.is_empty() {
            return Ok(());
        }
        let event_count = events.len();

        if !self.try_reserve_ingest_capacity(event_count) {
            warn!(
                queued_events = self.ingest_queue_events.load(Ordering::Acquire),
                incoming_events = event_count,
                queue_capacity = self.ingest_queue_max_events,
                "Rejecting ingest batch because queue is overloaded"
            );
            return Err(AppError::IngestOverloaded {
                retry_after_seconds: self.ingest_retry_after_seconds,
            });
        }

        let wal_end_offset = match self.append_ingest_wal_record(&events).await {
            Ok(end) => end,
            Err(e) => {
                self.release_ingest_capacity(event_count);
                return Err(e);
            }
        };

        {
            let mut queue = self.ingest_queue.lock().await;
            queue.push_back(IngestBatch {
                wal_end_offset,
                events,
                retries: 0,
            });
        }
        self.schedule_ingest_worker();
        Ok(())
    }

    /// Schedule a background flush if one is not already running.
    fn schedule_flush(&self) {
        if self
            .flush_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let buffer = Arc::clone(&self.buffer);
        let analytics = Arc::clone(&self.analytics);
        let flush_in_progress = Arc::clone(&self.flush_in_progress);

        tokio::spawn(async move {
            loop {
                let batch: Vec<Event> = {
                    let mut buf = buffer.lock().await;
                    if buf.is_empty() {
                        Vec::new()
                    } else {
                        std::mem::take(&mut *buf)
                    }
                };

                if batch.is_empty() {
                    break;
                }

                match analytics.insert_events(&batch).await {
                    Ok(()) => info!(count = batch.len(), "Buffer flushed"),
                    Err(e) => {
                        error!(count = batch.len(), error = %e, "Buffer flush failed - events lost")
                    }
                }
            }

            flush_in_progress.store(false, Ordering::Release);
        });
    }

    fn schedule_ingest_worker(self: &Arc<Self>) {
        if self
            .ingest_worker_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let state = Arc::clone(self);
        tokio::spawn(async move {
            state.run_ingest_worker().await;
        });
    }

    async fn run_ingest_worker(self: Arc<Self>) {
        self.drain_ingest_queue(true).await;
        self.ingest_worker_running.store(false, Ordering::Release);

        // If new batches arrived during shutdown of this worker, run another cycle.
        if self.ingest_queue_events.load(Ordering::Acquire) > 0 {
            self.schedule_ingest_worker();
        }
    }

    async fn drain_ingest_queue(&self, retry_on_error: bool) {
        let _drain_guard = self.ingest_drain_lock.lock().await;

        loop {
            let mut batches = {
                let mut queue = self.ingest_queue.lock().await;
                let mut selected = Vec::new();
                let mut selected_events = 0usize;
                while selected.len() < self.ingest_drain_max_batches {
                    let Some(next) = queue.pop_front() else {
                        break;
                    };
                    selected_events += next.events.len();
                    selected.push(next);
                    if selected_events >= self.ingest_drain_max_events {
                        break;
                    }
                }
                selected
            };

            if batches.is_empty() {
                break;
            }

            let total_events: usize = batches.iter().map(|b| b.events.len()).sum();
            let max_retries = batches.iter().map(|b| b.retries).max().unwrap_or(0);
            let mut merged_events = Vec::with_capacity(total_events);
            for batch in &batches {
                merged_events.extend(batch.events.iter().cloned());
            }

            let mut persist_result = self.assign_pending_sessions(&mut merged_events).await;
            if persist_result.is_ok() {
                persist_result = self.analytics.insert_events(&merged_events).await;
            }

            match persist_result {
                Ok(()) => {
                    let wal_end_offset = batches
                        .last()
                        .map(|b| b.wal_end_offset)
                        .unwrap_or_else(|| self.ingest_wal_cursor_offset.load(Ordering::Acquire));
                    self.release_ingest_capacity(total_events);
                    self.persist_ingest_wal_cursor(wal_end_offset).await;
                    self.maybe_compact_ingest_wal().await;
                }
                Err(e) => {
                    let retry_delay_ms = (self
                        .ingest_retry_base_ms
                        .saturating_mul(2u64.saturating_pow(max_retries.saturating_add(1) as u32)))
                    .min(self.ingest_retry_max_ms);
                    error!(
                        error = %e,
                        retries = max_retries.saturating_add(1),
                        delay_ms = retry_delay_ms,
                        batch_count = batches.len(),
                        event_count = total_events,
                        "Ingest queue persist failed, will retry"
                    );
                    {
                        let mut queue = self.ingest_queue.lock().await;
                        for mut batch in batches.drain(..).rev() {
                            batch.retries = batch.retries.saturating_add(1);
                            queue.push_front(batch);
                        }
                    }

                    if retry_on_error {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_delay_ms)).await;
                        continue;
                    }
                    break;
                }
            }
        }
    }

    /// Drain the buffer and write all pending events through the analytics backend.
    pub async fn flush_buffer(&self) {
        self.drain_ingest_queue(false).await;

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

    pub fn queued_ingest_events(&self) -> usize {
        self.ingest_queue_events.load(Ordering::Acquire)
    }

    /// Return `true` if the `website_id` is known to exist (cache + DB).
    pub async fn is_valid_website(&self, website_id: &str) -> bool {
        {
            let cache = self.website_cache.read().await;
            if cache.contains(website_id) {
                return true;
            }
        }
        match self.get_website_metadata_cached(website_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                error!(website_id, error = %e, "website metadata lookup failed");
                false
            }
        }
    }
}
