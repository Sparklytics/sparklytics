# Performance Benchmarks & Targets

**Purpose:** Define measurable performance targets and how to verify them.
**Updated:** 2026-02-17

## Benchmark Categories

### 1. Event Ingestion

How fast can we receive and store events?

**Target:**
- 100 events/sec sustained on 1-core VPS (self-hosted)
- 1,000 events/sec sustained on cloud infrastructure
- <50ms p99 response time for POST /api/collect

**How to measure:**

```bash
# k6 load test for event ingestion
k6 run --vus 10 --duration 60s scripts/bench-collect.js
```

```javascript
// scripts/bench-collect.js
import http from 'k6/http';

export default function() {
  const payload = JSON.stringify({
    website_id: 'site_test',
    type: 'pageview',
    url: `/page-${Math.floor(Math.random() * 1000)}`,
    referrer: 'https://google.com',
    screen: '1920x1080',
    language: 'en-US',
  });

  http.post('http://localhost:3000/api/collect', payload, {
    headers: { 'Content-Type': 'application/json' },
  });
}
```

**Comparison targets:**

| Platform | Ingestion Rate | Source |
|----------|---------------|--------|
| Umami | ~50 events/sec (single instance) | Community reports |
| Plausible | ~200 events/sec (Elixir) | Estimated |
| Sparklytics (target) | 100-1,000 events/sec | Based on Rust + buffering |

### 2. Dashboard Query Performance

How fast do analytics queries return?

**Target:**
- <200ms p99 for summary stats
- <500ms p99 for time series (30 days)
- <200ms p99 for top-10 breakdowns
- <100ms for real-time active visitors

**Test data sizes:**

| Dataset | Events | Size (DuckDB) | Use Case |
|---------|--------|---------------|----------|
| Small | 10,000 | ~2MB | Personal blog |
| Medium | 100,000 | ~15MB | Small SaaS |
| Large | 1,000,000 | ~150MB | Medium site |
| XL | 10,000,000 | ~1.5GB | Large site |

**Benchmark queries:**

```sql
-- Summary stats (most common)
SELECT
  count(*) as pageviews,
  count(DISTINCT visitor_id) as visitors,
  count(DISTINCT session_id) as sessions
FROM events
WHERE website_id = 'site_test'
  AND created_at BETWEEN '2026-01-17' AND '2026-02-17';

-- Time series (chart data)
SELECT
  date_trunc('day', created_at) as date,
  count(*) as pageviews,
  count(DISTINCT visitor_id) as visitors
FROM events
WHERE website_id = 'site_test'
  AND created_at BETWEEN '2026-01-17' AND '2026-02-17'
GROUP BY date
ORDER BY date;

-- Top pages breakdown
SELECT
  url,
  count(*) as pageviews,
  count(DISTINCT visitor_id) as visitors
FROM events
WHERE website_id = 'site_test'
  AND created_at BETWEEN '2026-01-17' AND '2026-02-17'
  AND event_type = 'pageview'
GROUP BY url
ORDER BY pageviews DESC
LIMIT 10;
```

**How to measure:**

```rust
// Rust criterion benchmark
fn bench_stats_query(c: &mut Criterion) {
    let backend = setup_duckdb_with_data(1_000_000);

    c.bench_function("stats_query_1M", |b| {
        b.iter(|| {
            let _ = backend.query_stats(&StatsQuery {
                website_id: "site_test",
                date_range: DateRange::Last30Days,
                ..Default::default()
            });
        });
    });
}
```

### 3. Memory Usage

**Target:**

| Workload | Max RAM | Measurement |
|----------|---------|-------------|
| Idle (no traffic) | <150MB | `docker stats` after 5 min (DuckDB min is 125MB/thread; hard floor) |
| 1K events/day | <180MB | 24h test with simulated traffic |
| 10K events/day | <200MB | 24h test |
| 100K events/day | <350MB | 24h test |

**Note:** DuckDB must be initialised with `SET memory_limit = '128MB'` to prevent it from consuming 80% of system RAM by default. Without this, idle memory is unbounded.

**Comparison:**

