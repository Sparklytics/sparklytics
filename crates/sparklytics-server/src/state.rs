use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
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
use sparklytics_duckdb::DuckDbBackend;
use sparklytics_metadata::Website;

use crate::bot_detection::{BotOverrideDecision, BotPolicyInput};
use crate::error::AppError;
use crate::metadata::{duckdb::DuckDbMetadataStore, MetadataStore};

const SESSION_ID_PENDING: &str = "__pending__";
const DEFAULT_INGEST_QUEUE_MAX_EVENTS: usize = 100_000;
const DEFAULT_INGEST_DRAIN_MAX_EVENTS: usize = 5_000;
const DEFAULT_INGEST_DRAIN_MAX_BATCHES: usize = 128;
const DEFAULT_INGEST_RETRY_BASE_MS: u64 = 200;
const DEFAULT_INGEST_RETRY_MAX_MS: u64 = 5_000;
const DEFAULT_INGEST_RETRY_MAX_ATTEMPTS: u8 = 8;
const DEFAULT_SESSION_CACHE_MAX_ENTRIES: usize = 50_000;
const DEFAULT_SESSION_CACHE_TTL_SECONDS: i64 = 1_800;
const DEFAULT_RATE_LIMIT_MAX_KEYS: usize = 100_000;
const DEFAULT_ACQUISITION_CACHE_MAX_ENTRIES: usize = 10_000;
const DEFAULT_ACQUISITION_CACHE_TTL_SECONDS: u64 = 60;
const DEFAULT_COLLECT_CACHE_MAX_ENTRIES: usize = 100_000;
const DEFAULT_COLLECT_CACHE_TTL_SECONDS: u64 = 120;
const DEFAULT_BOT_THRESHOLD_STRICT: i32 = 60;
const DEFAULT_BOT_THRESHOLD_BALANCED_OR_OFF: i32 = 70;
const DEFAULT_EXPORT_CACHE_MAX_ENTRIES: usize = 2;
const DEFAULT_EXPORT_CACHE_TTL_SECONDS: u64 = 2;
const DEFAULT_EXPORT_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_WEBSITE_INGEST_PEAK_EPS: usize = 10_000;
const DEFAULT_WEBSITE_INGEST_QUEUE_MAX_EVENTS: usize = 100_000;
const INGEST_WAL_LOG_FILE: &str = "segment.log";
const INGEST_WAL_CURSOR_FILE: &str = "segment.cursor";

#[derive(Debug, Clone)]
struct IngestBatch {
    wal_start_offset: u64,
    wal_end_offset: u64,
    events: Vec<Event>,
    website_event_counts: Vec<(String, usize)>,
    retries: u8,
}

#[derive(Debug, Clone, Copy)]
struct IngestWalRecordOffsets {
    start_offset: u64,
    end_offset: u64,
}

