# Umami Codebase Analysis

**Source:** github.com/umami-software/umami (v3.0.3, February 2026)
**Purpose:** Understand Umami's architecture to inform Sparklytics design decisions.

## Repository Overview

Umami is a monorepo built with Next.js. The entire application (frontend + backend + API) lives in one Next.js project using API routes for the backend.

**Key stats (v3):**
- 35,200+ GitHub stars
- Node.js 18+ required
- PostgreSQL 12+ only (MySQL dropped in v3)
- pnpm as package manager
- React 19 + Next.js 16

## Architecture

### Monolith Design

Umami runs as a single Next.js application. This means:
- Frontend (React pages) and Backend (API routes) in one process
- No separate API server
- PostgreSQL is the only external dependency
- Session state in memory

This is simpler to deploy but limits scaling options. You can't scale the API independently from the dashboard.

### Database Layer

Umami uses Prisma ORM with PostgreSQL:
- `User` table: accounts with bcrypt passwords
- `Website` table: tracked websites (both integer ID and UUID)
- `Session` table: visitor sessions with device/location data
- `WebsiteEvent` table: individual pageview and event records

Key design: Umami uses both integer IDs (for foreign keys, storage efficiency) and UUIDs (for API exposure, prevents enumeration). Smart pattern.

### Tracking Script

The tracking script (`script.js`) is under 2KB and does:
1. Creates a unique visitor hash (using `Date.now()` and random values)
2. Detects SPA navigation via `History.pushState` and `History.replaceState` monkey-patching
3. Sends events via `fetch()` with `keepalive` flag (v3 change)
4. Falls back to `XMLHttpRequest` when `keepalive` not available
5. Reads configuration from `data-*` attributes on the script tag
6. Supports custom events via `umami.track()` global function
7. Supports user identification via `umami.identify()` (v3)

### Event Collection

The collection endpoint (`POST /api/send`) receives events and:
1. Extracts IP from headers (X-Forwarded-For chain)
2. Parses User-Agent
3. Looks up GeoIP data
4. Creates or updates session
5. Inserts event into database
6. Returns 200 OK

No buffering - each event is a separate database INSERT. This is simpler but less efficient than batching.

### Dashboard

Built with React and `@umami/react-zen` (their custom component library). Uses Chart.js for visualization. Dashboard queries hit the same Next.js API routes.

v3 Dashboard layout:
- Top bar: website selector, date range
- Stats cards: pageviews, visitors, bounces, time on page
- Main chart: pageviews over time
- Tabbed sections: pages, referrers, browsers, OS, devices, countries, regions, cities, languages, screens
- New sections: events, goals, funnels, segments, cohorts

## What We Learn From Umami

### Patterns to Copy

1. **Dual-ID pattern:** Integer for internal FK, UUID for external API. We use string IDs with prefix (`site_`, `usr_`) instead.

2. **Script configuration via data attributes:** `data-website-id` is clean. We keep this pattern.

3. **SPA detection via History API patching:** The standard approach. We do the same.

4. **Dashboard layout:** Top stats -> chart -> breakdowns. It's what users expect.

5. **Website management CRUD:** Simple, necessary, well-designed in Umami.

### Patterns to Improve

1. **No event batching:** Umami writes each event individually. We batch (100 events or 5 seconds).

2. **Node.js overhead:** 240-500MB RAM for the process. Rust targets <100MB.

3. **PostgreSQL dependency:** Requires a separate database server. DuckDB eliminates this.

4. **No framework SDK:** Generic script tag only. We add `@sparklytics/next`.

5. **Prisma overhead:** ORM adds latency and complexity. We use raw SQL.

6. **Monolithic scaling:** Can't scale API independently. We separate concerns.

## Umami V3 New Features (For Reference)

### Segments
Saved filter combinations that users can name and reapply. We could add this in V2.

### Cohorts
Group users by time and activity. "Users who signed up in January and visited 5+ times." Advanced, defer to V2.

### Links
Short URLs for measuring clicks/downloads. Interesting but not core analytics.

### Pixels
Invisible tracking images for email open rates. Niche use case.

### Compare Feature
Compare two custom date ranges for A/B-style analysis. Useful, add to our date range picker in V1.1.

## Performance Observations

Reported by users:
- Dashboard loads: 2-5 seconds under moderate load
- Memory: 240MB idle, spikes to 500MB+ under load
- CPU: ~1% idle, spikes during complex queries
- Database: PostgreSQL requires regular VACUUM for performance

Our targets are significantly more aggressive:
- Dashboard loads: <2s cold, <500ms warm
- Memory: <100MB at 10K events/day
- CPU: <5% average
- Database: zero maintenance (DuckDB self-manages)

## API Endpoints Reference

For feature parity, we need to match these core Umami endpoints:

| Umami Endpoint | Our Equivalent | Status |
|----------------|----------------|--------|
| POST /api/send | POST /api/collect | V1 |
| GET /api/websites | GET /api/websites | V1 |
| POST /api/websites | POST /api/websites | V1 |
| GET /api/websites/:id/stats | GET /api/websites/:id/stats | V1 |
| GET /api/websites/:id/pageviews | GET /api/websites/:id/pageviews | V1 |
| GET /api/websites/:id/metrics | GET /api/websites/:id/metrics | V1 |
| GET /api/websites/:id/active | GET /api/websites/:id/realtime | V1 |
| POST /api/auth/login | POST /api/auth/login | V1 (cloud) |
| GET /api/me | GET /api/auth/me | V1 (cloud) |
