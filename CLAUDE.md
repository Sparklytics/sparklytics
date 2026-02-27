# Sparklytics — AI Navigation Guide

**This file tells AI assistants where to find information in this repo.**
Read this file first. Then follow the pointers below to find what you need.

---

## Project Summary

Sparklytics is an open-source, self-hosted web analytics platform built in Rust.

- **Backend**: Rust (Axum 0.8, Tokio), DuckDB (self-hosted) or PostgreSQL + ClickHouse (cloud)
- **Frontend**: Next 16 + TailwindCSS + shadcn/ui
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
| 2 | [`docs/sprints/sprint-2.md`](docs/sprints/sprint-2.md) | Next.js dashboard, login UI |
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

### "How do I launch Sparklytics for the first time?"
→ [`FIRST-LAUNCH-RUNBOOK.md`](FIRST-LAUNCH-RUNBOOK.md)
Step-by-step first-run guide for Docker/binary, mode-specific onboarding (`local` / `password` / `none`), first-event verification, and troubleshooting.

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
Three repos:
- **`sparklytics/sparklytics`** (public, MIT): core Rust crates, dashboard, DuckDB backend.
- **`sparklytics/sparklytics-cloud`** (private): cloud binary (`main.rs`), `cloud/src/auth/` (Clerk JWT middleware), `sparklytics-clickhouse` crate (ClickHouseBackend), `sparklytics-billing` crate (Stripe), ops configs, PostgreSQL migrations. Sprint 7 moves Clerk + ClickHouse here and removes `--features cloud` from public repo.
- **`sparklytics/sparklytics-next`** (public): `@sparklytics/next` npm SDK (extracted in Sprint 7 from `sdk/`).
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
7. **Visitor ID**: `sha256(salt_epoch + ip + user_agent)[0:16]` — 16 hex chars. `salt_epoch = floor(unix_timestamp / 86400)` — changes at midnight UTC. The `daily_salt` and `previous_salt` entries in the `settings` table are retained for documentation/grace-period purposes but are NOT used in the visitor ID computation (live code uses salt_epoch integer directly).
8. **ClickHouse ORDER BY**: `(tenant_id, website_id, created_at)` — tenant_id FIRST. Critical for multi-tenant isolation.
9. **`@sparklytics/next`**: this is the npm package name. NOT `@sparklytics/web` (old name, changed).
10. **Umami stars**: ~6,400, NOT 35K. Don't revert this.
11. **Visitor ID generation**: default visitor IDs are computed server-side as `sha256(salt_epoch + ip + user_agent)[0:16]`. Client-side `localStorage` override is only used when integrators call SDK `identify()` (stored under `sparklytics_visitor_id`); there is no built-in TTL rotation for that override.
12. **DuckDB memory limit**: configurable via `SPARKLYTICS_DUCKDB_MEMORY` env var (default `"1GB"`). Always set an explicit limit — the DuckDB default (80% of system RAM) is not acceptable for a server process. Accepts any DuckDB size string: `"512MB"`, `"1GB"`, `"4GB"`, etc. Modern 4–32 GB VPS instances can safely set 2–8 GB for better query performance. DuckDB minimum is ~125MB per thread.
13. **Billing code is private**: `sparklytics-billing` (Stripe, plan enforcement, usage counters) lives in the private `sparklytics-cloud` repo. NEVER add billing logic to the public `sparklytics` repo. Plan limits are enforced via the `BillingGate` trait — `NullBillingGate` (public, always allows) vs `StripeBillingGate` (private, checks PostgreSQL). See [`docs/15-REPO-STRATEGY.md`](docs/15-REPO-STRATEGY.md).
14. **Marketing site tech**: sparklytics.dev is Next.js 16 App Router, deployed on Vercel, uses `@sparklytics/next` SDK on the site itself (dog food). Full spec in [`docs/16-MARKETING-SITE.md`](docs/16-MARKETING-SITE.md).
15. **`GET /api/auth/status` response**: Returns `{ "mode": "...", "setup_required": bool, "authenticated": bool }`. Returns 404 in `none` mode (endpoint not registered). Never returns 401. This is the only endpoint the frontend calls without auth to determine redirect destination.
16. **DuckDB 1.4+ enforces foreign keys**: The events table has `FOREIGN KEY (website_id) REFERENCES websites(id)`. DuckDB 1.4.4 (current version) actively enforces this. Application code must delete child rows first (events → sessions → goals → website), **wrapped in a single transaction** so FK checks happen at commit time when the child rows are already gone. `delete_website()` uses `conn.transaction()` for this reason. Same care needed for login_attempts cleanup.