#[derive(Debug, Clone)]
struct CachedSession {
    session_id: String,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SessionResolvedEffect {
    session_id: String,
    additional_pageviews: u32,
    last_seen_at: DateTime<Utc>,
    website_id: String,
    visitor_id: String,
    is_bot: bool,
    bot_score: i32,
    bot_reason: Option<String>,
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

type BotOverrideCacheKey = (String, String, u64);
type BotOverrideCache = HashMap<BotOverrideCacheKey, CachedBotOverride>;

#[derive(Debug, Clone)]
struct CachedExportResponse {
    value: Bytes,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct RuntimeTuning {
    ingest_queue_max_events: usize,
    ingest_drain_max_events: usize,
    ingest_drain_max_batches: usize,
    ingest_retry_base_ms: u64,
    ingest_retry_max_ms: u64,
    ingest_retry_max_attempts: u8,
    session_cache_max_entries: usize,
    rate_limiter_max_entries: usize,
    acquisition_cache_max_entries: usize,
    acquisition_cache_ttl_seconds: u64,
    collect_cache_max_entries: usize,
    collect_cache_ttl_seconds: u64,
    export_cache_max_entries: usize,
    export_cache_ttl_seconds: u64,
    export_cache_max_bytes: usize,
    website_ingest_peak_eps_default: usize,
    website_ingest_queue_max_events_default: usize,
}

#[derive(Debug, Clone)]
struct IngestWalInit {
    log_path: PathBuf,
    cursor_path: PathBuf,
    next_offset: u64,
    cursor_offset: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct IngestEnqueueOutcome {
    pub accepted_events: usize,
    pub dropped_events: usize,
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
    event_rate_limiter: Arc<Mutex<HashMap<String, VecDeque<(Instant, usize)>>>>,
    rate_limiter_max_entries: usize,
    campaign_link_cache: Arc<Mutex<HashMap<String, CachedCampaignLink>>>,
    tracking_pixel_cache: Arc<Mutex<HashMap<String, CachedTrackingPixel>>>,
    acquisition_cache_max_entries: usize,
    acquisition_cache_ttl: Duration,
    website_metadata_cache: Arc<Mutex<HashMap<String, CachedWebsiteMetadata>>>,
    bot_policy_cache: Arc<Mutex<HashMap<String, CachedBotPolicy>>>,
    bot_override_cache: Arc<Mutex<BotOverrideCache>>,
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
    website_ingest_queue_events: Arc<Mutex<HashMap<String, usize>>>,
    ingest_queue_max_events: usize,
    website_ingest_peak_eps_default: usize,
    website_ingest_queue_max_events_default: usize,
    ingest_drain_max_events: usize,
    ingest_drain_max_batches: usize,
    ingest_retry_base_ms: u64,
    ingest_retry_max_ms: u64,
    ingest_retry_max_attempts: u8,
    ingest_wal_log_path: PathBuf,
    ingest_wal_cursor_path: PathBuf,
    ingest_wal_append_lock: Arc<Mutex<()>>,
    ingest_wal_next_offset: Arc<AtomicU64>,
    ingest_wal_cursor_offset: Arc<AtomicU64>,
    session_cache: Arc<Mutex<HashMap<(String, String), CachedSession>>>,
    session_cache_max_entries: usize,
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

    fn env_u8(name: &str, default: u8) -> u8 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
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

    fn runtime_tuning() -> RuntimeTuning {
        RuntimeTuning {
            ingest_queue_max_events: Self::env_usize(
                "SPARKLYTICS_INGEST_QUEUE_MAX_EVENTS",
                DEFAULT_INGEST_QUEUE_MAX_EVENTS,
            ),
            ingest_drain_max_events: Self::env_usize(
                "SPARKLYTICS_INGEST_DRAIN_MAX_EVENTS",
                DEFAULT_INGEST_DRAIN_MAX_EVENTS,
            ),
            ingest_drain_max_batches: Self::env_usize(
                "SPARKLYTICS_INGEST_DRAIN_MAX_BATCHES",
                DEFAULT_INGEST_DRAIN_MAX_BATCHES,
            ),
            ingest_retry_base_ms: Self::env_u64(
                "SPARKLYTICS_INGEST_RETRY_BASE_MS",
                DEFAULT_INGEST_RETRY_BASE_MS,
            ),
            ingest_retry_max_ms: Self::env_u64(
                "SPARKLYTICS_INGEST_RETRY_MAX_MS",
                DEFAULT_INGEST_RETRY_MAX_MS,
            ),
            ingest_retry_max_attempts: Self::env_u8(
                "SPARKLYTICS_INGEST_RETRY_MAX_ATTEMPTS",
                DEFAULT_INGEST_RETRY_MAX_ATTEMPTS,
            ),
            session_cache_max_entries: Self::env_usize(
                "SPARKLYTICS_SESSION_CACHE_MAX_ENTRIES",
                DEFAULT_SESSION_CACHE_MAX_ENTRIES,
            ),
            rate_limiter_max_entries: Self::env_usize(
                "SPARKLYTICS_RATE_LIMIT_MAX_KEYS",
                DEFAULT_RATE_LIMIT_MAX_KEYS,
            ),
            acquisition_cache_max_entries: Self::env_usize(
                "SPARKLYTICS_ACQUISITION_CACHE_MAX_ENTRIES",
                DEFAULT_ACQUISITION_CACHE_MAX_ENTRIES,
            ),
            acquisition_cache_ttl_seconds: Self::env_u64(
                "SPARKLYTICS_ACQUISITION_CACHE_TTL_SECONDS",
                DEFAULT_ACQUISITION_CACHE_TTL_SECONDS,
            ),
            collect_cache_max_entries: Self::env_usize(
                "SPARKLYTICS_COLLECT_CACHE_MAX_ENTRIES",
                DEFAULT_COLLECT_CACHE_MAX_ENTRIES,
            ),
            collect_cache_ttl_seconds: Self::env_u64(
                "SPARKLYTICS_COLLECT_CACHE_TTL_SECONDS",
                DEFAULT_COLLECT_CACHE_TTL_SECONDS,
            ),
            export_cache_max_entries: Self::env_usize(
                "SPARKLYTICS_EXPORT_CACHE_MAX_ENTRIES",
                DEFAULT_EXPORT_CACHE_MAX_ENTRIES,
            ),
            export_cache_ttl_seconds: Self::env_u64(
                "SPARKLYTICS_EXPORT_CACHE_TTL_SECONDS",
                DEFAULT_EXPORT_CACHE_TTL_SECONDS,
            ),
            export_cache_max_bytes: Self::env_usize(
                "SPARKLYTICS_EXPORT_CACHE_MAX_BYTES",
                DEFAULT_EXPORT_CACHE_MAX_BYTES,
            ),
            website_ingest_peak_eps_default: Self::env_usize(
                "SPARKLYTICS_INGEST_WEBSITE_PEAK_EPS_DEFAULT",
                DEFAULT_WEBSITE_INGEST_PEAK_EPS,
            ),
            website_ingest_queue_max_events_default: Self::env_usize(
                "SPARKLYTICS_INGEST_WEBSITE_QUEUE_MAX_EVENTS_DEFAULT",
                DEFAULT_WEBSITE_INGEST_QUEUE_MAX_EVENTS,
            ),
        }
    }

    fn prepare_ingest_wal(data_dir: &str) -> IngestWalInit {
        let ingest_wal_dir = PathBuf::from(data_dir).join("ingest-wal");
        if let Err(e) = std::fs::create_dir_all(&ingest_wal_dir) {
            warn!(error = %e, path = %ingest_wal_dir.display(), "Failed to create ingest WAL directory");
        }

        let log_path = ingest_wal_dir.join(INGEST_WAL_LOG_FILE);
        let cursor_path = ingest_wal_dir.join(INGEST_WAL_CURSOR_FILE);

        if let Err(e) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            warn!(
                error = %e,
                path = %log_path.display(),
                "Failed to initialize ingest WAL log file"
            );
        }

        if !cursor_path.exists() {
            if let Err(e) = std::fs::write(&cursor_path, "0") {
                warn!(
                    error = %e,
                    path = %cursor_path.display(),
                    "Failed to initialize ingest WAL cursor file"
                );
            }
        }

        let next_offset = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
        let cursor_offset = std::fs::read_to_string(&cursor_path)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
            .min(next_offset);

        IngestWalInit {
            log_path,
            cursor_path,
            next_offset,
            cursor_offset,
        }
    }

    fn hash_user_agent(user_agent: &str) -> u64 {
        // FNV-1a keeps the cache key compact while remaining deterministic and cheap.
        let mut hash = 14695981039346656037_u64;
        for byte in user_agent.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(1099511628211_u64);
        }
        hash
    }

    fn evict_cache_entries<K, V, F>(
        cache: &mut HashMap<K, V>,
        max_entries: usize,
        now: Instant,
        expires_at: F,
    ) where
        K: Clone + Eq + Hash,
        F: Fn(&V) -> Instant,
    {
        if max_entries == 0 {
            cache.clear();
            return;
        }
        if cache.len() < max_entries {
            return;
        }

        cache.retain(|_, entry| expires_at(entry) > now);
        while cache.len() >= max_entries {
            let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, entry)| expires_at(entry))
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            cache.remove(&oldest_key);
        }
    }

