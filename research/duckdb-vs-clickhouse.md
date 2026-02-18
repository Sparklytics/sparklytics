# DuckDB vs ClickHouse for Web Analytics

**Purpose:** Inform the dual-backend architecture decision.
**Updated:** 2026-02-17

## Overview

Both databases are columnar and optimized for analytical workloads. The key difference: DuckDB is embedded (in-process, no server) while ClickHouse is a distributed database server.

## DuckDB Deep Dive

### Architecture
- Embedded, in-process database (like SQLite, but columnar)
- No external server needed
- Single file storage
- MVCC with optimistic concurrency control
- Vectorized query execution with SIMD instructions

### Performance Characteristics
- 100-1000x faster than SQLite for analytical queries
- 3-8x faster than Apache Spark on reasonably-sized datasets
- Leverages CPU L1/L2 caches for fast aggregation
- No code generation/JIT overhead = fast cold starts

### Memory Model
- Minimum: 125MB per thread
- Default: 80% of system RAM
- Workload-specific: 1-2GB per thread for aggregations, 3-4GB for joins
- Larger-than-memory support: disk spilling for datasets exceeding RAM
- Limitation: some aggregate functions (list(), string_agg()) don't support disk offloading

### Concurrency
- Single-writer model (MVCC within single process)
- Multiple concurrent readers
- Not suitable for high-concurrency write workloads
- For our use case: writer = event buffer flush, readers = dashboard queries. This works.

### Practical Limits
- No practical file size limit (15TB+ databases tested)
- Single-node only (no distributed queries)
- Practical sweet spot: <100GB datasets
- Our estimate: good for sites up to ~1M events/day

### Rust Integration (duckdb-rs)
- Official crate, actively maintained
- `bundled` feature compiles DuckDB from source (no system dependency)
- API inspired by rusqlite (familiar to Rust developers)
- Supports: Arrow, Parquet, JSON, CSV
- Can create custom scalar and table functions

### Configuration for Our Use Case
```sql
SET memory_limit = '128MB';      -- Hard cap for self-hosted (DuckDB default is 80% of RAM — unsafe on shared VPS)
SET threads = 2;                  -- Limit CPU usage
SET checkpoint_threshold = '1GB'; -- When to write to disk
```
**Important:** Never rely on DuckDB's default memory limit. The default (`80% of system RAM`) will consume all available memory on a 512MB VPS. Always set `memory_limit` at startup.

## ClickHouse Deep Dive

### Architecture
- Distributed column-oriented DBMS
- Client-server model (HTTP and native TCP protocols)
- MergeTree engine family for ordered data storage
- Materialized views for incremental aggregation
- Partitioning for data lifecycle management

### Performance Characteristics
- 2-3M events/second ingestion (multi-threaded)
- Millisecond queries on billions of rows
- 10:1 to 30:1 compression ratios (LZ4/ZSTD)
- Linear scaling with additional cluster nodes
- Point lookups ~15ms (not optimized for, but acceptable)

### Resource Requirements (Self-Hosted)
- Minimum: 4 cores, 16GB RAM
- Recommended: 8-16 cores, 64-128GB RAM
- HA setup: minimum 2 nodes with 16+ cores, 64+ GB each
- Our plan: Hetzner CPX31 (4 vCPU, 8GB RAM) = EUR 16/month

### LowCardinality Optimization
ClickHouse has a special `LowCardinality` type that dictionary-encodes columns with few distinct values. Perfect for: event_type, country, browser, os, device_type, language. Reduces storage by 5-10x for these columns.

### Materialized Views
Pre-compute aggregations on INSERT. Instead of scanning the full events table for "daily pageviews per website," a materialized view maintains running totals. Dashboard queries hit the materialized view (milliseconds) instead of the raw table (seconds).

### Multi-Tenancy
Options for tenant isolation:
1. **Row-level filtering:** WHERE website_id = ? (our approach)
2. **Separate databases:** One database per customer (overkill for our scale)
3. **Separate tables:** One table per customer (operational nightmare)

Row-level filtering with ORDER BY starting with website_id is efficient because ClickHouse can skip entire data granules for non-matching website_ids.

### Rust Integration
Multiple crates available:
1. **clickhouse-rs (official):** HTTP protocol, serde-based, schema validation
2. **klickhouse:** Native protocol, maximum performance
3. **suharev7/clickhouse-rs:** Async/await with tokio

We use the official `clickhouse-rs` for stability and support.

### ClickHouse Cloud vs Self-Hosted
- Cloud: starts at ~$172/month. Too expensive for launch.
- Self-hosted minimum: **CPX51 (8 vCPU, 16GB RAM) on Hetzner — EUR 42/month**. CPX31 (8GB RAM) is below ClickHouse's minimum 16GB requirement and will OOM under load.
- Cloud advantage: auto-scaling, no ops. Consider after EUR 1K+ MRR.
- **Launch decision: don't use ClickHouse at all for cloud V1.** Use DuckDB-per-tenant (see cloud-platform-requirements.md). Add ClickHouse only when a tenant exceeds 5M events/month.

