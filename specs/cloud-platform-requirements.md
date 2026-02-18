# Cloud Platform Requirements Specification

**Component:** Sparklytics Cloud (SaaS)
**Target Launch:** Week 13
**Infrastructure:** Fly.io (API) + Hetzner (ClickHouse) + Stripe (billing)

## Overview

The cloud platform adds multi-tenancy, authentication, billing, and managed ClickHouse to the core Sparklytics server. It runs the same Rust binary with cloud-specific environment variables enabled.

## Multi-Tenancy Model

```
User Account
  ├── Profile (email, name, password)
  ├── Subscription (Stripe, plan, limits)
  ├── API Keys (0-N)
  └── Websites (0-N)
       └── Events (isolated by website_id)
```

**Isolation:** All analytics queries include `WHERE website_id IN (SELECT id FROM websites WHERE user_id = ?)`. This is enforced at the query builder level, not the route handler level, to prevent accidental data leaks.

**Cloud detection:** `SPARKLYTICS_MODE=cloud` environment variable enables auth, billing, and multi-tenancy features. Without it, the server runs in single-user self-hosted mode.

## Authentication

### Registration Flow

```
1. User submits email + password + name
2. Server validates (email format, password min 8 chars)
3. Server creates user (password hashed with bcrypt, cost 12)
4. Server sends verification email
5. User clicks verification link
6. Account activated

Until verified:
- Can create websites and view dashboard
- Cannot access API keys
- Reminder banner shown in dashboard
```

### Login Flow

```
1. User submits email + password
2. Server verifies against bcrypt hash
3. If valid: create JWT, set HttpOnly cookie
4. If invalid: return 401 (don't reveal if email exists)
5. Rate limit: 5 attempts per minute per IP
```

### JWT Token

```json
{
  "sub": "usr_abc123",
  "email": "user@example.com",
  "iat": 1708000000,
  "exp": 1708604800
}
```

- Algorithm: RS256
- Expiry: 7 days
- Cookie: `sparklytics_token`, HttpOnly, Secure, SameSite=Strict
- Refresh: on each authenticated request, if >1 day old

### Password Reset

```
1. User requests reset (email input)
2. Server generates reset token (32 random bytes, hashed before storage)
3. Token expires in 1 hour
4. Email sent with reset link
5. User submits new password + token
6. Server validates token, updates password, invalidates token
```

## Subscription Management

### Plans

| Plan | Event Limit | Websites | Retention | API Access | Price |
|------|-------------|----------|-----------|------------|-------|
| Free | 10K/month | 1 | 6 months | No | EUR 0 |
| Pro | 100K/month | Unlimited | 12 months | Yes | EUR 12/mo |
| Business | 1M/month | Unlimited | 24 months | Yes | EUR 39/mo |

### Stripe Integration

**Checkout:**
```
1. User clicks "Upgrade to Pro"
2. Server creates Stripe Checkout Session
3. User redirected to Stripe-hosted payment page
4. Stripe sends checkout.session.completed webhook
5. Server activates subscription
```

**Webhook Events:**

| Event | Action |
|-------|--------|
| checkout.session.completed | Activate subscription |
| customer.subscription.updated | Update plan/status |
| customer.subscription.deleted | Downgrade to free |
| invoice.payment_failed | Mark subscription as past_due |
| invoice.paid | Clear past_due status |

**Customer Portal:**
- Stripe-hosted portal for subscription management
- Users can update payment method, download invoices, cancel
- Accessible via "Manage Subscription" in account settings

### Usage Tracking

```sql
-- Increment on each event batch flush
UPDATE usage_records
SET event_count = event_count + ?,
    updated_at = CURRENT_TIMESTAMP
WHERE user_id = ? AND period = ?;
```

**Enforcement:**
- At 80% of limit: warning banner in dashboard
- At 100% of limit: email notification, events still accepted
- At 120% of limit: events rejected with 429 (Too Many Requests) response with `X-Sparklytics-Quota-Exceeded: true` header
- Reset: monthly, on subscription period start

### Upgrade/Downgrade

**Upgrade:** Immediate. Stripe prorates the charge. New limits apply instantly.
**Downgrade:** End of billing period. Current limits maintained until period ends.
**Cancel:** Downgrade to Free at end of billing period. Data retained for 30 days, then deleted.

## Email System

**Provider:** Resend (or Postmark)

**Emails sent:**
1. Verification email (on registration)
2. Password reset email
3. Usage warning (at 80% limit)
4. Usage exceeded (at 100% limit)
5. Subscription confirmation
6. Payment failed notification

**Templates:** Simple HTML emails, no heavy branding. Plain text fallback.

## API Key Management

**Key format:** `spk_live_` + 32 hex chars (e.g., `spk_live_a1b2c3d4e5f6...`)

**Storage:** Only the argon2 hash stored in database. Key prefix (`spk_live_a1b2`) stored for display/identification.