    /// Constructor for self-hosted mode.
    pub fn new(db: DuckDbBackend, config: Config) -> Self {
        let tuning = Self::runtime_tuning();
        let ingest_wal = Self::prepare_ingest_wal(&config.data_dir);

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
            event_rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            rate_limiter_max_entries: tuning.rate_limiter_max_entries,
            campaign_link_cache: Arc::new(Mutex::new(HashMap::new())),
            tracking_pixel_cache: Arc::new(Mutex::new(HashMap::new())),
            acquisition_cache_max_entries: tuning.acquisition_cache_max_entries,
            acquisition_cache_ttl: Duration::from_secs(tuning.acquisition_cache_ttl_seconds),
            website_metadata_cache: Arc::new(Mutex::new(HashMap::new())),
            bot_policy_cache: Arc::new(Mutex::new(HashMap::new())),
            bot_override_cache: Arc::new(Mutex::new(HashMap::new())),
            collect_cache_max_entries: tuning.collect_cache_max_entries,
            collect_cache_ttl: Duration::from_secs(tuning.collect_cache_ttl_seconds),
            export_cache: Arc::new(Mutex::new(HashMap::new())),
            export_cache_compute_lock: Arc::new(Mutex::new(())),
            export_cache_max_entries: tuning.export_cache_max_entries,
            export_cache_ttl: Duration::from_secs(tuning.export_cache_ttl_seconds),
            export_cache_max_bytes: tuning.export_cache_max_bytes,
            billing_gate: Arc::new(NullBillingGate),
            export_semaphore: Arc::new(Semaphore::new(1)),
            funnel_results_semaphore: Arc::new(Semaphore::new(1)),
            journey_semaphore: Arc::new(Semaphore::new(2)),
            retention_semaphore: Arc::new(Semaphore::new(2)),
            flush_in_progress: Arc::new(AtomicBool::new(false)),
            ingest_queue: Arc::new(Mutex::new(VecDeque::new())),
            ingest_queue_events: Arc::new(AtomicUsize::new(0)),
            website_ingest_queue_events: Arc::new(Mutex::new(HashMap::new())),
            ingest_queue_max_events: tuning.ingest_queue_max_events,
            website_ingest_peak_eps_default: tuning.website_ingest_peak_eps_default,
            website_ingest_queue_max_events_default: tuning.website_ingest_queue_max_events_default,
            ingest_drain_max_events: tuning.ingest_drain_max_events,
            ingest_drain_max_batches: tuning.ingest_drain_max_batches,
            ingest_retry_base_ms: tuning.ingest_retry_base_ms,
            ingest_retry_max_ms: tuning.ingest_retry_max_ms,
            ingest_retry_max_attempts: tuning.ingest_retry_max_attempts,
            ingest_wal_log_path: ingest_wal.log_path,
            ingest_wal_cursor_path: ingest_wal.cursor_path,
            ingest_wal_append_lock: Arc::new(Mutex::new(())),
            ingest_wal_next_offset: Arc::new(AtomicU64::new(ingest_wal.next_offset)),
            ingest_wal_cursor_offset: Arc::new(AtomicU64::new(ingest_wal.cursor_offset)),
            session_cache: Arc::new(Mutex::new(HashMap::new())),
            session_cache_max_entries: tuning.session_cache_max_entries,
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
        if self.restore_ingest_queue_from_wal_internal().await {
            self.schedule_ingest_worker();
        }
    }

