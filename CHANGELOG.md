# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0] — 2026-02-27

### Added

- New `sparklytics-metadata` crate with shared metadata contracts (`MetadataStore`, website/api-key models).
- Metadata abstraction wiring in server state and routes, including DuckDB metadata adapter.
- New metadata-injection integration coverage (`state_metadata_integration`) validating backend separation.
- Bot policy/override cache improvements in collect path:
  - hashed user-agent in override cache key (lower memory footprint),
  - bounded eviction strategy (expire-first, oldest-next) across caches,
  - explicit fallback threshold constants for strict/balanced-off modes.
- API key generation fully mode-aware (`spk_selfhosted_` and `spk_live_`) with updated docs and tests.

### Changed

- Collect/auth/share/website flows now consistently route metadata operations through `MetadataStore`.
- Bot policy + override cache invalidation now happens on relevant policy/list mutations.
- Clippy-clean refactors in bot recompute/bot detection/state modules to keep CI strict-warning clean.

### Fixed

- Fixed stale bot-policy/override cache behavior after policy/list updates.
- Fixed CI merge-blocking clippy failures (`type_complexity`, doc continuation) on release branch.

## [0.1.0] — 2026-02-19

### Added

**Core**
- Rust workspace: `sparklytics-core`, `sparklytics-duckdb`, `sparklytics-server`
- `POST /api/collect` — single event and batch ingestion (up to 50 events, 100KB max)
- In-memory event buffer with 5-second flush interval and 100-event immediate flush
- DuckDB embedded analytics storage with automatic schema initialization
- Visitor ID: `sha256(salt_epoch + ip + user_agent)[0:16]` — privacy-preserving, no cookies, daily rotation at midnight UTC
- Session tracking: 30-minute inactivity timeout, server-side, no client state required
- GeoIP enrichment via MaxMind GeoLite2-City (optional, graceful degradation if absent)
- User-Agent parsing (browser, OS, device type) via `woothee`
- UTM parameter extraction from URL query string

**Analytics API**
- `GET /api/websites/:id/stats` — pageviews, visitors, sessions, bounce rate, avg duration + prior-period comparison
- `GET /api/websites/:id/pageviews` — time series (hourly/daily/monthly/auto granularity)
- `GET /api/websites/:id/metrics` — dimension breakdowns (page, referrer, country, browser, OS, device, language, screen, UTM)
- `GET /api/websites/:id/realtime` — active visitors + last 100 events in 30-minute window
- `GET /health` — liveness probe (200 OK / 503 degraded)

**Website Management**
- `GET /api/websites` — paginated list
- `POST /api/websites` — create with tracking snippet
- `PUT /api/websites/:id` — update name/domain/timezone
- `DELETE /api/websites/:id` — delete with full cascade (events → sessions → website)

**Auth (Self-Hosted)**
- Three modes: `none` (open), `password` (env var), `local` (full Argon2id + JWT)
- JWT HttpOnly + SameSite=Strict cookies; configurable 1–30 day expiry
- API key management (`spk_selfhosted_` prefix, SHA-256 hashed, never stored raw)
- `GET /api/auth/status` — mode detection for frontend routing (never returns 401)
- `POST /api/auth/setup`, `POST /api/auth/login`, `POST /api/auth/logout`

**Dashboard**
- Next.js 16 App Router, static export embedded in Rust binary
- Dark mode by default, TailwindCSS + shadcn/ui
- URL-param-driven filters (date range, page, country, browser, OS, device, UTM)
- Realtime panel with 10-second polling
- Login / setup UI for auth modes

**SDK (`@sparklytics/next`)**
- `<SparklyticsProvider>` — zero-config App Router + Pages Router support
- Auto pageview tracking (pushState monkey-patch + popstate + Pages Router routeChangeComplete)
- `useSparklytics()` hook with `track(name, data?)` for custom events
- `<SparklyticsEvent>` — declarative click tracking component
- Batch queue: 500ms debounce, max 10 events, `sendBeacon` on unload
- DNT + GPC privacy signals respected by default
- TypeScript strict mode, < 5KB gzipped

**Security**
- Rate limiting: 60 req/min per IP on `/api/collect` (sliding window)
- Payload limits: 100KB total body, 4KB per `event_data`
- CORS: `/api/collect` allows `*`; analytics endpoints enforce `SPARKLYTICS_CORS_ORIGINS`
- SQL injection safe: all user input stored via parameterized DuckDB queries
- DuckDB memory limit: configurable via `SPARKLYTICS_DUCKDB_MEMORY` (default `1GB`; set 2–8 GB on high-RAM VPS for better query performance)

**Infrastructure**
- Multi-stage Dockerfile: musl static binary → `distroless/static:nonroot` (uid 65532)
- Docker multi-arch: `linux/amd64` + `linux/arm64`
- `sparklytics health` subcommand for `HEALTHCHECK CMD`
- `docker-compose.yml` for self-hosting
- GitHub Actions CI: fmt, clippy, test, audit, build, npm test, npm build, bundle size, docker buildx
- Pre-built binaries: `linux-amd64`, `linux-arm64`, `darwin-arm64`

[0.2.0]: https://github.com/sparklytics/sparklytics/releases/tag/v0.2.0
[0.1.0]: https://github.com/sparklytics/sparklytics/releases/tag/v0.1.0