| Platform | Idle RAM | Under Load |
|----------|---------|------------|
| Umami (Node.js) | 240MB | 300-500MB |
| Plausible (Elixir) | 200MB | 300-400MB |
| PostHog (Python) | 500MB | 700MB+ |
| Liwan (Rust/DuckDB) | ~150MB | ~200MB |
| Sparklytics (target) | <150MB | <350MB |

### 4. Tracking Script Performance

**Target:**
- Script size: <3KB gzipped
- Load time: <100ms
- No layout shifts (CLS = 0)
- No impact on LCP
- Zero JavaScript errors

**How to measure:**

```bash
# Size check
gzip -c dist/s.js | wc -c  # Must be <3072 bytes

# Lighthouse impact
# Run Lighthouse on a test page with and without script
# Difference in Performance score should be <5 points
```

### 5. Dashboard UI Performance

**Target:**
- First Contentful Paint: <1.5s
- Largest Contentful Paint: <2.0s
- Time to Interactive: <2.5s
- Cumulative Layout Shift: <0.1
- Total bundle size (gzipped): <200KB

**How to measure:**

```bash
# Lighthouse CI
npx lhci autorun --config=lighthouserc.json
```

### 6. Startup Time

**Target:** Server ready to accept requests in <3 seconds.

Breakdown:
- Binary load: <100ms
- DuckDB initialization: <500ms
- GeoIP database load: <1s
- Schema migration check: <500ms
- HTTP server bind: <100ms

**How to measure:**

```bash
# Time from process start to first successful health check
time (./sparklytics &; while ! curl -s http://localhost:3000/health > /dev/null; do sleep 0.1; done)
```

## Benchmark Infrastructure

### CI Benchmarks

Run on every PR to catch regressions:
- criterion microbenchmarks (Rust)
- k6 load tests (HTTP)
- Docker image size check
- Bundle size check (dashboard)
- Lighthouse CI (dashboard)

### Baseline Benchmarks

Run weekly on consistent hardware:
- Hetzner CX22 (2 vCPU, 4GB RAM) - our "reference self-hosted" machine
- Multiple dataset sizes (10K, 100K, 1M events)
- Results published in GitHub Wiki or docs

## Performance Budget Violations

If any benchmark exceeds its target:
1. Create a GitHub issue tagged `performance`
2. Investigate root cause
3. Fix before next release
4. Add regression test

Critical violations (block release):
- Memory >350MB at 10K events/day
- Dashboard load >3s
- Event ingestion <50 events/sec
- Tracking script >5KB gzipped

## Data Generation for Benchmarks

```rust
// Generate realistic test data
fn generate_test_events(count: usize) -> Vec<Event> {
    let pages = vec!["/", "/blog", "/pricing", "/docs", "/about", "/contact"];
    let referrers = vec!["google.com", "twitter.com", "github.com", "(direct)"];
    let countries = vec!["US", "DE", "GB", "FR", "PL", "NL", "CA", "AU"];
    let browsers = vec!["Chrome", "Firefox", "Safari", "Edge"];
    let devices = vec!["desktop", "mobile", "tablet"];

    (0..count).map(|i| {
        Event {
            id: Uuid::new_v4().to_string(),
            website_id: "site_bench".into(),
            session_id: format!("sess_{}", i / 5),  // ~5 pages per session
            visitor_id: format!("vis_{}", i / 15),   // ~3 sessions per visitor
            event_type: "pageview".into(),
            url: pages[i % pages.len()].into(),
            referrer_domain: Some(referrers[i % referrers.len()].into()),
            country: Some(countries[i % countries.len()].into()),
            browser: Some(browsers[i % browsers.len()].into()),
            device_type: Some(devices[i % devices.len()].into()),
            created_at: Utc::now() - Duration::seconds((count - i) as i64 * 10),
            ..Default::default()
        }
    }).collect()
}
```

## Reporting

Benchmark results are tracked in:
1. GitHub Actions CI output (every PR)
2. `BENCHMARKS.md` in repo root (updated on release)
3. Landing page (marketing: "3x less RAM than Umami")

Compare against Umami's community-reported numbers (they have no official benchmarks). Our honest claim: "predictable, capped memory usage under load" — not a specific MB figure, since Umami's numbers vary wildly by configuration.