**Permissions:** API keys have same access as the user who created them. Scoped to all websites owned by that user.

**Limits:** Max 5 API keys per user.

**Lifecycle:**
1. User creates key: full key shown once, never again
2. User uses key: `Authorization: Bearer spk_live_...`
3. Server: hash incoming key, compare against stored hashes
4. User revokes key: delete from database, immediately invalid

## Data Isolation

**Critical security requirement.** Every query must be scoped to the authenticated user.

```rust
// This is enforced at the trait level, not the route level
impl CloudAnalyticsBackend {
    async fn query_stats(&self, user_id: &str, website_id: &str, query: &StatsQuery) -> Result<Stats> {
        // First: verify website belongs to user
        let website = self.get_website(website_id).await?;
        if website.user_id != user_id {
            return Err(SparkError::NotFound { resource: "website".into() });
        }

        // Then: query with website_id filter (always present)
        self.clickhouse.query_stats(website_id, query).await
    }
}
```

**Testing:** Dedicated integration tests that attempt cross-tenant access (must fail).

## Infrastructure (Cloud Mode)

### Fly.io Configuration

```toml
# fly.toml
app = "sparklytics"
primary_region = "ams"

[build]
  dockerfile = "Dockerfile"

[env]
  SPARKLYTICS_MODE = "cloud"
  SPARKLYTICS_BACKEND = "clickhouse"
  SPARKLYTICS_PORT = "8080"

[http_service]
  internal_port = 8080
  force_https = true
  auto_start_machines = true
  auto_stop_machines = true
  min_machines_running = 1

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 256
```

### Environment Variables (Cloud Secrets)

```bash
fly secrets set \
  SPARKLYTICS_JWT_SECRET="..." \
  SPARKLYTICS_CLICKHOUSE_URL="http://clickhouse.example:8123" \
  SPARKLYTICS_DATABASE_URL="postgres://..." \
  STRIPE_SECRET_KEY="sk_live_..." \
  STRIPE_WEBHOOK_SECRET="whsec_..." \
  RESEND_API_KEY="re_..."
```

### ClickHouse (Hetzner) — Phase 2 Only

**Decision: Cloud V1 launches on DuckDB-per-tenant, NOT ClickHouse.**

Each cloud tenant gets their own DuckDB file (`/data/{tenant_id}.db`). This eliminates ClickHouse as a SPOF at launch, keeps infrastructure cost near zero, and allows shipping cloud sooner. ClickHouse is introduced only when a tenant exceeds **5M events/month** or total cloud storage exceeds **50GB**.

This changes the launch architecture to: Fly.io (API) + Fly.io Volumes (DuckDB files) + Stripe (billing). No Hetzner server needed at launch.

**ClickHouse migration path (when needed):**
- Export tenant data from DuckDB to Parquet: `COPY (SELECT * FROM events) TO '/tmp/export.parquet'`
- Load into ClickHouse via `clickhouse-client --query="INSERT INTO events FORMAT Parquet" < export.parquet`
- Script this as `sparklytics migrate-tenant --id <tenant_id> --to clickhouse`

**When ClickHouse is eventually needed:**

```bash
# Hetzner CPX51: 8 vCPU, 16GB RAM, 240GB SSD (NOT CPX31 — ClickHouse minimum is 16GB RAM)
# EUR 42.00/month

# Docker Compose on Hetzner
services:
  clickhouse:
    image: clickhouse/clickhouse-server:24.3
    ports:
      - "8123:8123"   # HTTP
      - "9000:9000"   # Native
    volumes:
      - clickhouse-data:/var/lib/clickhouse
    environment:
      - CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1
    ulimits:
      nofile:
        soft: 262144
        hard: 262144
```

### PostgreSQL (Fly.io)

Used for users, subscriptions, API keys (not analytics data).

```bash
fly postgres create --name sparklytics-db --region ams
fly postgres attach sparklytics-db
```

## Monitoring

**Health checks:**
- `/health` - checks ClickHouse and PostgreSQL connectivity
- Fly.io auto-restarts on consecutive failures

**Metrics (Prometheus):**
- Active users
- Events ingested per minute
- Query latency
- Error rate
- Subscription counts by plan

**Alerts:**
- Error rate > 1% for 5 minutes -> PagerDuty/Telegram
- ClickHouse disk > 80% -> Email
- Payment webhook failures -> Email

## Security Checklist (Cloud)

- [ ] All traffic over TLS 1.3
- [ ] JWT secrets rotated quarterly
- [ ] API keys hashed with argon2
- [ ] SQL injection impossible (parameterized queries)
- [ ] Rate limiting on all endpoints
- [ ] CORS restricted to sparklytics.dev
- [ ] Stripe webhook signature verification
- [ ] Cross-tenant access tests in CI
- [ ] Container image scanning (Trivy)
- [ ] Dependency audit (cargo audit)
- [ ] No PII in logs
- [ ] GDPR data export endpoint
- [ ] GDPR data deletion endpoint
