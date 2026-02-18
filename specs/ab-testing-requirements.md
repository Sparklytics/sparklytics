# A/B Testing Requirements Specification

**Version:** 1.0
**Target:** V1.1 (Week 8-9)
**Status:** Planning

## Problem

Developers want simple A/B testing integrated with their analytics. Current options are either dead (Google Optimize), expensive ($50K+/year Optimizely), complex (VWO), or a separate tool requiring another integration (GrowthBook). PostHog has A/B testing but costs $450+/month and is overkill.

No open-source analytics platform has built-in A/B testing. This is our moat.

## Solution

Built-in experiment framework that uses the same tracking infrastructure as analytics. Client-side variant assignment (deterministic, no cookies), server-side statistical analysis (ClickHouse chi-squared functions for cloud, DuckDB equivalent for self-hosted).

## Architecture

```
Client (useExperiment hook)
  │
  ├─ Deterministic variant assignment: hash(visitor_id + experiment_id) → variant
  ├─ Send exposure event: POST /api/collect { type: "experiment", experiment_id, variant }
  └─ On conversion: POST /api/collect { type: "conversion", experiment_id, variant, event_name }

Server (Rust backend)
  │
  ├─ Store in experiments table (same ingestion pipeline as events)
  ├─ Materialized view for experiment stats (ClickHouse)
  └─ Statistical significance calculation (chi-squared test)

Dashboard
  │
  ├─ Experiment list: name, status, variants, date range
  ├─ Results: exposures, conversions, rates per variant
  ├─ Statistical significance indicator (p-value < 0.05 = significant)
  └─ Winner declaration with confidence interval
```

## Database Schema

### DuckDB (Self-Hosted)

```sql
-- Experiment exposures
CREATE TABLE IF NOT EXISTS experiment_exposures (
    id              VARCHAR NOT NULL,
    website_id      VARCHAR NOT NULL,
    visitor_id      VARCHAR NOT NULL,
    experiment_id   VARCHAR NOT NULL,
    variant         VARCHAR NOT NULL,
    url             VARCHAR,
    created_at      TIMESTAMP NOT NULL,
    FOREIGN KEY (website_id) REFERENCES websites(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_exp_website_experiment
    ON experiment_exposures (website_id, experiment_id, created_at);

-- Experiment conversions
CREATE TABLE IF NOT EXISTS experiment_conversions (
    id              VARCHAR NOT NULL,
    website_id      VARCHAR NOT NULL,
    visitor_id      VARCHAR NOT NULL,
    experiment_id   VARCHAR NOT NULL,
    variant         VARCHAR NOT NULL,
    conversion_name VARCHAR NOT NULL,
    conversion_data VARCHAR,           -- JSON string
    revenue         DOUBLE,            -- optional revenue value
    created_at      TIMESTAMP NOT NULL,
    FOREIGN KEY (website_id) REFERENCES websites(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_conv_website_experiment
    ON experiment_conversions (website_id, experiment_id, created_at);

-- Experiment definitions (managed via dashboard)
CREATE TABLE IF NOT EXISTS experiments (
    id              VARCHAR PRIMARY KEY,
    website_id      VARCHAR NOT NULL,
    name            VARCHAR NOT NULL,
    status          VARCHAR NOT NULL DEFAULT 'draft',  -- draft, running, paused, completed
    variants        VARCHAR NOT NULL,                  -- JSON array: ["control", "variant-a"]
    weights         VARCHAR,                           -- JSON array: [50, 50]
    goal_event      VARCHAR,                           -- conversion event name to track
    started_at      TIMESTAMP,
    ended_at        TIMESTAMP,
    created_at      TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (website_id) REFERENCES websites(id) ON DELETE CASCADE
);
```

### ClickHouse (Cloud)

