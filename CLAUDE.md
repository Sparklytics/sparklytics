# Sparklytics — AI Navigation Guide

**This file tells AI assistants where to find information in this repo.**
Read this file first. Then follow the pointers below to find what you need.

---

## Project Summary

Sparklytics is an open-source, self-hosted web analytics platform built in Rust.

- **Backend**: Rust (Axum 0.8, Tokio), DuckDB (self-hosted) or PostgreSQL + ClickHouse (cloud)
- **Frontend**: React 18 + Vite + TailwindCSS + shadcn/ui
- **SDK**: `@sparklytics/next` (npm package, Next.js App Router + Pages Router)
- **Self-hosted auth**: Argon2id password, JWT HttpOnly cookie, 3 modes (none/password/local)
- **Cloud auth**: Clerk (clerk-rs crate), Organizations = multi-tenancy
- **Multi-tenancy**: `tenant_id` column on every analytics table; NULL in self-hosted

---

## Master Index

**Always start here:**
→ [`docs/INDEX.md`](docs/INDEX.md)

This file has every document categorised by topic, a "when to read" column, and a quick-reference table of all key design decisions.

---

## Sprint Navigation

→ [`docs/sprints/sprint-index.md`](docs/sprints/sprint-index.md)

This is the source of truth for:
- Data model (DuckDB + ClickHouse + PostgreSQL schemas)
- API contracts (full request/response JSON for every endpoint)
- Auth flow diagrams
- Cargo.toml dependency versions
- Risk log

Individual sprint files:
| Sprint | File | Focus |
|--------|------|-------|
| 0 | [`docs/sprints/sprint-0.md`](docs/sprints/sprint-0.md) | Rust workspace, event collect, DuckDB |
| 1 | [`docs/sprints/sprint-1.md`](docs/sprints/sprint-1.md) | Query API, sessions, self-hosted auth |
| 2 | [`docs/sprints/sprint-2.md`](docs/sprints/sprint-2.md) | React dashboard, login UI |
| 3 | [`docs/sprints/sprint-3.md`](docs/sprints/sprint-3.md) | `@sparklytics/next` npm SDK |
| 4 | [`docs/sprints/sprint-4.md`](docs/sprints/sprint-4.md) | OSS launch, load tests, Docker |
| 5 | [`docs/sprints/sprint-5.md`](docs/sprints/sprint-5.md) | Clerk auth, cloud, PostgreSQL |

---

## Topic → Document Lookup

### "Where is the database schema?"
→ [`docs/08-DATABASE-SCHEMA.md`](docs/08-DATABASE-SCHEMA.md)
Covers DuckDB (self-hosted), ClickHouse (cloud analytics), and PostgreSQL (cloud metadata).
The PostgreSQL schema is the **authoritative version** for cloud tenants/users/websites/api_keys.

### "How does self-hosted auth work?"
→ [`docs/13-SELF-HOSTED-AUTH.md`](docs/13-SELF-HOSTED-AUTH.md)
Three modes: `SPARKLYTICS_AUTH=none|password|local`. Includes:
- Argon2id parameters, JWT structure, cookie flags
- All `/api/auth/*` endpoint contracts
- `local_api_keys` DuckDB table
- 40+ BDD scenarios
- Rust module layout (`crates/sparklytics-server/src/auth/`)

### "How does cloud/Clerk auth work?"
→ [`docs/sprints/sprint-5.md`](docs/sprints/sprint-5.md)
→ [`docs/sprints/sprint-index.md`](docs/sprints/sprint-index.md) (Auth Flow section)
Covers: `clerk-rs` Axum middleware, `TenantContext` extractor, JWT claim structure (`o.id` = tenant_id), Svix webhook verification, required env vars.

### "What are the API endpoints?"
→ [`docs/07-API-SPECIFICATION.md`](docs/07-API-SPECIFICATION.md) — overview + auth table
→ [`docs/sprints/sprint-index.md`](docs/sprints/sprint-index.md) — full JSON request/response shapes

### "What does the dashboard look like / which components to use?"
→ [`docs/14-BRAND-STYLE-GUIDE.md`](docs/14-BRAND-STYLE-GUIDE.md)
- Primary color: `#00D084` (electric green)
- Dark mode default
- shadcn/ui is the primary component library — check it before building custom
- React component tree is in [`docs/sprints/sprint-2.md`](docs/sprints/sprint-2.md)

