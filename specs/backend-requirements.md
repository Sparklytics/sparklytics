# Backend Requirements Specification

**Component:** `sparklytics-server` + shared crates  
**Language:** Rust 2021  
**Framework:** Axum 0.8 + Tokio  
**Status:** Aligned with current public repo state (2026-02-27)

## Workspace Structure (Current)

```text
sparklytics/
├── Cargo.toml
├── crates/
│   ├── sparklytics-core/      # shared models, config, analytics traits, billing trait
│   ├── sparklytics-duckdb/    # DuckDB analytics + metadata implementation
│   ├── sparklytics-metadata/  # metadata contracts (traits/types)
│   └── sparklytics-server/    # Axum app, routes, auth, state, scheduler
├── dashboard/
└── sdk/next/                  # nested SDK repo
```

## Runtime Model

- Public runtime binary: `sparklytics` (`crates/sparklytics-server/src/main.rs`)
- Server wiring lives in `app.rs`, shared state in `state.rs`, route handlers in `routes/*`
- Self-hosted mode uses DuckDB for analytics and metadata
- Cloud runtime (ClickHouse/PostgreSQL/Billing wiring) lives in the private `sparklytics-cloud` repo and consumes public traits

## Core Dependencies (Public Repo)

Key workspace dependencies from root `Cargo.toml`:

- `axum`, `tokio`, `tower`, `tower-http`
- `duckdb`
- `serde`, `serde_json`, `chrono`, `uuid`
- `tracing`, `tracing-subscriber`
- `argon2`, `jsonwebtoken`, `sha2`, `hex`, `rand`
- `maxminddb`, `woothee`, `url`, `psl`, `ipnet`

Public workspace intentionally does not include cloud-only dependencies like Clerk/ClickHouse/sqlx.

## Configuration Contract

Configuration is defined in `sparklytics_core::config::Config` and loaded from env.

| Variable | Default | Notes |
|---|---|---|
| `SPARKLYTICS_PORT` | `3000` | HTTP listen port |
| `SPARKLYTICS_DATA_DIR` | `./data` | data directory (DuckDB + local assets) |
| `SPARKLYTICS_GEOIP_PATH` | `./GeoLite2-City.mmdb` | MMDB path (geo enrichment optional) |
| `SPARKLYTICS_AUTH` | `local` | `none` \| `password` \| `local` |
| `SPARKLYTICS_PASSWORD` | none | required when `SPARKLYTICS_AUTH=password` |
| `SPARKLYTICS_HTTPS` | `true` | controls secure cookie behavior |
| `SPARKLYTICS_RETENTION_DAYS` | `365` | retention horizon |
| `SPARKLYTICS_CORS_ORIGINS` | empty | allowlist for query endpoints |
| `SPARKLYTICS_SESSION_DAYS` | `7` | session cookie lifetime days |
| `SPARKLYTICS_MODE` | `selfhosted` | `selfhosted` \| `cloud` |
| `SPARKLYTICS_ARGON2_MEMORY_KB` | `65536` | Argon2id memory cost |
| `SPARKLYTICS_PUBLIC_URL` | `http://localhost:3000` | used in snippets/links |
| `SPARKLYTICS_RATE_LIMIT_DISABLE` | `false` | benchmark-only bypass for collect limiter |
| `SPARKLYTICS_DUCKDB_MEMORY` | `1GB` | explicit DuckDB memory limit |

Fixed ingest settings in code:

- `buffer_flush_interval_ms = 5000`
- `buffer_max_size = 100`

## Auth and API Contracts

- `GET /api/auth/status` is flat JSON (`mode`, `setup_required`, `authenticated`) and is not registered in `none` mode (404)
- `POST /api/collect` is unauthenticated, rate-limited per IP (60/min), max batch size 50
- API key generation is mode-aware:
  - self-hosted: `spk_selfhosted_...`
  - cloud runtime: `spk_live_...`
- API key DB prefix storage currently uses first 20 characters of the raw key

## Data Integrity Requirements

- Self-hosted `tenant_id` stays `NULL` (single-tenant mode)
- Website IDs use `site_` + 10 random alphanumeric chars
- DuckDB FK constraints are enforced; delete flows must remove child rows first in a single transaction
- Bounce-rate SQL should use CTEs (DuckDB-safe)

## Build and Verification

Required checks after backend changes:

```bash
cargo check
cargo test
```

Release binary name is `sparklytics`:

```bash
cargo build --release -p sparklytics-server
# target/release/sparklytics
```