```sql
CREATE TABLE IF NOT EXISTS experiment_exposures (
    id              UUID,
    website_id      String,
    visitor_id      String,
    experiment_id   String,
    variant         LowCardinality(String),
    url             Nullable(String),
    created_at      DateTime64(3, 'UTC')
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (website_id, experiment_id, created_at, visitor_id)
TTL toDateTime(created_at) + INTERVAL 12 MONTH;

CREATE TABLE IF NOT EXISTS experiment_conversions (
    id              UUID,
    website_id      String,
    visitor_id      String,
    experiment_id   String,
    variant         LowCardinality(String),
    conversion_name String,
    conversion_data Nullable(String),
    revenue         Nullable(Float64),
    created_at      DateTime64(3, 'UTC')
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (website_id, experiment_id, created_at, visitor_id)
TTL toDateTime(created_at) + INTERVAL 12 MONTH;

-- Materialized view for real-time experiment results
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_experiment_results
ENGINE = AggregatingMergeTree()
ORDER BY (website_id, experiment_id, variant)
AS
SELECT
    website_id,
    experiment_id,
    variant,
    uniqState(visitor_id) AS unique_visitors,
    countState() AS total_exposures
FROM experiment_exposures
GROUP BY website_id, experiment_id, variant;
```

## Statistical Significance

### Architecture: Application-Layer, Not SQL

**All significance calculations run in Rust, not in SQL.** The DB (DuckDB or ClickHouse) only returns raw counts: `(exposures, conversions)` per variant. Rust then runs the statistical test. This applies to both backends equally — no separate SQL-level chi-squared implementations.

The `statrs` crate provides the chi-squared distribution. Add to Cargo.toml:
```toml
statrs = "0.17"
```

### Implementation (Rust)

```rust
use statrs::distribution::{ChiSquared, ContinuousCDF};

/// Run a chi-squared test across all variants.
/// `data`: slice of (conversions, exposures) per variant.
/// Returns p-value. Guard: returns 1.0 (not significant) if inputs are degenerate.
fn chi_squared_p_value(data: &[(u64, u64)]) -> f64 {
    let total_conv: u64 = data.iter().map(|&(c, _)| c).sum();
    let total_exp: u64 = data.iter().map(|&(_, e)| e).sum();

    // Guard: zero total exposures or zero total conversions → not testable
    if total_exp == 0 || total_conv == 0 {
        return 1.0;
    }
    let expected_rate = total_conv as f64 / total_exp as f64;

    let mut chi_sq = 0.0;
    for &(conversions, exposures) in data {
        if exposures == 0 { return 1.0; } // Guard: empty variant
        let exp_conv = expected_rate * exposures as f64;
        let exp_no_conv = (1.0 - expected_rate) * exposures as f64;
        // Guard: avoid division by zero when expected cell count is ~0
        if exp_conv < 1e-9 || exp_no_conv < 1e-9 { return 1.0; }
        chi_sq += (conversions as f64 - exp_conv).powi(2) / exp_conv;
        chi_sq += ((exposures - conversions) as f64 - exp_no_conv).powi(2) / exp_no_conv;
    }

    let degrees_of_freedom = (data.len() - 1) as f64;
    let dist = ChiSquared::new(degrees_of_freedom).expect("df >= 1 guaranteed by variant count check");
    1.0 - dist.cdf(chi_sq)
}
```

**SQL queries** (same shape for DuckDB and ClickHouse — just raw counts, no stats in SQL):

```sql
-- DuckDB
SELECT variant,
       COUNT(DISTINCT e.visitor_id) AS exposures,
       COUNT(DISTINCT c.visitor_id) AS conversions
FROM experiment_exposures e
LEFT JOIN experiment_conversions c
    ON e.visitor_id = c.visitor_id AND e.experiment_id = c.experiment_id
WHERE e.website_id = ? AND e.experiment_id = ?
GROUP BY variant;
```

```sql
-- ClickHouse (same logic, different distinct count syntax)
SELECT variant,
       uniq(e.visitor_id) AS exposures,
       uniqIf(c.visitor_id, c.visitor_id != '') AS conversions
FROM experiment_exposures e
LEFT JOIN experiment_conversions c
    ON e.visitor_id = c.visitor_id AND e.experiment_id = c.experiment_id
WHERE e.website_id = ? AND e.experiment_id = ?
GROUP BY variant;
```

### Minimum Sample Threshold

Before showing p-value, require **both**:
- ≥100 **conversions** per variant (not just exposures — low conversion counts make the test unreliable)
- ≥100 **exposures** per variant

Below either threshold, show "Collecting data..." in the dashboard. Never display a p-value from insufficient data.

### Weights Validation

Variant weights **must sum to exactly 100**. Enforce at experiment creation:

```rust
let total: u32 = weights.iter().sum();
if total != 100 {
    return Err(SparkError::Validation {
        message: format!("Variant weights must sum to 100, got {}", total),
        field: Some("weights".into()),
    });
}
```