### "How do I start coding? Day 1 guide?"
→ [`docs/06-UMAMI-MIGRATION-PLAN.md`](docs/06-UMAMI-MIGRATION-PLAN.md) — Part 10: Day 1 Coding Guide
Then follow: [`docs/sprints/sprint-0.md`](docs/sprints/sprint-0.md)

### "How does the Next.js SDK work?"
→ [`docs/sprints/sprint-3.md`](docs/sprints/sprint-3.md)
Package name: `@sparklytics/next`. Covers SparklyticsProvider, useSparklytics(), package structure, peerDependencies.

### "Who are the competitors and how do we compare?"
→ [`docs/02-COMPETITIVE-ANALYSIS.md`](docs/02-COMPETITIVE-ANALYSIS.md)
**Important:** Umami has ~6,400 GitHub stars (NOT 35K — old docs were wrong). PostHog HAS A/B testing. Our positioning is "no lightweight privacy-first self-hostable analytics tool has A/B testing built in."

### "What's the roadmap / what are the V1.1 and V1.2 features?"
→ [`docs/11-ROADMAP.md`](docs/11-ROADMAP.md)
- V1.1: Built-in A/B testing
- V1.2: Self-hosted auto-update daemon

### "What are the BDD test scenarios?"
Each sprint file contains Gherkin scenarios for that sprint's features.
Self-hosted auth BDD is in [`docs/13-SELF-HOSTED-AUTH.md`](docs/13-SELF-HOSTED-AUTH.md).

### "How are the repos structured? What's open source vs private?"
→ [`docs/15-REPO-STRATEGY.md`](docs/15-REPO-STRATEGY.md)
Two repos:
- **`sparklytics/sparklytics`** (public, MIT): all core code including Clerk auth + ClickHouse queries
- **`sparklytics/sparklytics-cloud`** (private): `sparklytics-billing` crate (Stripe), admin UI, ops configs
Billing is injected via the `BillingGate` trait defined in `sparklytics-core`. Self-hosted uses `NullBillingGate`. Cloud injects `StripeBillingGate` from the private crate.

### "What does the marketing site look like / what pages does it need?"
→ [`docs/16-MARKETING-SITE.md`](docs/16-MARKETING-SITE.md)
sparklytics.dev — Next.js 15, dogfoods `@sparklytics/next` SDK. Sections: hero, performance numbers, setup code demo, feature comparison table, pricing preview. Separate pages: `/pricing`, `/blog`, `/changelog`. Pre-launch checklist included.

---

## Critical Facts to Keep Consistent

These are the things most likely to create contradictions if not checked:

1. **API key prefixes**: `spk_selfhosted_` (self-hosted) vs `spk_live_` (cloud). Never mix them.
2. **`tenant_id` in self-hosted**: always `NULL`. Only set in cloud mode.
3. **`recent_events`**: the realtime API field is `recent_events` (NOT `recent_pageviews` — old bug, fixed).
4. **Bounce rate SQL**: must use CTEs. Correlated subqueries don't work in DuckDB. See [`docs/06-UMAMI-MIGRATION-PLAN.md`](docs/06-UMAMI-MIGRATION-PLAN.md) Part 3.
5. **Rate limit**: 60 req/min per IP on `/api/collect`. NOT 100.
6. **Session timeout**: 30 minutes inactivity, server-side, no cookies.
7. **Visitor ID**: `sha256(daily_salt + ip + user_agent)[0:16]` — 16 hex chars. Salt rotated at midnight UTC with 5-min grace period.
8. **ClickHouse ORDER BY**: `(tenant_id, website_id, created_at)` — tenant_id FIRST. Critical for multi-tenant isolation.
9. **`@sparklytics/next`**: this is the npm package name. NOT `@sparklytics/web` (old name, changed).
10. **Umami stars**: ~6,400, NOT 35K. Don't revert this.
11. **Visitor ID generation**: `sha256(salt_epoch + ip + user_agent)[0:16]` where `salt_epoch = floor(unix_timestamp / 86400)`. The ID is generated on first visit and **materialized client-side in localStorage with a 24h TTL**. The salt epoch is only used when generating a *new* visitor ID (empty localStorage). Existing IDs are read from localStorage directly — they do NOT get recalculated on each request. This means midnight UTC salt rotation never breaks in-progress sessions.
12. **DuckDB memory limit**: always set `SET memory_limit = '128MB'` at DuckDB init. The default (80% of system RAM) is not acceptable for a self-hosted binary on shared VPS. Note: DuckDB minimum is ~125MB per thread — this is a floor, not a target. Actual memory usage will be profiled during Sprint 4 benchmarks.
13. **Billing code is private**: `sparklytics-billing` (Stripe, plan enforcement, usage counters) lives in the private `sparklytics-cloud` repo. NEVER add billing logic to the public `sparklytics` repo. Plan limits are enforced via the `BillingGate` trait — `NullBillingGate` (public, always allows) vs `StripeBillingGate` (private, checks PostgreSQL). See [`docs/15-REPO-STRATEGY.md`](docs/15-REPO-STRATEGY.md).
14. **Marketing site tech**: sparklytics.dev is Next.js 15 App Router, deployed on Vercel, uses `@sparklytics/next` SDK on the site itself (dog food). Full spec in [`docs/16-MARKETING-SITE.md`](docs/16-MARKETING-SITE.md).
15. **`GET /api/auth/status` response**: Returns `{ "mode": "...", "setup_required": bool, "authenticated": bool }`. Returns 404 in `none` mode (endpoint not registered). Never returns 401. This is the only endpoint the frontend calls without auth to determine redirect destination.
16. **DuckDB does not enforce foreign keys**: `ON DELETE CASCADE` is declared in schema for documentation but DuckDB does not enforce it. Application code must explicitly delete child rows: events → sessions → website, in that order. Same for login_attempts cleanup.

