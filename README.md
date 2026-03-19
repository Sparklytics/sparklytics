# Sparklytics

**Open-source, self-hosted web analytics. Ships as a single Rust binary — no Node.js, no PostgreSQL, no Redis.**

[![CI](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml/badge.svg)](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-js?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-js&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-next?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-next&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-vue?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-vue&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)

Track pageviews, sessions, custom events, funnels, and retention — with full data ownership and no cookies.

![Sparklytics Dashboard](docs/images/dashboard-screenshot.png)

---

## Deploy on your VPS in 5 minutes

Docker is the recommended first-time install path. Use plain HTTP only for local testing; for a public VPS, put Sparklytics behind HTTPS with Caddy, Nginx, or Traefik.

### 1. Start Sparklytics

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.yml
# Edit docker-compose.yml:
# - set SPARKLYTICS_BOOTSTRAP_PASSWORD for first boot
# - set SPARKLYTICS_PUBLIC_URL to your final public origin
docker compose up -d
```

For a VPS deploy that should only pull a ready-made image instead of compiling Rust + Next.js on the server:

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.image.yml
# Edit docker-compose.image.yml inputs via env:
# - SPARKLYTICS_IMAGE_TAG=latest           (or a sha-* image tag from GHCR)
# - SPARKLYTICS_BOOTSTRAP_PASSWORD
# - SPARKLYTICS_PUBLIC_URL
docker compose -f docker-compose.image.yml pull
docker compose -f docker-compose.image.yml up -d
```

Prebuilt images are published to `ghcr.io/sparklytics/sparklytics` on every push to `main` and can also be published manually from a branch via the `Docker Publish` GitHub Actions workflow.

Open `http://your-server-ip:3000` and Sparklytics will guide you through:

1. creating your admin password,
2. landing in onboarding and adding your first website,
3. installing the tracking snippet, and
4. verifying that your first pageview was received.

Fresh installs start with zero websites. There is no seeded default site; the first website is created in onboarding.

In `SPARKLYTICS_AUTH=local`, first boot is protected by a bootstrap password:

- preferred: set `SPARKLYTICS_BOOTSTRAP_PASSWORD` explicitly during install
- fallback: if you leave it unset, Sparklytics uses `sparklytics`
- safety: when the fallback is used, Sparklytics forces an immediate admin password rotation before the dashboard becomes usable

If you use plain HTTP locally, set `SPARKLYTICS_HTTPS=false` in `docker-compose.yml` (or env). Keep `SPARKLYTICS_HTTPS=true` only behind HTTPS/TLS.

### 2. Connect your site

Pick the integration that fits your stack:

#### Any website — HTML snippet

```html
<!-- Add inside <head> on every page -->
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

### 3. Production minimum

Before exposing Sparklytics on the public internet, make sure you have:

- HTTPS enabled via reverse proxy
- `SPARKLYTICS_HTTPS=true` in the Sparklytics container behind TLS
- `SPARKLYTICS_PUBLIC_URL=https://analytics.example.com` set to the real public origin
- a persistent Docker volume mounted at `/data`
- an explicit DuckDB memory cap via `SPARKLYTICS_DUCKDB_MEMORY`
- `SPARKLYTICS_BOOTSTRAP_PASSWORD` set to a non-default value for production installs
- `SPARKLYTICS_TRUSTED_PROXIES` set when running behind Caddy, Nginx, or Traefik
- a strong password in `local` or `password` mode
- an explicit `SPARKLYTICS_CORS_ORIGINS` allowlist when browser-side analytics API access is needed

Avoid `SPARKLYTICS_AUTH=none` outside trusted local or private-network development.

---

## Enable HTTPS (recommended for production)