### Confidence Intervals

Display 95% confidence interval for each variant's conversion rate. Uses the Wilson score interval (more accurate than the normal approximation at low conversion rates):

```
p̂ = conversions / exposures
z = 1.96 (95% CI)
center = (p̂ + z²/2n) / (1 + z²/n)
margin = z * sqrt(p̂(1-p̂)/n + z²/(4n²)) / (1 + z²/n)
CI = [center - margin, center + margin]
```

### ⚠️ Peeking Problem

Chi-squared is a **fixed-horizon test** — it is only valid if you decide the sample size *before* running the experiment and evaluate significance exactly once. Checking daily and stopping when `p < 0.05` is p-hacking and will produce false positives at a rate much higher than 5%.

For V1.1, mitigate this by:
1. **Requiring a planned duration** when creating an experiment (e.g., "run for 14 days")
2. **Only computing significance on the planned end date**, not before
3. Dashboard shows a countdown: "Results in 7 days" — greys out significance until duration is reached

Sequential testing (SPRT) that is valid for early stopping is planned for V2 (see Future section).

## API Endpoints

### GET /api/websites/:website_id/experiments

List all experiments.

```json
{
  "data": [
    {
      "id": "exp_abc123",
      "name": "Pricing Test",
      "status": "running",
      "variants": ["control", "higher-price"],
      "weights": [50, 50],
      "goal_event": "purchase",
      "started_at": "2026-02-10T00:00:00Z",
      "results": {
        "control": {
          "exposures": 1245,
          "conversions": 89,
          "conversion_rate": 0.0715,
          "confidence_interval": [0.0573, 0.0857]
        },
        "higher-price": {
          "exposures": 1198,
          "conversions": 124,
          "conversion_rate": 0.1035,
          "confidence_interval": [0.0863, 0.1207]
        },
        "significant": true,
        "p_value": 0.0042,
        "winner": "higher-price",
        "lift": "+44.8%"
      }
    }
  ]
}
```

### POST /api/websites/:website_id/experiments

Create a new experiment.

```json
{
  "name": "Pricing Test",
  "variants": ["control", "higher-price"],
  "weights": [50, 50],
  "goal_event": "purchase"
}
```

### PUT /api/websites/:website_id/experiments/:experiment_id

Update experiment (start, pause, complete).

### DELETE /api/websites/:website_id/experiments/:experiment_id

Delete experiment and all associated data.

## Dashboard UI

### Experiments List Page

Table showing: experiment name, status badge (draft/running/paused/completed), variants count, exposures, conversion rate, significance indicator, start date.

### Experiment Detail Page

```
┌─────────────────────────────────────────────────────┐
│ Pricing Test                    [Running] [Stop]     │
│ Started: Feb 10, 2026 | Running: 7 days              │
├─────────────────────────────────────────────────────┤
│                                                      │
│  Variant         Exposures  Conversions  Rate        │
│  ─────────────────────────────────────────────       │
│  control         1,245      89           7.15%       │
│  higher-price    1,198      124          10.35%      │
│                                                      │
│  ┌─────────────────────────────────────────────┐    │
│  │  Statistical Significance: YES (p=0.004)    │    │
│  │  Winner: higher-price (+44.8% lift)         │    │
│  │  Confidence: 95%                            │    │
│  └─────────────────────────────────────────────┘    │
│                                                      │
│  [Conversion Rate Over Time - Line Chart]            │
│  ──── control (7.15%)                                │
│  ──── higher-price (10.35%)                          │
│                                                      │
└─────────────────────────────────────────────────────┘
```

## Limitations (V1.1)

- Maximum 4 variants per experiment (including control)
- Maximum 10 concurrent experiments per website
- Client-side assignment only (no server-side rendering of variants)
- No multivariate testing (one variable per experiment)
- No traffic allocation (all traffic sees the experiment)
- Fixed-horizon testing only (no early stopping) — mitigated by requiring planned duration at creation
- No revenue currency field — `revenue DOUBLE` assumes single currency; multi-currency is V2

## Future (V2)

- Sequential testing (SPRT) — valid for checking results before the planned end date
- Server-side variant assignment via Edge Middleware
- Multivariate testing (multiple variables)
- Traffic allocation (run experiment on X% of traffic)
- Mutually exclusive experiment groups
- Multi-currency revenue tracking
