# Sparklytics

**Open-source, self-hosted web analytics. Ships as a single Rust binary — no Node.js, no PostgreSQL, no Redis.**

[![CI](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml/badge.svg)](https://github.com/Sparklytics/sparklytics/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-js?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-js&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-next?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-next&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)
![CodeRabbit Pull Request Reviews](https://img.shields.io/coderabbit/prs/github/Sparklytics/sparklytics-vue?utm_source=oss&utm_medium=github&utm_campaign=Sparklytics%2Fsparklytics-vue&labelColor=171717&color=FF570A&link=https%3A%2F%2Fcoderabbit.ai&label=CodeRabbit+Reviews)

Track pageviews, sessions, custom events, funnels, and retention — with full data ownership and no cookies.

![Sparklytics Dashboard](https://raw.githubusercontent.com/Sparklytics/sparklytics-docs/main/images/dashboard-screenshot.png)

---

## Deploy on your VPS in 5 minutes

Docker is the recommended first-time install path. Use plain HTTP only for local testing; for a public VPS, put Sparklytics behind HTTPS with Caddy, Nginx, or Traefik.

### 1. Start Sparklytics

```bash
git clone https://github.com/Sparklytics/sparklytics.git
cd sparklytics
# docker-compose.yml is localhost-ready:
# - SPARKLYTICS_AUTH=local
# - SPARKLYTICS_HTTPS=false
# - SPARKLYTICS_PUBLIC_URL=http://localhost:3000
# Before production use, set SPARKLYTICS_BOOTSTRAP_PASSWORD and switch to
# docker-compose.caddy.yml or docker-compose.image.yml with your real HTTPS origin.
docker compose up --build -d
```

The source-build compose files pass `CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-1}`
to the Docker build to keep small VPS builds stable. Set `CARGO_BUILD_JOBS=4`
before `docker compose up --build -d` on larger builders if you want faster
source builds.

For a VPS deploy that should only pull a ready-made image instead of compiling Rust + Next.js on the server:

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.image.yml
# Edit docker-compose.image.yml inputs via env:
# - SPARKLYTICS_IMAGE_TAG=latest           (or a sha-* image tag from GHCR)
# - SPARKLYTICS_BOOTSTRAP_PASSWORD
# - SPARKLYTICS_PUBLIC_URL
# - SPARKLYTICS_TRACKING_PUBLIC_BASE       (optional, e.g. https://example.com/_sl)
docker compose -f docker-compose.image.yml pull
docker compose -f docker-compose.image.yml up -d
```

The optional prebuilt-image flow uses `ghcr.io/sparklytics/sparklytics`. The default source-build flow above does not require registry access; use it if `docker compose pull` returns `unauthorized` for the GHCR package.

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

#### Public websites behind Cloudflare / blockers — first-party proxy (recommended)

```html
<!-- Add inside <head> on every page -->
<script defer src="/_sl/s.js" data-website-id="YOUR_WEBSITE_ID"></script>
```

Sparklytics serves `/_sl/s.js` and `/_sl/e` natively on its own origin. If you
want those paths on your main app/domain, route them through your reverse proxy:

- `/_sl/s.js` → Sparklytics `/s.js`
- `/_sl/e` → Sparklytics `/e`

If you want onboarding and the API to emit this form by default, set:

```bash
SPARKLYTICS_TRACKING_PUBLIC_BASE=https://example.com/_sl
```

`data-api-host` remains available as a legacy override, but standard self-hosted installs should not need it.

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
- `SPARKLYTICS_TRACKING_PUBLIC_BASE=https://example.com/_sl` when using a first-party proxy path
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
git clone https://github.com/Sparklytics/sparklytics.git
cd sparklytics
# Edit Caddyfile — replace analytics.example.com with your domain
# Edit docker-compose.caddy.yml:
# - set SPARKLYTICS_BOOTSTRAP_PASSWORD
# - set SPARKLYTICS_PUBLIC_URL to your domain
# - optionally set SPARKLYTICS_TRACKING_PUBLIC_BASE=https://your-site.com/_sl
# - set SPARKLYTICS_TRUSTED_PROXIES for your proxy/network CIDR
docker compose -f docker-compose.caddy.yml up --build -d
```

Your analytics dashboard will be live at `https://analytics.yourdomain.com`.

> **Nginx / Traefik:** See [Reverse Proxy Setup](https://github.com/Sparklytics/sparklytics-docs/blob/main/reverse-proxy.md) for alternative configs.
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
| GeoIP (optional MMDB) | ✅ | ❌ | ✅ |
| Multi-site | ✅ | ✅ | ✅ |
| Next.js SDK | ✅ | ❌ | ❌ |
| Docker arm64 | ✅ | ✅ | ✅ |

---

## Benchmarks

Measured on Apple Silicon macOS, release builds, 100k–1M realistic events.
Full methodology and raw data: [`perf-baseline.md`](https://github.com/Sparklytics/sparklytics-docs/blob/main/perf-baseline.md).

### Self-Hosted (DuckDB)

| Metric | Value |
|--------|-------|
| Peak ingest throughput | ~26,000 req/s (single event) |
| Batch ingestion | ~74,800 events/s (batch of 10) |
| Ingestion p99 latency (800 req/s) | 1.14 ms |
| Memory (idle) | **~29 MB** |
| Memory (under load) | ~64 MB |
| Storage per 1M events | ~278 MB |
| Binary size (linux-amd64 release) | ~15 MB |
| Dashboard bundle (gzipped) | ~632 KB |
| `@sparklytics/next` SDK (gzipped) | < 5 KB |

### Self-Hosted Scaling Notes

Sparklytics self-host runs on one embedded DuckDB file. For larger local
datasets, raise `SPARKLYTICS_DUCKDB_MEMORY` and keep `SPARKLYTICS_DATA_DIR` on
fast persistent storage.

| Dimension | Self-hosted DuckDB note |
|-----------|-------------------------|
| Query memory | 100k-event query peak measured around 407 MB; 1M-event peak around 3.5 GB |
| Ingest throughput | Single-event peak remains above 10k req/s at 1M-event scale in release builds |
| Storage efficiency | Around 278 MB per 1M realistic events in the measured fixture |
| Tuning | Use `SPARKLYTICS_DUCKDB_MEMORY=4GB` or higher on larger VPS instances |

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
| `SPARKLYTICS_GEOIP_PATH` | `./GeoLite2-City.mmdb` | Optional path to a DB-IP or MaxMind city MMDB. Missing files are allowed and store `NULL` geo fields. |
| `SPARKLYTICS_TRACKING_PUBLIC_BASE` | `SPARKLYTICS_PUBLIC_URL` | Optional public tracker base. Example: `https://example.com/_sl` emits `https://example.com/_sl/s.js`. |
| `SPARKLYTICS_TRUSTED_PROXIES` | — | Comma/space-separated proxy CIDRs trusted for `X-Forwarded-For` / `X-Real-IP`. Required behind Caddy/Nginx/Traefik for correct source IP, GeoIP, visitor ID, and per-IP rate limits. |

### First-party proxy example

Nginx:

```nginx
location /_sl/s.js {
    proxy_pass http://127.0.0.1:3000/s.js;
    proxy_set_header Host $host;
}

location /_sl/e {
    proxy_pass http://127.0.0.1:3000/e;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Caddy:

```caddyfile
example.com {
    handle /_sl/s.js {
        rewrite * /s.js
        reverse_proxy localhost:3000
    }

    handle /_sl/e {
        rewrite * /e
        reverse_proxy localhost:3000
    }
}
```

If browser extensions or Cloudflare challenge `analytics.example.com`, switch to the first-party path above. `ERR_BLOCKED_BY_CLIENT` usually means an extension blocked the analytics origin or endpoint. Sparklytics accepts `/_sl/s.js` and `/_sl/e` directly when those paths reach the Sparklytics service; use the proxy snippets when the paths live on another app/domain.

### Auth modes

| Mode | `SPARKLYTICS_AUTH` | First-run UX |
|------|-------------------|--------------|
| Local (recommended) | `local` | One-time `/setup` page → login |
| Single password | `password` | Login with `SPARKLYTICS_PASSWORD` |
| Open (no auth) | `none` | Dashboard opens directly |

### GeoIP

GeoIP enrichment is optional. If no MMDB file is configured, Sparklytics starts normally and stores `NULL` country/region/city fields.

To enable GeoIP, download the free [DB-IP City Lite](https://db-ip.com) database:

```bash
./scripts/download-geoip.sh
export SPARKLYTICS_GEOIP_PATH=./dbip-city-lite.mmdb
```

`SPARKLYTICS_GEOIP_PATH` defaults to `./GeoLite2-City.mmdb` in bare-metal runs and `/geoip/dbip-city-lite.mmdb` in Docker images. Mount the MMDB file at that path or set the env var to the file you actually install.

> You can also use MaxMind GeoLite2-City.mmdb — just point `SPARKLYTICS_GEOIP_PATH` at it.

---

## Pre-built binaries

Download from [Releases](https://github.com/Sparklytics/sparklytics/releases):

```
sparklytics-linux-amd64
sparklytics-linux-arm64
sparklytics-darwin-amd64
sparklytics-darwin-arm64
```

Run directly — the dashboard is embedded:

```bash
mkdir -p ./data
SPARKLYTICS_DATA_DIR=./data \
SPARKLYTICS_AUTH=local \
SPARKLYTICS_BOOTSTRAP_PASSWORD=replace-this-bootstrap-password \
SPARKLYTICS_HTTPS=false \
SPARKLYTICS_PUBLIC_URL=http://localhost:3000 \
SPARKLYTICS_DUCKDB_MEMORY=1GB \
./sparklytics
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

# Deterministic self-host API smoke (local/password/none auth modes)
cd dashboard && npm run build && cd ..
cargo build --release
node scripts/selfhost-api-smoke.mjs

# Browser first-launch release smoke
cd dashboard && npm run test:release-smoke
# If backend port 3000 is busy:
# cd dashboard && PLAYWRIGHT_RELEASE_BACKEND_URL=http://127.0.0.1:3333 npm run test:release-smoke

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
- [Reverse Proxy Setup](https://github.com/Sparklytics/sparklytics-docs/blob/main/reverse-proxy.md) — Nginx / Traefik configs
- [API Specification](https://github.com/Sparklytics/sparklytics-docs/blob/main/07-API-SPECIFICATION.md)
- [Database Schema](https://github.com/Sparklytics/sparklytics-docs/blob/main/08-DATABASE-SCHEMA.md)
- [Self-Hosted Auth](https://github.com/Sparklytics/sparklytics-docs/blob/main/13-SELF-HOSTED-AUTH.md)
- [SDK (`@sparklytics/next`)](https://github.com/Sparklytics/sparklytics-next)

---

## License

MIT — see [LICENSE](LICENSE).
