/// DuckDB initialization SQL.
///
/// Executed once at database open time via `Connection::execute_batch`.
/// All statements use `IF NOT EXISTS` so they are safe to re-run on every
/// startup (idempotent).
///
/// `memory_limit` is passed at runtime from `Config.duckdb_memory_limit`
/// (env `SPARKLYTICS_DUCKDB_MEMORY`, default `"1GB"`). DuckDB accepts any
/// size string it supports — e.g. `"512MB"`, `"1GB"`, `"4GB"`.
/// Modern VPS instances with 4–32 GB RAM can comfortably set 2–8 GB;
/// minimal 1 GB instances should keep it at `"512MB"`.
///
/// IMPORTANT (CLAUDE.md critical fact #12):
///   - Always set an explicit memory limit — the DuckDB default (80% of
///     system RAM) is not acceptable for a server process.
///   - `SET threads = 2` — limits background thread pool; safe for single-
///     writer embedded use.
///
/// IMPORTANT (CLAUDE.md critical fact #4):
///   - Bounce-rate queries MUST use CTEs. Correlated subqueries do not work
///     in DuckDB. See bounce_rate CTE patterns in the query layer.
///
/// NOTE: DuckDB 1.4+ enforces FOREIGN KEY constraints immediately at statement
/// execution time (not deferred to commit). However, within a single
/// transaction, writes made earlier in the same transaction ARE visible to FK
/// checks on later statements. So: run all cascade deletes inside one
/// transaction (child rows first, parent last). The EXISTS check must also be
/// inside the same transaction. See delete_website() in website.rs.
/// ALTER TABLE ... DROP CONSTRAINT is NOT supported in DuckDB 1.4.4, so the
/// events.website_id FK was removed from the schema for NEW databases (see
/// events table below). Existing databases retain the FK but delete_website()
/// handles it correctly via the full-transaction approach.
pub fn init_sql(memory_limit: &str) -> String {
    format!(
        r#"SET memory_limit = '{memory_limit}';
SET threads = 2;

-- ===========================================
-- SETTINGS (self-hosted only)
-- ===========================================
-- Keys stored in this table:
--   'daily_salt'     – 32-byte random hex for visitor_id hashing (rotated daily at midnight UTC)
--   'previous_salt'  – Previous day's salt, kept for 5-minute grace period after midnight UTC rotation
--   'version'        – Database schema version (for migrations)
--   'install_id'     – Unique installation identifier
CREATE TABLE IF NOT EXISTS settings (
    key             VARCHAR PRIMARY KEY,
    value           VARCHAR NOT NULL
);

-- ===========================================
-- WEBSITES
-- ===========================================
CREATE TABLE IF NOT EXISTS websites (
    id              VARCHAR PRIMARY KEY,           -- 'site_' + nanoid(10)
    tenant_id       VARCHAR,                       -- NULL in self-hosted mode; Clerk org_id in cloud
    name            VARCHAR NOT NULL,
    domain          VARCHAR NOT NULL,
    timezone        VARCHAR(64) NOT NULL DEFAULT 'UTC',  -- IANA timezone string
    share_id        VARCHAR(50) UNIQUE,            -- V1.1: public read-only link (NULL until enabled)
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP  -- Track last modification
);
CREATE INDEX IF NOT EXISTS idx_websites_tenant   ON websites(tenant_id);
CREATE INDEX IF NOT EXISTS idx_websites_share_id ON websites(share_id);

-- ===========================================
-- SESSIONS (derived, updated on each event)
-- ===========================================
-- Session deduplication strategy:
-- To handle concurrent event processing, session updates use DuckDB's
-- INSERT OR REPLACE semantics. When multiple events for the same session
-- arrive in the same buffer flush batch, the UNIQUE constraint on session_id
-- ensures only the latest state is preserved. The event buffer (in Rust) orders
-- events by timestamp before flushing, so pageview_count increments correctly.
CREATE TABLE IF NOT EXISTS sessions (
    session_id      VARCHAR PRIMARY KEY,
    website_id      VARCHAR NOT NULL,
    tenant_id       VARCHAR,                       -- NULL in self-hosted; Clerk org_id in cloud
    visitor_id      VARCHAR NOT NULL,
    first_seen      TIMESTAMP NOT NULL,
    last_seen       TIMESTAMP NOT NULL,
    pageview_count  INTEGER NOT NULL DEFAULT 1,    -- Incremented on upsert
    entry_page      VARCHAR NOT NULL
);
-- Optimised for "active visitors in last N minutes" query (realtime endpoint)
CREATE INDEX IF NOT EXISTS idx_sessions_website_visitor
    ON sessions(website_id, visitor_id, last_seen DESC);
-- Optimised for realtime active-visitors query
CREATE INDEX IF NOT EXISTS idx_sessions_realtime
    ON sessions(website_id, last_seen DESC);
-- Optimised for sessions explorer cursor pagination
CREATE INDEX IF NOT EXISTS idx_sessions_website_last_seen
    ON sessions(website_id, last_seen DESC, session_id DESC);
-- Optimized for retention cohorts (visitor first-seen lookup)
CREATE INDEX IF NOT EXISTS idx_sessions_website_visitor_first
    ON sessions(website_id, visitor_id, first_seen ASC);

-- ===========================================
-- EVENTS (main analytics table)
-- ===========================================
CREATE TABLE IF NOT EXISTS events (
    -- Identity
    id              VARCHAR NOT NULL,              -- UUID v4
    website_id      VARCHAR NOT NULL,
    tenant_id       VARCHAR,                       -- NULL in self-hosted; Clerk org_id in cloud
    session_id      VARCHAR NOT NULL,
    visitor_id      VARCHAR NOT NULL,              -- sha256(salt_epoch + ip + ua)[0:16]

    -- Event data
    event_type      VARCHAR NOT NULL,              -- 'pageview' | 'event'
    url             VARCHAR NOT NULL,
    referrer_url    VARCHAR,
    referrer_domain VARCHAR,
    event_name      VARCHAR,                       -- custom event name (nullable)
    event_data      VARCHAR,                       -- JSON string for custom properties (nullable)

    -- GeoIP
    country         VARCHAR(2),                    -- ISO 3166-1 alpha-2
    region          VARCHAR,
    city            VARCHAR,

    -- User Agent
    browser         VARCHAR,
    browser_version VARCHAR,
    os              VARCHAR,
    os_version      VARCHAR,
    device_type     VARCHAR,                       -- 'desktop' | 'mobile' | 'tablet'

    -- Client
    screen          VARCHAR,                       -- e.g. '1920x1080'
    language        VARCHAR,

    -- UTM parameters
    utm_source      VARCHAR,
    utm_medium      VARCHAR,
    utm_campaign    VARCHAR,
    utm_term        VARCHAR,
    utm_content     VARCHAR,

    -- Timestamp
    created_at      TIMESTAMP NOT NULL
    -- NOTE: No FOREIGN KEY on website_id. DuckDB 1.4+ enforces FK constraints
    -- immediately (not deferred), which conflicts with the manual cascade-delete
    -- order needed for events → sessions → goals → websites. See migration
    -- m001_drop_events_fk() in backend.rs which drops this constraint on
    -- existing databases that were created with the old FK declaration.
);

-- Primary query pattern: website + date range
CREATE INDEX IF NOT EXISTS idx_events_website_time
    ON events(website_id, created_at DESC);

-- Accelerates session-level aggregations
CREATE INDEX IF NOT EXISTS idx_events_website_session
    ON events(website_id, session_id);
-- Accelerates session timeline queries ordered by created_at
CREATE INDEX IF NOT EXISTS idx_events_website_session_time
    ON events(website_id, session_id, created_at);

-- Accelerates per-visitor history lookups
CREATE INDEX IF NOT EXISTS idx_events_visitor
    ON events(website_id, visitor_id, created_at);

-- Accelerates event-type breakdowns (pageviews vs custom events) within a date range
CREATE INDEX IF NOT EXISTS idx_events_type_date
    ON events(website_id, event_type, created_at);

-- Accelerates funnel/page filters targeting pageview URLs
CREATE INDEX IF NOT EXISTS idx_events_page_url_date
    ON events(website_id, event_type, url, created_at DESC);

-- Accelerates custom-events queries (event names, properties, event timeseries)
CREATE INDEX IF NOT EXISTS idx_events_name_date
    ON events(website_id, event_type, event_name, created_at DESC);

-- Accelerates country breakdown queries
CREATE INDEX IF NOT EXISTS idx_events_country_date
    ON events(website_id, country, created_at);

-- Schema-parity index with ClickHouse cloud schema (tenant_id always NULL in self-hosted)
CREATE INDEX IF NOT EXISTS idx_events_tenant
    ON events(tenant_id, created_at DESC);

-- ===========================================
-- GOALS (self-hosted conversions)
-- ===========================================
CREATE TABLE IF NOT EXISTS goals (
    id              VARCHAR PRIMARY KEY,
    website_id      VARCHAR NOT NULL,
    name            VARCHAR NOT NULL,
    goal_type       VARCHAR NOT NULL,              -- 'page_view' | 'event'
    match_value     VARCHAR NOT NULL,
    match_operator  VARCHAR NOT NULL DEFAULT 'equals',
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_goals_website_id
    ON goals(website_id);

-- ===========================================
-- SAVED REPORTS (Insights Builder)
-- ===========================================
CREATE TABLE IF NOT EXISTS saved_reports (
    id              VARCHAR PRIMARY KEY,
    website_id      VARCHAR NOT NULL,
    name            VARCHAR NOT NULL,
    description     VARCHAR,
    config_json     VARCHAR NOT NULL,
    last_run_at     TIMESTAMP,
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_saved_reports_website
    ON saved_reports(website_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_saved_reports_name_website
    ON saved_reports(website_id, name);

-- ===========================================
-- FUNNELS (self-hosted conversion paths)
-- ===========================================
CREATE TABLE IF NOT EXISTS funnels (
    id              VARCHAR PRIMARY KEY,
    website_id      VARCHAR NOT NULL,
    name            VARCHAR NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_funnels_website
    ON funnels(website_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_funnels_website_name
    ON funnels(website_id, name);

CREATE TABLE IF NOT EXISTS funnel_steps (
    id              VARCHAR PRIMARY KEY,
    funnel_id       VARCHAR NOT NULL,
    step_order      INTEGER NOT NULL,
    step_type       VARCHAR NOT NULL,              -- 'page_view' | 'event'
    match_value     VARCHAR NOT NULL,
    match_operator  VARCHAR NOT NULL DEFAULT 'equals',
    label           VARCHAR NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_funnel_steps_funnel_order
    ON funnel_steps(funnel_id, step_order);

-- ===========================================
-- LOCAL API KEYS (self-hosted only)
-- Cloud equivalent lives in PostgreSQL api_keys table.
-- Self-hosted key prefix: 'spk_selfhosted_' (15 chars) + display chars = 25 chars total.
-- ===========================================
CREATE TABLE IF NOT EXISTS local_api_keys (
    id              VARCHAR PRIMARY KEY,           -- 'key_' + nanoid(10)
    name            VARCHAR NOT NULL,
    key_hash        VARCHAR(64) NOT NULL UNIQUE,   -- sha256(raw_key); never stored raw
    key_prefix      VARCHAR(25) NOT NULL,          -- first 25 chars: 'spk_selfhosted_' + display
    created_at      TIMESTAMP NOT NULL,
    last_used_at    TIMESTAMP,                     -- NULL until first use
    revoked_at      TIMESTAMP                      -- NULL = active; set to revoke
);
CREATE INDEX IF NOT EXISTS idx_local_api_keys_hash ON local_api_keys(key_hash);

-- ===========================================
-- LOGIN ATTEMPTS (self-hosted only)
-- Used for brute-force protection on POST /api/auth/login.
-- Cleanup: DELETE WHERE attempted_at < NOW() - INTERVAL 24 HOURS (daily maintenance task).
-- Rate limiter: SELECT COUNT(*) WHERE ip_address = ? AND attempted_at > ? AND succeeded = false
-- ===========================================
CREATE TABLE IF NOT EXISTS login_attempts (
    id           VARCHAR PRIMARY KEY,              -- nanoid(10)
    ip_address   VARCHAR NOT NULL,
    attempted_at TIMESTAMP NOT NULL,
    succeeded    BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_login_attempts_ip_time
    ON login_attempts(ip_address, attempted_at DESC);
"#
    )
}

/// Migrations tracking table SQL.
///
/// Run before INIT_SQL migrations are applied. Tracks which numbered migrations
/// have been applied so restarts don't re-run them.
pub const MIGRATIONS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS _migrations (
    id          VARCHAR PRIMARY KEY,
    applied_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;