## Head-to-Head Comparison

| Aspect | DuckDB | ClickHouse |
|--------|--------|-----------|
| **Deployment** | Embedded (no server) | Client-server (HTTP/TCP) |
| **Min resources** | 125MB RAM | 4 cores, 16GB RAM |
| **Scaling** | Vertical only (single node) | Horizontal (cluster) |
| **Write model** | Single writer | Multi-writer |
| **Read concurrency** | Multiple readers | Highly concurrent |
| **Compression** | Good (10-20x) | Excellent (10-30x) |
| **Materialized views** | No | Yes (incremental) |
| **Partitioning** | No | By month/day |
| **TTL (auto-delete)** | Manual (DELETE query) | Native (TTL clause) |
| **Maintenance** | VACUUM, CHECKPOINT | OPTIMIZE TABLE |
| **SQL dialect** | PostgreSQL-like | ClickHouse SQL |
| **Rust crate** | duckdb-rs (official) | clickhouse-rs (official) |
| **Cost** | $0 (embedded) | EUR 16+/month (Hetzner) |
| **Best for** | <1M events/day, self-hosted | >1M events/day, cloud |

## SQL Dialect Differences

These are the differences our query abstraction layer must handle:

| Operation | DuckDB | ClickHouse |
|-----------|--------|-----------|
| Date truncate | `date_trunc('day', ts)` | `toStartOfDay(ts)` |
| Date format | `strftime(ts, '%Y-%m-%d')` | `formatDateTime(ts, '%Y-%m-%d')` |
| Approx distinct | `approx_count_distinct(x)` | `uniq(x)` |
| Exact distinct | `count(DISTINCT x)` | `count(DISTINCT x)` (same) |
| String concat | `string_agg(x, ',')` | `groupArray(x)` |
| Current time | `CURRENT_TIMESTAMP` | `now()` |
| Interval | `INTERVAL 7 DAY` | `INTERVAL 7 DAY` (same) |
| Null coalesce | `COALESCE(x, 'default')` | `coalesce(x, 'default')` (same) |
| Integer types | `BIGINT` | `UInt64` or `Int64` |
| Boolean | `BOOLEAN` | `UInt8` (0/1) |
| JSON access | `json_extract(x, '$.key')` | `JSONExtractString(x, 'key')` |

## Abstraction Layer Design

```rust
pub trait QueryBuilder: Send + Sync {
    /// Format a date truncation expression
    fn date_trunc(&self, unit: &str, column: &str) -> String;

    /// Format a date for display
    fn date_format(&self, column: &str, format: &str) -> String;

    /// Approximate distinct count
    fn approx_distinct(&self, column: &str) -> String;

    /// Current timestamp
    fn now(&self) -> String;

    /// Build a complete analytics query
    fn build_stats_query(&self, query: &StatsQuery) -> String;
    fn build_timeseries_query(&self, query: &TimeseriesQuery) -> String;
    fn build_breakdown_query(&self, query: &BreakdownQuery) -> String;
    fn build_realtime_query(&self, website_id: &str) -> String;
}

pub struct DuckDBQueryBuilder;
pub struct ClickHouseQueryBuilder;

impl QueryBuilder for DuckDBQueryBuilder {
    fn date_trunc(&self, unit: &str, column: &str) -> String {
        format!("date_trunc('{unit}', {column})")
    }
    // ...
}

impl QueryBuilder for ClickHouseQueryBuilder {
    fn date_trunc(&self, unit: &str, column: &str) -> String {
        match unit {
            "day" => format!("toStartOfDay({column})"),
            "hour" => format!("toStartOfHour({column})"),
            "month" => format!("toStartOfMonth({column})"),
            _ => panic!("Unsupported unit: {unit}"),
        }
    }
    // ...
}
```

## Recommendation

**Start with DuckDB** (weeks 1-6). It's simpler, requires no infrastructure, and covers 99% of self-hosted use cases. The DuckDB backend is our primary differentiator over Umami.

**Cloud V1 also uses DuckDB** (one file per tenant). This is the lowest-risk path — DuckDB handles up to ~1M events/day per tenant, which covers every customer we'll have in Year 1.

**Add ClickHouse** only when needed — when a cloud tenant exceeds 5M events/month, or when total cloud storage exceeds 50GB. Use the same test suite for both backends to ensure behavioral parity.

**Migration path (DuckDB → ClickHouse):**
```bash
# Export a tenant from DuckDB
COPY (SELECT * FROM events WHERE website_id IN (...)) TO '/tmp/tenant.parquet';

# Load into ClickHouse
clickhouse-client --query="INSERT INTO events FORMAT Parquet" < /tmp/tenant.parquet
```
This should be scripted and tested before being needed in production.

**Design the abstraction layer early** (week 1). Even before adding ClickHouse, write the `AnalyticsBackend` trait and implement it for DuckDB. This forces clean separation and makes adding ClickHouse straightforward.

**Don't try to unify SQL.** The dialects are different enough that a generic SQL builder (like SQLx) won't work cleanly. Instead, use separate query builder implementations with a shared interface. It's more code but less surprising behavior.