---

## UI Change Verification Protocol

**Any change to dashboard UI (components, layout, styles, tokens) must be verified visually before marking done.**

Steps after every UI edit:

1. **Start the dev server** — `cd dashboard && npm run dev` (Next.js dev server on :3001; `/api` rewrites to :3000 via `next.config.ts`)
2. **Take a desktop screenshot** — full viewport at 1440px wide. Use the `/screenshot` skill or system screenshot tool.
3. **Take a mobile screenshot** — viewport at 390px wide (iPhone 14 size). Toggle DevTools responsive mode.
4. **Check against design system** — run `/interface-design:audit` to catch spacing/color/depth violations before they accumulate.
5. **Verify these specifically**:
   - No box-shadows (borders-only depth strategy)
   - Numbers use IBM Plex Mono (`font-mono tabular-nums`)
   - Labels use Inter
   - Spacing on 4px grid (no off-grid values like py-2.5, px-1.5, mb-5)
   - Badge radius 2px (`rounded-sm`), card radius 8px (`rounded-lg`)
   - Active nav: 2px `--spark` left border, transparent background, `--ink` text
   - No hardcoded rgba colors outside the defined token set

**Mobile breakpoints to verify** (dashboard is responsive):
- 390px — single column, stat cards stack 1-col
- 768px — 2-col stat grid, sidebar collapses
- 1280px+ — 4-col stat grid, full sidebar

If you cannot start the dev server (e.g., no Node installed), describe the changes made and flag that visual verification is pending.

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
| `SPARKLYTICS_DUCKDB_MEMORY` | `1GB` | DuckDB memory limit. Any DuckDB size string (`"512MB"`, `"2GB"`, `"8GB"`). Set higher on 16–32 GB VPS for better analytics query performance. |
| `SPARKLYTICS_GEOIP_PATH` | `./GeoLite2-City.mmdb` | Path to MMDB GeoIP file. Docker images set this to bundled DB-IP at `/geoip/dbip-city-lite.mmdb`. MaxMind GeoLite2 is also supported. If missing, geo fields are stored as NULL (server still runs). |

### Cloud Mode (additional)
| Variable | Description |
|----------|-------------|
| `CLERK_SECRET_KEY` | Clerk backend secret |
| `NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY` | Clerk frontend key (inlined at `next build` time) |
| `CLERK_WEBHOOK_SIGNING_SECRET` | Svix webhook verification |
| `DATABASE_URL` | PostgreSQL connection string |
| `CLICKHOUSE_URL` | ClickHouse HTTP endpoint |
| `CLICKHOUSE_USER` | ClickHouse user |
| `CLICKHOUSE_PASSWORD` | ClickHouse password |
| `SPARKLYTICS_ENFORCE_BILLING` | **Required** in cloud binary. `stub` = Sprint 7 stub active (warns at startup, all tenants allowed). `true` = Sprint 8 enforcement active. Unset/other value = binary refuses to start. Never deploy cloud without setting this explicitly. |

---

## Multi-Repo Commit Workflow

This project uses **nested git repos** — the target architecture defined in Sprint 7. All repos live under `sparklytics/` on disk; each subdirectory has its own `.git/` and pushes to a separate GitHub remote. The parent `.gitignore` hides nested repos from the public tree.

> **Current on-disk state:**
> - `cloud/` exists as a nested git repo (`sparklytics-cloud`, private runtime)
> - `sdk/next/` exists as a nested git repo (`sparklytics-next`, public SDK)
> - Parent `.gitignore` excludes `cloud/` and `sdk/next/`

