# Backend Requirements Specification

**Component:** sparklytics-server + sparklytics-core
**Language:** Rust (2021 edition)
**Framework:** Axum 0.8+
**Runtime:** Tokio (multi-threaded)

## Cargo Workspace Structure

```
sparklytics/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── sparklytics-server/     # Binary crate (main entry point)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs         # Entry point, config loading
│   │       ├── config.rs       # Environment variable parsing
│   │       ├── routes/         # Axum route handlers
│   │       │   ├── mod.rs
│   │       │   ├── collect.rs  # POST /api/collect
│   │       │   ├── stats.rs    # GET /api/websites/:id/stats
│   │       │   ├── pageviews.rs
│   │       │   ├── metrics.rs
│   │       │   ├── realtime.rs
│   │       │   ├── websites.rs # CRUD
│   │       │   ├── auth.rs     # Login, register, etc.
│   │       │   └── keys.rs     # API key management
│   │       ├── middleware/
│   │       │   ├── mod.rs
│   │       │   ├── auth.rs     # JWT validation
│   │       │   ├── rate_limit.rs
│   │       │   └── cors.rs
│   │       ├── buffer.rs       # Event buffer (flush logic)
│   │       └── dashboard.rs    # Serve embedded SPA
│   │
│   ├── sparklytics-core/       # Shared types and logic
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs        # Event, Session, Website, User structs
│   │       ├── query.rs        # AnalyticsQuery, StatsQuery, etc.
│   │       ├── backend.rs      # AnalyticsBackend trait
│   │       ├── enrichment.rs   # GeoIP, UA parsing
│   │       ├── session.rs      # Session management logic
│   │       ├── hash.rs         # Visitor ID hashing
│   │       └── validation.rs   # Input validation
│   │
│   ├── sparklytics-duckdb/     # DuckDB backend implementation
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── backend.rs      # AnalyticsBackend impl
│   │       ├── schema.rs       # Table creation
│   │       ├── queries.rs      # SQL query generation
│   │       └── migrations.rs   # Schema migrations
│   │
│   └── sparklytics-clickhouse/ # ClickHouse backend (Week 7+)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── backend.rs
│           ├── schema.rs
│           ├── queries.rs
│           └── views.rs        # Materialized view management
│
├── dashboard/                  # React SPA (separate build)
├── packages/
│   └── next/                   # @sparklytics/next SDK
├── scripts/                    # Build scripts
└── docker/
    └── Dockerfile
```

## Core Dependencies

```toml
[workspace.dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "v7"] }  # v7 used for website_id (time-ordered, 128-bit, harder to enumerate than nanoid)
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
sha2 = "0.10"
maxminddb = "0.24"
woothee = "0.13"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
duckdb = { version = "1.0", features = ["bundled"] }
clickhouse = "0.12"    # Official ClickHouse Rust client
jsonwebtoken = "9"
bcrypt = "0.15"
nanoid = "0.4"
include_dir = "0.7"
```

## Performance Requirements

| Metric | Requirement | How to Verify |
|--------|-------------|---------------|
| Event ingestion p99 | <50ms | criterion benchmarks |
| Query response p99 | <200ms | k6 load tests |
| Memory (idle) | <150MB | `docker stats` after 5 min (DuckDB min is 125MB/thread) |
| Memory (10K events/day) | <200MB | Load test with k6 |
| Memory (100K events/day) | <350MB | Load test with k6 |
| Startup time | <3s | Measured in CI |
| Binary size | <50MB | Checked in CI |
| Docker image | <100MB | Multi-stage build |

## Configuration

All configuration via environment variables (12-factor app):

```rust
pub struct Config {
    // Server
    pub port: u16,                    // SPARKLYTICS_PORT (default: 3000)
    pub host: String,                 // SPARKLYTICS_HOST (default: "0.0.0.0")

    // Backend
    pub backend: BackendType,         // SPARKLYTICS_BACKEND (default: "duckdb")

    // DuckDB
    pub duckdb_path: PathBuf,         // SPARKLYTICS_DATA_DIR/sparklytics.db

    // ClickHouse
    pub clickhouse_url: String,       // SPARKLYTICS_CLICKHOUSE_URL
    pub clickhouse_database: String,  // SPARKLYTICS_CLICKHOUSE_DB (default: "sparklytics")

    // Auth
    pub password: Option<String>,     // SPARKLYTICS_PASSWORD (single-user mode)
    pub jwt_secret: String,           // SPARKLYTICS_JWT_SECRET (auto-generated if not set)

    // Data
    pub retention_days: u32,          // SPARKLYTICS_RETENTION_DAYS (default: 365)
    pub geoip_path: PathBuf,          // SPARKLYTICS_GEOIP_PATH

    // Limits
    pub cors_origins: Vec<String>,    // SPARKLYTICS_CORS_ORIGINS (default: "same-origin"; never "*")
    pub rate_limit: u32,              // SPARKLYTICS_RATE_LIMIT (default: 60 req/min per IP on /api/collect)
    pub buffer_size: usize,           // SPARKLYTICS_BUFFER_SIZE (default: 10_000)
    pub buffer_flush_ms: u64,         // SPARKLYTICS_BUFFER_FLUSH_MS (default: 5000)
    // Buffer overflow policy: when buffer is at capacity, return HTTP 503 Retry-After: 5 to the
    // caller (SDK retries automatically). Also drain to overflow WAL file (events_overflow.db,
    // SQLite) for replay on next flush tick. NEVER silently drop events.

    // Logging
    pub log_level: String,            // SPARKLYTICS_LOG_LEVEL (default: "info")
    pub log_format: LogFormat,        // SPARKLYTICS_LOG_FORMAT (default: "json")
}
```

## Error Handling

Use `thiserror` for error types, `anyhow` for internal errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SparkError {
    #[error("Validation error: {message}")]
    Validation { message: String, field: Option<String> },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Rate limited")]
    RateLimited,

    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

All errors map to the standard JSON error format defined in API spec.

## Testing Strategy

**Unit tests:** Per-module, focus on query building, validation, hashing, session logic.
**Integration tests:** Full HTTP request/response cycle with in-memory DuckDB.
**Load tests:** k6 scripts for event ingestion and dashboard queries.
**Benchmark tests:** criterion for hot paths (enrichment, buffering, query building).

Test both backends with the same test suite (parameterized tests).

## Build & Release

```dockerfile
# Multi-stage Dockerfile
FROM rust:1.82-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sparklytics-server /usr/local/bin/sparklytics
COPY --from=builder /app/data/GeoLite2-City.mmdb /data/GeoLite2-City.mmdb
EXPOSE 3000
CMD ["sparklytics"]
```

Target image size: <100MB (Rust binary ~15MB + GeoIP ~60MB + base image ~25MB).