---

## Environment Variables Reference

### Self-Hosted Mode
| Variable | Default | Description |
|----------|---------|-------------|
| `SPARKLYTICS_MODE` | `selfhosted` | `selfhosted` or `cloud` |
| `SPARKLYTICS_AUTH` | `local` | `none`, `password`, or `local` |
| `SPARKLYTICS_PASSWORD` | — | Required when `AUTH=password` |
| `SPARKLYTICS_SESSION_DAYS` | `7` | JWT expiry (1–30) |
| `SPARKLYTICS_HTTPS` | `true` | Set `false` for localhost (disables Secure cookie flag) |
| `SPARKLYTICS_RETENTION_DAYS` | `365` | DuckDB data retention |
| `SPARKLYTICS_CORS_ORIGINS` | — | Allowed origins for query endpoints |
| `SPARKLYTICS_ARGON2_MEMORY_KB` | `65536` | Argon2id memory parameter in KB (64MB default). Only relevant when `SPARKLYTICS_AUTH=local`. |
| `SPARKLYTICS_GEOIP_PATH` | `./GeoLite2-City.mmdb` | Path to MaxMind GeoLite2-City.mmdb file. If missing, geo fields stored as NULL (server still runs). |

### Cloud Mode (additional)
| Variable | Description |
|----------|-------------|
| `CLERK_SECRET_KEY` | Clerk backend secret |
| `CLERK_PUBLISHABLE_KEY` | Clerk frontend key |
| `CLERK_WEBHOOK_SIGNING_SECRET` | Svix webhook verification |
| `DATABASE_URL` | PostgreSQL connection string |
| `CLICKHOUSE_URL` | ClickHouse HTTP endpoint |
| `CLICKHOUSE_USER` | ClickHouse user |
| `CLICKHOUSE_PASSWORD` | ClickHouse password |

---

## Repo Layout (planned)

```
sparklytics/
├── CLAUDE.md                  ← you are here
├── CHANGELOG.md
├── README.md
├── Cargo.toml                 # workspace root
├── crates/
│   ├── sparklytics-core/      # Event structs, visitor ID, buffer
│   ├── sparklytics-duckdb/    # DuckDB backend + queries
│   └── sparklytics-server/    # Axum server, routes, auth middleware
├── dashboard/                 # React + Vite frontend
│   └── src/
├── sdk/                       # @sparklytics/next npm package
│   └── src/
├── migrations/                # sqlx PostgreSQL migrations (cloud)
├── GeoLite2-City.mmdb         # NOT included — user-provided (see SPARKLYTICS_GEOIP_PATH)
├── docs/
│   ├── INDEX.md               ← master doc index
│   ├── sprints/
│   │   ├── sprint-index.md    ← data model + API contracts
│   │   └── sprint-{0..5}.md
│   └── *.md                   ← all other docs
└── .github/
    └── workflows/
        └── ci.yml
```