### Repo layout on disk

```
sparklytics/                    ← .git → github.com/Sparklytics/sparklytics  (PUBLIC CANONICAL)
├── .gitignore                  ← includes: cloud/   sdk/next/   (ignores nested repos)
├── crates/
├── dashboard/
├── cloud/                      ← .git → github.com/Sparklytics/sparklytics-cloud  (PRIVATE)
│   ├── crates/sparklytics-clickhouse/
│   ├── crates/sparklytics-billing/
│   ├── src/main.rs
│   ├── ops/                    ← fly.toml, clickhouse-setup.sh live here, NOT in public tree
│   └── migrations/             ← PostgreSQL migrations live here, NOT in public tree
└── sdk/
    └── next/                   ← .git → github.com/Sparklytics/sparklytics-next  (PUBLIC)
        └── package.json        ← @sparklytics/next
```

### One-time setup (only if nested repos are missing locally)

```bash
# Create nested repos:
mkdir -p cloud && git -C cloud init && git -C cloud remote add origin git@github.com:Sparklytics/sparklytics-cloud.git
mkdir -p sdk/next && git -C sdk/next init && git -C sdk/next remote add origin git@github.com:Sparklytics/sparklytics-next.git

# Update parent .gitignore — ensure:
#   cloud/
#   sdk/next/

# Verify:
git ls-files cloud/ sdk/next/   # must return empty
```

### Rules for committing

| Where you are | `git commit` goes to | Remote |
|---------------|----------------------|--------|
| `sparklytics/` (root) | Public canonical repo | `github.com/Sparklytics/sparklytics` |
| `sparklytics/cloud/` | Private cloud repo | `github.com/Sparklytics/sparklytics-cloud` |
| `sparklytics/sdk/next/` | Public SDK repo | `github.com/Sparklytics/sparklytics-next` |

**Critical rules:**
1. **Never `git add -A` or `git add .` from `sparklytics/` root.** `cloud/` and `sdk/next/` are gitignored, but other new files at the root can be staged accidentally. Always stage by explicit path: `git add crates/ dashboard/ Cargo.toml`.
2. **Always check `git remote -v` before pushing** to confirm you are in the right nested repo.
3. **`cloud/` and `sdk/next/` must appear in `.gitignore`** — verify with `git ls-files cloud/ sdk/next/` (must return empty).
4. **Billing, ClickHouseBackend code, Clerk auth, ops configs, and PostgreSQL migrations belong in `cloud/`** — never commit them to the public `sparklytics/` root.
5. **`cloud/Cargo.toml` uses path deps** (`../../crates/sparklytics-core`) for local development. These work because `cloud/` is nested inside `sparklytics/`. CI overrides with git tags via `cloud/.cargo/config.toml` (which must be in `cloud/.gitignore`).

### Repo Layout (public tree)

```
sparklytics/
├── CLAUDE.md                  ← you are here
├── CHANGELOG.md
├── README.md
├── Cargo.toml                 # workspace root
├── crates/
│   ├── sparklytics-core/      # AnalyticsBackend trait, BillingGate trait, Event structs
│   ├── sparklytics-duckdb/    # DuckDB backend + queries, implements AnalyticsBackend
│   └── sparklytics-server/    # Axum server, routes, auth middleware
├── dashboard/                 # Next.js 15 App Router (static export → out/)
│   ├── app/
│   ├── components/
│   ├── hooks/
│   └── lib/
├── sdk/
│   └── next/                  # GITIGNORED nested repo (@sparklytics/next)
├── cloud/                     # GITIGNORED nested private repo (cloud binary)
├── GeoLite2-City.mmdb         # NOT included — user-provided (see SPARKLYTICS_GEOIP_PATH)
├── docs/
│   ├── INDEX.md               ← master doc index
│   ├── sprints/
│   │   ├── sprint-index.md    ← data model + API contracts
│   │   └── sprint-{0..30}.md
│   └── *.md                   ← all other docs
└── .github/
    └── workflows/
        └── ci.yml
```