    async fn restore_ingest_queue_from_wal_internal(&self) -> bool {
        let _append_guard = self.ingest_wal_append_lock.lock().await;
        let mut wal_bytes = match tokio::fs::read(&self.ingest_wal_log_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!(
                    error = %e,
                    path = %self.ingest_wal_log_path.display(),
                    "Could not read ingest WAL log"
                );
                return false;
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

        let queue_front_start = {
            let queue = self.ingest_queue.lock().await;
            queue.front().map(|batch| batch.wal_start_offset)
        };
        let replay_upper_bound = queue_front_start.unwrap_or(wal_len);
        if cursor >= replay_upper_bound {
            return false;
        }

        let mut restored_batch_count = 0usize;
        let mut restored_events = 0usize;
        let mut restored_batches: Vec<IngestBatch> = Vec::new();
        let mut restored_website_counts: HashMap<String, usize> = HashMap::new();
        let mut offset = cursor as usize;
        let mut truncate_tail = false;

        while offset.saturating_add(4) <= wal_bytes.len() && (offset as u64) < replay_upper_bound {
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
            if (payload_end as u64) > replay_upper_bound {
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
            let current = self
                .ingest_queue_events
                .load(Ordering::Acquire)
                .saturating_add(restored_events);
            if current.saturating_add(event_count) > self.ingest_queue_max_events {
                if event_count > self.ingest_queue_max_events {
                    warn!(
                        event_count,
                        queue_capacity = self.ingest_queue_max_events,
                        "Skipping unreplayable WAL batch because it exceeds in-memory ingest queue cap"
                    );
                    offset = payload_end;
                    cursor = offset as u64;
                    self.persist_ingest_wal_cursor(cursor).await;
                    continue;
                }
                warn!(
                    event_count,
                    queue_capacity = self.ingest_queue_max_events,
                    "Skipping WAL replay batch because in-memory ingest queue cap would be exceeded"
                );
                break;
            }

            let website_event_counts = Self::count_events_by_website(&events);
            for (website_id, count) in &website_event_counts {
                let next = restored_website_counts
                    .get(website_id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(*count);
                restored_website_counts.insert(website_id.clone(), next);
            }

            restored_batches.push(IngestBatch {
                wal_start_offset: offset as u64,
                wal_end_offset: payload_end as u64,
                events,
                website_event_counts,
                retries: 0,
            });
            restored_batch_count += 1;
            restored_events += event_count;
            offset = payload_end;
        }

        if !truncate_tail
            && replay_upper_bound == wal_len
            && offset < wal_bytes.len()
            && wal_bytes.len().saturating_sub(offset) < 4
        {
            warn!(
                offset,
                wal_len = wal_bytes.len(),
                "Ingest WAL contains trailing bytes smaller than record header; truncating tail"
            );
            truncate_tail = true;
        }

        if !restored_website_counts.is_empty() {
            let mut website_queue = self.website_ingest_queue_events.lock().await;
            for (website_id, count) in &restored_website_counts {
                let next = website_queue
                    .get(website_id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(*count);
                website_queue.insert(website_id.clone(), next);
            }
        }

        if !restored_batches.is_empty() {
            let mut queue = self.ingest_queue.lock().await;
            if queue_front_start.is_some() {
                for batch in restored_batches.into_iter().rev() {
                    queue.push_front(batch);
                }
            } else {
                for batch in restored_batches {
                    queue.push_back(batch);
                }
            }
            self.ingest_queue_events
                .fetch_add(restored_events, Ordering::AcqRel);
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

        if restored_batch_count > 0 {
            info!(
                restored_batches = restored_batch_count,
                restored_events, "Restored ingest batches from WAL"
            );
        }
        restored_batch_count > 0
    }

    pub fn pending_session_marker() -> &'static str {
        SESSION_ID_PENDING
    }

    pub fn ingest_queue_capacity(&self) -> usize {
        self.ingest_queue_max_events
    }

    pub fn website_ingest_peak_eps_default(&self) -> usize {
        self.website_ingest_peak_eps_default
    }

    pub fn website_ingest_queue_max_events_default(&self) -> usize {
        self.website_ingest_queue_max_events_default
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

    /// Append a single WAL record while `ingest_wal_append_lock` is already held.
    async fn append_ingest_wal_record_locked(
        &self,
        events: &[Event],
    ) -> Result<IngestWalRecordOffsets, AppError> {
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
        Ok(IngestWalRecordOffsets {
            start_offset,
            end_offset,
        })
    }

    fn reserve_ingest_capacity(&self, requested_events: usize) -> usize {
        if requested_events == 0 {
            return 0;
        }

        loop {
            let current = self.ingest_queue_events.load(Ordering::Acquire);
            if current >= self.ingest_queue_max_events {
                return 0;
            }

            let remaining = self.ingest_queue_max_events.saturating_sub(current);
            let granted = requested_events.min(remaining);
            if granted == 0 {
                return 0;
            }

            if self
                .ingest_queue_events
                .compare_exchange(
                    current,
                    current.saturating_add(granted),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return granted;
            }
        }
    }

    fn release_ingest_capacity(&self, event_count: usize) {
        self.ingest_queue_events
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                Some(v.saturating_sub(event_count))
            })
            .ok();
    }

    fn count_events_by_website(events: &[Event]) -> Vec<(String, usize)> {
        let mut by_website: HashMap<String, usize> = HashMap::new();
        for event in events {
            if event.website_id.is_empty() {
                continue;
            }
            let next = by_website
                .get(&event.website_id)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            by_website.insert(event.website_id.clone(), next);
        }
        by_website.into_iter().collect()
    }

    fn count_events_by_tenant(events: &[Event]) -> Vec<(String, usize)> {
        let mut by_tenant: HashMap<String, usize> = HashMap::new();
        for event in events {
            let Some(tenant_id) = event.tenant_id.as_deref() else {
                continue;
            };
            if tenant_id.is_empty() {
                continue;
            }
            let next = by_tenant
                .get(tenant_id)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            by_tenant.insert(tenant_id.to_string(), next);
        }
        by_tenant.into_iter().collect()
    }

    fn contiguous_prefix_len(cursor: u64, batches: &[IngestBatch]) -> usize {
        let mut next_cursor = cursor;
        let mut contiguous_len = 0usize;
        for batch in batches {
            if batch.wal_start_offset != next_cursor {
                break;
            }
            next_cursor = batch.wal_end_offset;
            contiguous_len = contiguous_len.saturating_add(1);
        }
        contiguous_len
    }

    async fn release_website_queue_capacity(&self, website_event_counts: &[(String, usize)]) {
        if website_event_counts.is_empty() {
            return;
        }
        let mut queue_counts = self.website_ingest_queue_events.lock().await;
        for (website_id, count) in website_event_counts {
            let current = queue_counts.get(website_id).copied().unwrap_or(0);
            let next = current.saturating_sub(*count);
            if next == 0 {
                queue_counts.remove(website_id);
            } else {
                queue_counts.insert(website_id.clone(), next);
            }
        }
    }

    async fn reserve_website_queue_capacity(
        &self,
        events: Vec<Event>,
        website_queue_caps: Option<&HashMap<String, usize>>,
    ) -> (Vec<Event>, usize, Vec<(String, usize)>) {
        let Some(website_queue_caps) = website_queue_caps else {
            let website_counts = Self::count_events_by_website(&events);
            if !website_counts.is_empty() {
                let mut queue_counts = self.website_ingest_queue_events.lock().await;
                for (website_id, count) in &website_counts {
                    let next = queue_counts
                        .get(website_id)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(*count);
                    queue_counts.insert(website_id.clone(), next);
                }
            }
            return (events, 0, website_counts);
        };
        if website_queue_caps.is_empty() {
            let website_counts = Self::count_events_by_website(&events);
            if !website_counts.is_empty() {
                let mut queue_counts = self.website_ingest_queue_events.lock().await;
                for (website_id, count) in &website_counts {
                    let next = queue_counts
                        .get(website_id)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(*count);
                    queue_counts.insert(website_id.clone(), next);
                }
            }
            return (events, 0, website_counts);
        }

        let mut accepted = Vec::with_capacity(events.len());
        let mut dropped = 0usize;
        let mut website_event_counts: HashMap<String, usize> = HashMap::new();
        let mut queue_counts = self.website_ingest_queue_events.lock().await;

        for event in events {
            let website_id = event.website_id.clone();
            let cap = website_queue_caps
                .get(&website_id)
                .copied()
                .unwrap_or(self.website_ingest_queue_max_events_default);
            let current = queue_counts.get(&website_id).copied().unwrap_or(0);
            if current >= cap {
                dropped = dropped.saturating_add(1);
                continue;
            }

            queue_counts.insert(website_id.clone(), current.saturating_add(1));
            let next = website_event_counts
                .get(&website_id)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            website_event_counts.insert(website_id, next);
            accepted.push(event);
        }

        (
            accepted,
            dropped,
            website_event_counts.into_iter().collect(),
        )
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

        if at.signed_duration_since(entry.last_seen_at).num_seconds()
            > DEFAULT_SESSION_CACHE_TTL_SECONDS
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
                    .num_seconds()
                    <= DEFAULT_SESSION_CACHE_TTL_SECONDS
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

    async fn assign_pending_sessions(
        &self,
        events: &mut [Event],
    ) -> anyhow::Result<Vec<SessionResolvedEffect>> {
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

        let mut effects = Vec::with_capacity(session_cache.len());
        for entry in session_cache.into_values() {
            let additional_pageviews = if entry.base_count_already_recorded {
                entry.count.saturating_sub(1)
            } else {
                entry.count
            };
            effects.push(SessionResolvedEffect {
                session_id: entry.session_id,
                additional_pageviews,
                last_seen_at: entry.last_seen_at,
                website_id: entry.website_id,
                visitor_id: entry.visitor_id,
                is_bot: entry.is_bot,
                bot_score: entry.bot_score,
                bot_reason: entry.bot_reason,
            });
        }

        Ok(effects)
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
            Self::evict_cache_entries(&mut cache, self.collect_cache_max_entries, now, |entry| {
                entry.expires_at
            });
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
            Self::evict_cache_entries(&mut cache, self.collect_cache_max_entries, now, |entry| {
                entry.expires_at
            });
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
        self.invalidate_bot_policy_cache(website_id).await;
        self.invalidate_bot_override_cache(website_id).await;
    }

    pub async fn invalidate_bot_policy_cache(&self, website_id: &str) {
        self.bot_policy_cache.lock().await.remove(website_id);
    }

    pub async fn invalidate_bot_override_cache(&self, website_id: &str) {
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

        let policy = self.metadata.get_bot_policy(website_id).await?;
        let mode = policy.mode;
        let threshold_score = match mode {
            sparklytics_core::analytics::BotPolicyMode::Strict if policy.threshold_score <= 0 => {
                DEFAULT_BOT_THRESHOLD_STRICT
            }
            sparklytics_core::analytics::BotPolicyMode::Balanced
            | sparklytics_core::analytics::BotPolicyMode::Off
                if policy.threshold_score <= 0 =>
            {
                DEFAULT_BOT_THRESHOLD_BALANCED_OR_OFF
            }
            _ => policy.threshold_score,
        };
        let input = BotPolicyInput {
            mode,
            threshold_score,
        };
        {
            let mut cache = self.bot_policy_cache.lock().await;
            Self::evict_cache_entries(&mut cache, self.collect_cache_max_entries, now, |entry| {
                entry.expires_at
            });
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
            Self::hash_user_agent(user_agent),
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
            .metadata
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
            Self::evict_cache_entries(&mut cache, self.collect_cache_max_entries, now, |entry| {
                entry.expires_at
            });
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
        Self::evict_cache_entries(&mut cache, self.export_cache_max_entries, now, |entry| {
            entry.expires_at
        });
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

    /// Admit up to `requested_events` in a 60-second rolling window for `key`.
    /// Returns the accepted count (0..=requested_events).
    pub async fn admit_events_rate_limit_with_max(
        &self,
        key: &str,
        requested_events: usize,
        max_per_min: usize,
    ) -> usize {
        if requested_events == 0 || max_per_min == 0 {
            return 0;
        }

        let mut map = self.event_rate_limiter.lock().await;
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(60);

        if let Some(window) = map.get_mut(key) {
            while window.front().is_some_and(|(at, _)| *at < cutoff) {
                window.pop_front();
            }
            if window.is_empty() {
                map.remove(key);
            }
        }

        if !map.contains_key(key) && map.len() >= self.rate_limiter_max_entries {
            map.retain(|_, window| {
                while window.front().is_some_and(|(at, _)| *at < cutoff) {
                    window.pop_front();
                }
                !window.is_empty()
            });
            if !map.contains_key(key) && map.len() >= self.rate_limiter_max_entries {
                return 0;
            }
        }

        let window = map.entry(key.to_string()).or_default();
        let current_count: usize = window.iter().map(|(_, count)| *count).sum();
        if current_count >= max_per_min {
            return 0;
        }

        let remaining = max_per_min.saturating_sub(current_count);
        let admitted = requested_events.min(remaining);
        if admitted > 0 {
            window.push_back((now, admitted));
        }
        admitted
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
            Self::evict_cache_entries(
                &mut cache,
                self.acquisition_cache_max_entries,
                now,
                |entry| entry.expires_at,
            );
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
            Self::evict_cache_entries(
                &mut cache,
                self.acquisition_cache_max_entries,
                now,
                |entry| entry.expires_at,
            );
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
        let _ = self.enqueue_ingest_events_with_limits(events, None).await?;
        Ok(())
    }

    /// Persist events to WAL and enqueue them for async ingestion with optional
    /// per-website queue caps.
    pub async fn enqueue_ingest_events_with_limits(
        self: &Arc<Self>,
        events: Vec<Event>,
        website_queue_caps: Option<&HashMap<String, usize>>,
    ) -> Result<IngestEnqueueOutcome, AppError> {
        if events.is_empty() {
            return Ok(IngestEnqueueOutcome {
                accepted_events: 0,
                dropped_events: 0,
            });
        }

        let (events, dropped_by_website_cap, mut website_event_counts) = self
            .reserve_website_queue_capacity(events, website_queue_caps)
            .await;
        if events.is_empty() {
            return Ok(IngestEnqueueOutcome {
                accepted_events: 0,
                dropped_events: dropped_by_website_cap,
            });
        }

        let append_guard = self.ingest_wal_append_lock.lock().await;
        let globally_accepted = self.reserve_ingest_capacity(events.len());
        if globally_accepted == 0 {
            drop(append_guard);
            self.release_website_queue_capacity(&website_event_counts)
                .await;
            return Ok(IngestEnqueueOutcome {
                accepted_events: 0,
                dropped_events: dropped_by_website_cap.saturating_add(events.len()),
            });
        }

        let mut accepted_events = events;
        let mut dropped_by_global_cap = 0usize;
        if globally_accepted < accepted_events.len() {
            dropped_by_global_cap = accepted_events.len().saturating_sub(globally_accepted);
            let dropped_tail = accepted_events.split_off(globally_accepted);
            let dropped_tail_counts = Self::count_events_by_website(&dropped_tail);
            self.release_website_queue_capacity(&dropped_tail_counts)
                .await;
            website_event_counts = Self::count_events_by_website(&accepted_events);
        }

        let accepted_count = accepted_events.len();
        let wal_offsets = match self.append_ingest_wal_record_locked(&accepted_events).await {
            Ok(offsets) => offsets,
            Err(e) => {
                self.release_ingest_capacity(accepted_count);
                self.release_website_queue_capacity(&website_event_counts)
                    .await;
                return Err(e);
            }
        };

        {
            let mut queue = self.ingest_queue.lock().await;
            queue.push_back(IngestBatch {
                wal_start_offset: wal_offsets.start_offset,
                wal_end_offset: wal_offsets.end_offset,
                events: accepted_events,
                website_event_counts,
                retries: 0,
            });
        }
        drop(append_guard);
        self.schedule_ingest_worker();

        Ok(IngestEnqueueOutcome {
            accepted_events: accepted_count,
            dropped_events: dropped_by_website_cap.saturating_add(dropped_by_global_cap),
        })
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
                if self.ingest_wal_cursor_offset.load(Ordering::Acquire)
                    < self.ingest_wal_next_offset.load(Ordering::Acquire)
                {
                    self.restore_ingest_queue_from_wal_internal().await;
                    let cursor = self.ingest_wal_cursor_offset.load(Ordering::Acquire);
                    let next = self.ingest_wal_next_offset.load(Ordering::Acquire);
                    if cursor < next {
                        if self.ingest_queue_events.load(Ordering::Acquire)
                            >= self.ingest_queue_max_events
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                        continue;
                    }
                }
                break;
            }

            let cursor = self.ingest_wal_cursor_offset.load(Ordering::Acquire);
            let contiguous_len = Self::contiguous_prefix_len(cursor, &batches);
            if contiguous_len == 0 {
                {
                    let mut queue = self.ingest_queue.lock().await;
                    for batch in batches.drain(..).rev() {
                        queue.push_front(batch);
                    }
                }
                if cursor < self.ingest_wal_next_offset.load(Ordering::Acquire) {
                    self.restore_ingest_queue_from_wal_internal().await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
            if contiguous_len < batches.len() {
                let mut tail = batches.split_off(contiguous_len);
                let mut queue = self.ingest_queue.lock().await;
                for batch in tail.drain(..).rev() {
                    queue.push_front(batch);
                }
            }

            let total_events: usize = batches.iter().map(|b| b.events.len()).sum();
            let mut total_website_counts: HashMap<String, usize> = HashMap::new();
            for batch in &batches {
                for (website_id, count) in &batch.website_event_counts {
                    let next = total_website_counts
                        .get(website_id)
                        .copied()
                        .unwrap_or(0)
                        .saturating_add(*count);
                    total_website_counts.insert(website_id.clone(), next);
                }
            }
            let website_counts: Vec<(String, usize)> = total_website_counts
                .iter()
                .map(|(website_id, count)| (website_id.clone(), *count))
                .collect();
            let max_retries = batches.iter().map(|b| b.retries).max().unwrap_or(0);
            let mut merged_events = Vec::with_capacity(total_events);
            for batch in &batches {
                merged_events.extend(batch.events.iter().cloned());
            }

            let (session_effects, persist_result) =
                match self.assign_pending_sessions(&mut merged_events).await {
                    Ok(effects) => {
                        let insert_result = self.analytics.insert_events(&merged_events).await;
                        (effects, insert_result)
                    }
                    Err(e) => (Vec::new(), Err(e)),
                };

            match persist_result {
                Ok(()) => {
                    let wal_end_offset = batches
                        .last()
                        .map(|b| b.wal_end_offset)
                        .unwrap_or_else(|| self.ingest_wal_cursor_offset.load(Ordering::Acquire));
                    let tenant_counts = Self::count_events_by_tenant(&merged_events);

                    for effect in session_effects {
                        if effect.additional_pageviews > 0 {
                            if let Err(err) = self
                                .analytics
                                .increment_session_pageviews(
                                    &effect.session_id,
                                    effect.additional_pageviews,
                                    effect.last_seen_at,
                                )
                                .await
                            {
                                warn!(
                                    session_id = %effect.session_id,
                                    error = %err,
                                    "Failed to update session pageviews after event insert"
                                );
                            }
                        }
                        if effect.is_bot || effect.bot_score > 0 || effect.bot_reason.is_some() {
                            if let Err(err) = self
                                .analytics
                                .set_session_bot_classification(
                                    &effect.session_id,
                                    effect.is_bot,
                                    effect.bot_score,
                                    effect.bot_reason.as_deref(),
                                )
                                .await
                            {
                                warn!(
                                    session_id = %effect.session_id,
                                    error = %err,
                                    "Failed to update session bot classification after event insert"
                                );
                            }
                        }
                        self.put_cached_session(
                            effect.website_id,
                            effect.visitor_id,
                            effect.session_id,
                            effect.last_seen_at,
                        )
                        .await;
                    }

                    self.release_ingest_capacity(total_events);
                    self.release_website_queue_capacity(&website_counts).await;
                    self.persist_ingest_wal_cursor(wal_end_offset).await;
                    self.maybe_compact_ingest_wal().await;
                    for (tenant_id, count) in tenant_counts {
                        // TODO(accounting): Failed usage sync can create permanent drift.
                        // Add a durable retry queue for record_persisted_events failures.
                        if let Err(err) = self
                            .billing_gate
                            .record_persisted_events(&tenant_id, count)
                            .await
                        {
                            warn!(
                                tenant_id,
                                count,
                                error = %err,
                                "Failed to record persisted usage for tenant"
                            );
                        }
                    }
                }
                Err(e) => {
                    let next_retry_attempt = max_retries.saturating_add(1);
                    let retry_delay_ms = (self
                        .ingest_retry_base_ms
                        .saturating_mul(2u64.saturating_pow(next_retry_attempt as u32)))
                    .min(self.ingest_retry_max_ms);
                    if next_retry_attempt >= self.ingest_retry_max_attempts {
                        error!(
                            error = %e,
                            retries = next_retry_attempt,
                            max_retries = self.ingest_retry_max_attempts,
                            batch_count = batches.len(),
                            event_count = total_events,
                            "Ingest queue persist hit max retries, keeping batch at queue head with max backoff"
                        );
                        {
                            let mut queue = self.ingest_queue.lock().await;
                            for mut batch in batches.drain(..).rev() {
                                batch.retries = next_retry_attempt;
                                queue.push_front(batch);
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(
                            self.ingest_retry_max_ms,
                        ))
                        .await;
                        continue;
                    }
                    error!(
                        error = %e,
                        retries = next_retry_attempt,
                        delay_ms = retry_delay_ms,
                        batch_count = batches.len(),
                        event_count = total_events,
                        "Ingest queue persist failed, will retry"
                    );
                    {
                        let mut queue = self.ingest_queue.lock().await;
                        for mut batch in batches.drain(..).rev() {
                            batch.retries = next_retry_attempt;
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
