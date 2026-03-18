---
name: load-test
description: Run a load test against the Sparklytics collect endpoint (or any endpoint) and report throughput, latency percentiles, and error rate. Compares against a saved baseline if one exists. Usage: /load-test [url] [--duration 30s] [--rate 500] [--connections 50]
---

The user wants to run a load test. Arguments: $ARGUMENTS

## Step 1: Parse arguments

Defaults if not specified:
- `url`: `http://localhost:3001/api/collect`
- `--duration`: `30s`
- `--rate`: `500` req/sec (use as target RPS for `oha`/`hey`; `wrk` uses connection count instead)
- `--connections`: `50` concurrent connections
- `--tool`: auto-detect (`oha` preferred, then `hey`, then `wrk`, then `curl` loop as fallback)

## Step 2: Check tool availability

Run in order, use first available:
```bash
which oha    # preferred: Rust-based, histogram output, rate limiting
which hey    # good: Go-based, rate limiting with -q flag
which wrk    # ok: C-based, no rate limiting but very high throughput
```

If none available, tell the user:
```
Install oha (recommended): cargo install oha
Or: brew install oha
Or: brew install hey
```

## Step 3: Build the test payload

For `/api/collect`, use a realistic event payload:
```json
{
  "website_id": "00000000-0000-0000-0000-000000000001",
  "event_type": "pageview",
  "url": "https://example.com/pricing",
  "referrer": "https://google.com",
  "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
  "screen_width": 1440,
  "screen_height": 900,
  "language": "en-US"
}
```

For other endpoints, ask the user for the payload or derive it from the sprint spec.

## Step 4: Run the test

**With oha (preferred):**
```bash
oha -n 999999 -c {connections} -q {rate} -z {duration} \
    -m POST \
    -H "Content-Type: application/json" \
    -d '{payload}' \
    --no-tui \
    {url}
```

**With hey:**
```bash
hey -z {duration} -c {connections} -q {rate} \
    -m POST \
    -H "Content-Type: application/json" \
    -d '{payload}' \
    {url}
```

**With wrk:**
```bash
# wrk doesn't support rate limiting — just max throughput
wrk -t 4 -c {connections} -d {duration} \
    -s /tmp/sparklytics_post.lua \
    {url}
```
(Generate a wrk Lua script that POSTs the JSON payload.)

## Step 5: Parse and display results

Extract from tool output:
- **Total requests** sent
- **Requests/sec** (throughput)
- **Success rate** (% of 2xx responses)
- **Latency**: p50, p90, p95, p99, max
- **Error breakdown**: connection refused, 4xx, 5xx, timeouts

Format as:

```
## Load Test Results
URL:         http://localhost:3001/api/collect
Tool:        oha
Duration:    30s
Connections: 50
Target RPS:  500

### Throughput
Requests/sec:   487.3  (97.5% of target)
Total requests: 14,619
Success rate:   99.97% (3 errors)

### Latency
p50:   2.1ms
p90:   4.8ms
p95:   7.2ms
p99:   18.4ms
max:   143ms

### Errors
- 3x HTTP 500 (likely buffer overflow under load)

### vs Baseline
[if baseline exists: show diff table]
[if no baseline: "Run /load-test --save-baseline to record this as baseline"]
```

## Step 6: Baseline comparison

Check if `docs/perf-baseline.md` exists. If it does, show a comparison table:

```
| Metric      | Baseline | Current  | Delta     |
|-------------|----------|----------|-----------|
| req/sec     | 320.1    | 487.3    | +52% ✓    |
| p99 latency | 45ms     | 18.4ms   | -59% ✓    |
| error rate  | 0.02%    | 0.02%    | 0% →      |
```

If `--save-baseline` flag passed, write current results to `docs/perf-baseline.md`.

## Step 7: Interpretation

Based on results, give a brief verdict:

**Healthy signs:**
- p99 < 20ms at target RPS → buffer and batch write working correctly
- Error rate < 0.1% → no buffer overflow or DB backpressure
- Throughput within 10% of target → no OS connection limits hit

**Warning signs to flag:**
- p99 > 100ms → likely DuckDB write contention (mutex held too long, batch too large)
- Error rate > 1% → buffer overflow or rate limiter rejecting (check 429 vs 500 breakdown)
- Throughput < 50% of target → connection pool exhausted or blocking I/O in handler
- Max latency spike (e.g. 2s) → flush is happening synchronously in the request path

**After a bad result, suggest running:**
- `/agent:perf-reviewer` on the collect handler and buffer implementation
- Check DuckDB flush timing: `SET threads=2` and `SET memory_limit='128MB'` in place?
