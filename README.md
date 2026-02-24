# Sparklytics

**Open-source, self-hosted web analytics. Ships as a single Rust binary — no Node.js, no PostgreSQL, no Redis.**

[![CI](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml/badge.svg)](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

Track pageviews, sessions, custom events, funnels, and retention — with full data ownership and no cookies.

![Sparklytics Dashboard](docs/images/dashboard-screenshot.png)

---

## Deploy on your VPS in 5 minutes

### 1. Start Sparklytics

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.yml
docker compose up -d
```

Open `http://your-server-ip:3000` — you'll be guided through a one-time setup to create your admin account.

### 2. Add your first website

In the dashboard: **New website** → enter name + domain → click **Create**.

### 3. Connect your site

Pick the integration that fits your stack:

#### Any website — HTML snippet

```html
<!-- Add before </body> on every page -->
<script defer src="https://analytics.example.com/s.js" data-website-id="YOUR_WEBSITE_ID"></script>
```

Pageviews appear in the dashboard within seconds. To track custom events:

```js
window.sparklytics?.track('signup_click', { plan: 'pro' })
```

#### Next.js app — first-class SDK

```bash
npm install @sparklytics/next
```

```tsx
// app/layout.tsx
import { SparklyticsProvider } from '@sparklytics/next'

export default function RootLayout({ children }) {
  return (
    <html><body>
      <SparklyticsProvider
        host="https://analytics.example.com"
        websiteId="YOUR_WEBSITE_ID"
      >
        {children}
      </SparklyticsProvider>
    </body></html>
  )
}
```

Pageviews track automatically on every route change — App Router and Pages Router both supported. For custom events:

```tsx
'use client'
import { useSparklytics } from '@sparklytics/next'

export function SignupButton() {
  const { track } = useSparklytics()
  return (
    <button onClick={() => track('signup_click', { plan: 'pro' })}>
      Get started
    </button>
  )
}
```

That's it. Both integrations are < 5 KB gzipped and work without cookies.

---

## Enable HTTPS (recommended)

Use [Caddy](https://caddyserver.com) for automatic TLS — no certbot, no manual renewal:

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.caddy.yml
# Edit Caddyfile — replace analytics.example.com with your domain
docker compose -f docker-compose.caddy.yml up -d
```

Your analytics dashboard will be live at `https://analytics.yourdomain.com`.

> **Nginx / Traefik:** See [docs/reverse-proxy.md](docs/reverse-proxy.md) for alternative configs.

---

## Feature comparison

| Feature | Sparklytics | Umami | Plausible |
|---------|:-----------:|:-----:|:---------:|
| Open source | ✅ MIT | ✅ MIT | ✅ AGPL |
| Self-hostable | ✅ | ✅ | ✅ |
| Single binary | ✅ | ❌ Node + DB | ❌ Elixir + DB |
| No cookies | ✅ | ✅ | ✅ |
| Custom events | ✅ | ✅ | ✅ |
| Funnels | ✅ | ❌ | ❌ |
| Journey analysis | ✅ | ❌ | ❌ |
| Retention cohorts | ✅ | ❌ | ❌ |
| Session drilldown | ✅ | ✅ | ❌ |
| Goals & conversion | ✅ | ✅ | ✅ |
| Real-time dashboard | ✅ | ✅ | ✅ |
| Built-in A/B testing | 🗓 V1.1 | ❌ | ❌ |
| GeoIP (bundled) | ✅ | ❌ | ✅ |
| Multi-site | ✅ | ✅ | ✅ |
| Next.js SDK | ✅ | ❌ | ❌ |
| Docker arm64 | ✅ | ✅ | ✅ |

> Umami ~6,400 GitHub stars. Plausible is source-available (AGPL, not MIT).

---

## Benchmarks

| Metric | Value |
|--------|-------|
| Ingestion p99 latency | < 50ms on Linux at 900+ req/s |
| Memory (idle) | ~41 MB |
| Storage per million events | ~25 MB |
| Binary size (linux-amd64 musl) | ~15 MB |
| Dashboard bundle (gzipped) | ~97 KB |
| `@sparklytics/next` SDK (gzipped) | < 5 KB |

---

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPARKLYTICS_AUTH` | `local` | Auth mode: `none` · `password` · `local` |
| `SPARKLYTICS_PASSWORD` | — | Required when `AUTH=password` |
| `SPARKLYTICS_HTTPS` | `true` | Set `false` only for plain-HTTP local dev |
| `SPARKLYTICS_PORT` | `3000` | Listen port |
| `SPARKLYTICS_DATA_DIR` | `./data` | DuckDB data directory |
| `SPARKLYTICS_DUCKDB_MEMORY` | `1GB` | Query memory limit (raise to `2GB`–`8GB` on larger VPS) |
| `SPARKLYTICS_CORS_ORIGINS` | — | Comma-separated allowed origins for analytics API |
| `SPARKLYTICS_RETENTION_DAYS` | `365` | How long to keep raw events |
| `SPARKLYTICS_GEOIP_PATH` | auto | Docker images bundle DB-IP automatically |

### Auth modes

| Mode | `SPARKLYTICS_AUTH` | First-run UX |
|------|-------------------|--------------|
| Local (recommended) | `local` | One-time `/setup` page → login |
| Single password | `password` | Login with `SPARKLYTICS_PASSWORD` |
| Open (no auth) | `none` | Dashboard opens directly |

### GeoIP

Docker images bundle the [DB-IP City Lite](https://db-ip.com) database — **no setup needed**.

For bare-metal installs:

```bash
./scripts/download-geoip.sh
export SPARKLYTICS_GEOIP_PATH=./dbip-city-lite.mmdb
```

> You can also use MaxMind GeoLite2-City.mmdb — just point `SPARKLYTICS_GEOIP_PATH` at it.

---

## Pre-built binaries

Download from [Releases](https://github.com/Sparklytics/sparklytics/releases):

```
sparklytics-linux-amd64
sparklytics-linux-arm64
sparklytics-darwin-arm64
```

Run directly — the dashboard is embedded:

```bash
SPARKLYTICS_DATA_DIR=./data ./sparklytics
```

---

## Tech stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust · Axum 0.8 · Tokio |
| Storage | DuckDB (embedded, no separate DB process) |
| Dashboard | Next.js 16 · TailwindCSS · shadcn/ui |
| SDK | `@sparklytics/next` (npm) |
| Auth | Argon2id · JWT HttpOnly cookies |

---

## Development

```bash
# Backend
cargo build
cargo test
cargo run

# Dashboard (dev server at :3001, proxies /api → :3000)
cd dashboard && npm run dev

# SDK
cd sdk/next && npm run dev
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for full setup instructions.

---

## Documentation

- [First Launch Runbook](FIRST-LAUNCH-RUNBOOK.md) — detailed first-run verification with curl commands
- [Reverse Proxy Setup](docs/reverse-proxy.md) — Nginx / Traefik configs
- [API Specification](docs/07-API-SPECIFICATION.md)
- [Database Schema](docs/08-DATABASE-SCHEMA.md)
- [Self-Hosted Auth](docs/13-SELF-HOSTED-AUTH.md)
- [SDK (`@sparklytics/next`)](https://github.com/Sparklytics/sparklytics-next)

---

## License

MIT — see [LICENSE](LICENSE).
