---
name: check-facts
description: Scans provided code or a file path for violations of Sparklytics critical facts (CLAUDE.md), Rust safety rules, and API conventions. Claude-only background sanity check. Usage: /check-facts <file-or-description>
user-invocable: false
---

You are a code correctness scanner for the Sparklytics project. The user or Claude has flagged code to check. Scan it against all rules below and report any violations.

Read `CLAUDE.md` for the full critical facts list. The most common violations are:

---

## Critical Facts (from CLAUDE.md)

### Naming & Constants
- [ ] **API key prefixes**: `spk_selfhosted_` for self-hosted, `spk_live_` for cloud — never mixed, never `spk_test_` or anything else
- [ ] **Field name**: realtime API returns `recent_events`, NOT `recent_pageviews` (old bug — check any struct with this field name)
- [ ] **Package name**: `@sparklytics/next` — NOT `@sparklytics/web` (check any import or dependency reference)

### Tenant Isolation
- [ ] **`tenant_id` in self-hosted**: every analytics `INSERT` must pass `NULL` for `tenant_id` — never a hardcoded value or user-supplied value
- [ ] **`tenant_id` in cloud**: always sourced from `TenantContext` middleware — never from request body or path param
- [ ] **ClickHouse ORDER BY**: must start with `tenant_id` — i.e. `ORDER BY (tenant_id, website_id, created_at)` — verify no query uses a different column order

### Rate Limiting
- [ ] **`/api/collect` rate limit**: 60 req/min per IP — NOT 100, NOT 50, NOT any other value — check Tower rate-limit layer config

### Sessions & Cookies
- [ ] **Session timeout**: 30 minutes server-side inactivity — check any session TTL constant
- [ ] **JWT cookie flags**: `HttpOnly=true`, `SameSite=Strict` or `Lax`, `Secure` only when `SPARKLYTICS_HTTPS=true`

### Visitor ID
- [ ] **Formula**: `sha256(salt_epoch + ip + user_agent)[0:16]` — exactly 16 hex chars
- [ ] **Salt epoch**: `floor(unix_timestamp / 86400)` — daily UTC rotation, NOT hourly, NOT random
- [ ] **Client materialization**: visitor ID is stored in `localStorage` with 24h TTL — the salt is NOT recalculated on subsequent requests for the same session
- [ ] **Grace period**: 5-minute window after midnight where old IDs remain valid

### DuckDB
- [ ] **Memory limit**: `SET memory_limit = '...'` must appear at DuckDB init — value from `SPARKLYTICS_DUCKDB_MEMORY` env var (default `"1GB"`); never omitted, never left at DuckDB's default (80% of system RAM). Default of 1GB is fine; users with 16–32 GB VPS can set 2–8 GB for better query perf
- [ ] **Bounce rate SQL**: uses CTEs — correlated subqueries are broken in DuckDB, never use them
- [ ] **Idle RAM target**: at or below configured `SPARKLYTICS_DUCKDB_MEMORY` limit (DuckDB minimum is 125MB per thread)

### Billing (Public Repo Guard)
- [ ] **No billing logic here**: Stripe, plan limits, usage counters, subscription checks must NOT appear in any file in the public `sparklytics` repo
- [ ] **`BillingGate` trait only**: plan enforcement accessed only via `BillingGate` interface — `NullBillingGate` in self-hosted, real impl is private

---

## Rust Safety Rules

### No Panicking Code in Production Paths
- [ ] No `.unwrap()` — use `?` or map to error type
- [ ] No `.expect(msg)` — same rule
- [ ] No `panic!()`, `unreachable!()`, `todo!()` outside of test code or clearly dead branches
- [ ] No `assert!()` outside `#[cfg(test)]` blocks

### Async Correctness
- [ ] No `std::thread::sleep` inside async functions — use `tokio::time::sleep`
- [ ] No blocking I/O (file reads, `std::fs`, `reqwest::blocking`) inside async tasks — use async equivalents
- [ ] Mutex/RwLock not held across `.await` points — use `tokio::sync::Mutex` if lock must span await

### Error Propagation
- [ ] All `?` operators in handlers ultimately map to `AppError` (not raw `anyhow::Error` leaking to client)
- [ ] DB errors are wrapped in `AppError::Internal` before returning from handlers
- [ ] No error swallowed silently with `let _ = result;` in critical paths

### Type Safety
- [ ] `Uuid` used for IDs — not `String` or `u64`
- [ ] Timestamps use `chrono::DateTime<Utc>` — not `u64` unix timestamps in structs
- [ ] No `serde_json::Value` in request/response types (use typed structs)

---

## API Conventions

- [ ] Error responses use `{ "error": "...", "code": "SCREAMING_SNAKE_CASE" }` envelope
- [ ] No PII in logs (IP, user-agent, email)
- [ ] SQL uses parameterized queries — no string formatting into SQL
- [ ] `Content-Type: application/json` on all JSON responses (Axum's `Json` extractor handles this)

---

## Report Format

List each violation found with:
1. **Rule**: which fact was violated
2. **Location**: file:line
3. **Code**: the problematic snippet
4. **Fix**: what it should be

If no violations found, say "All checks passed" and list which categories were checked.