Use [Caddy](https://caddyserver.com) for automatic TLS — no certbot, no manual renewal:

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.caddy.yml
# Edit Caddyfile — replace analytics.example.com with your domain
# Edit docker-compose.caddy.yml:
# - set SPARKLYTICS_BOOTSTRAP_PASSWORD
# - set SPARKLYTICS_PUBLIC_URL to your domain
# - set SPARKLYTICS_TRUSTED_PROXIES for your proxy/network CIDR
docker compose -f docker-compose.caddy.yml up -d
```

Your analytics dashboard will be live at `https://analytics.yourdomain.com`.

> **Nginx / Traefik:** See [docs/reverse-proxy.md](docs/reverse-proxy.md) for alternative configs.
>
> **Local testing:** Keep the simple `docker-compose.yml` path and set `SPARKLYTICS_HTTPS=false` if you are using plain HTTP on localhost.

---

## Feature comparison

| Feature | Sparklytics | Umami | Plausible |
|---------|:-----------:|:-----:|:---------:|
| Open source | ✅ MIT | ✅ MIT | ✅ Open source |
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

---

## Benchmarks

Measured on Apple Silicon macOS, release builds, 100k–1M realistic events.
Full methodology and raw data: [`docs/perf-baseline.md`](docs/perf-baseline.md).

### Self-Hosted (DuckDB)

| Metric | Value |
|--------|-------|
| Peak ingest throughput | ~26,000 req/s (single event) |
| Batch ingestion | ~74,800 events/s (batch of 10) |
| Ingestion p99 latency (800 req/s) | 1.14 ms |
| Memory (idle) | **~29 MB** |
| Memory (under load) | ~64 MB |
| Storage per 1M events | ~278 MB |
| Binary size (linux-amd64 musl) | ~15 MB |
| Dashboard bundle (gzipped) | ~632 KB |
| `@sparklytics/next` SDK (gzipped) | < 5 KB |

### Cloud (ClickHouse)

| Metric | Value |
|--------|-------|
| Peak ingest throughput | ~18,000–24,000 req/s (single event) |
| Batch ingestion | ~86,660 events/s (batch of 10) |
| Query throughput (analytics) | 1,600–4,300 req/s (all endpoints) |
| Storage per 1M events | ~48 MB (5.8x more efficient) |
| ClickHouse vs DuckDB speedup | **10–68x** at 100k, **47–239x** at 1M |

### Scaling: 100k → 1M Events

| Dimension | DuckDB | ClickHouse |
|-----------|--------|-----------|
| Query degradation | 3.5–5x slower per 10x data | Near-constant (< 1.2x) |
| Ingest degradation | Drops 59% (26k→11k) | Unchanged |
| Memory (query peak) | 407 MB → 3.5 GB | 200 MB → 325 MB |
| Storage efficiency | 278 MB/1M | 48 MB/1M |

---

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPARKLYTICS_AUTH` | `local` | Auth mode: `none` · `password` · `local` |
| `SPARKLYTICS_PASSWORD` | — | Required when `SPARKLYTICS_AUTH=password` |
| `SPARKLYTICS_HTTPS` | `true` | Set `false` only for plain-HTTP local dev |
| `SPARKLYTICS_PORT` | `3000` | Listen port |
| `SPARKLYTICS_DATA_DIR` | `./data` | DuckDB data directory |
| `SPARKLYTICS_DUCKDB_MEMORY` | `1GB` | Query memory limit (raise to `2GB`–`8GB` on larger VPS) |
| `SPARKLYTICS_CORS_ORIGINS` | — | Comma-separated allowed origins for analytics API |
| `SPARKLYTICS_RETENTION_DAYS` | `365` | How long to keep raw events |
| `SPARKLYTICS_GEOIP_PATH` | `./GeoLite2-City.mmdb` | Path to city MMDB. Canonical default is `./GeoLite2-City.mmdb`; the bare-metal download script writes `./dbip-city-lite.mmdb`, so set this env var accordingly when using that script. |

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

`SPARKLYTICS_GEOIP_PATH` defaults to `./GeoLite2-City.mmdb` (generic packaged default), but the download script outputs `dbip-city-lite.mmdb`, so keep the env var aligned with the file you actually install.

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
| Dashboard | Next.js 15 · TailwindCSS · shadcn/ui |
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

# SDK (separate nested repo checkout)
# if sdk/next is not present locally:
#   git clone git@github.com:Sparklytics/sparklytics-next.git sdk/next
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
