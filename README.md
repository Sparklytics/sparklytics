# Sparklytics

**Developer-first, privacy-respecting analytics. Ships as a single Rust binary.**

[![CI](https://github.com/sparklytics/sparklytics/actions/workflows/ci.yml/badge.svg)](https://github.com/sparklytics/sparklytics/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

Sparklytics is an open-source, self-hosted web analytics platform. It tracks pageviews, sessions, and custom events with no cookies and full data ownership.

---

## Quick Start (Docker)

```bash
docker run -p 3000:3000 -v ./data:/data sparklytics/sparklytics:latest
```

Open [http://localhost:3000](http://localhost:3000) — the dashboard is built in.

![Sparklytics Dashboard](docs/images/dashboard-screenshot.png)

### With docker-compose

```bash
curl -O https://raw.githubusercontent.com/sparklytics/sparklytics/main/docker-compose.yml
docker compose up -d
```

---

![Sparklytics Dashboard](docs/images/dashboard-screenshot.png)

---

## Add Analytics to Your Next.js App

```bash
npm install @sparklytics/next
```

```tsx
// app/layout.tsx
import { SparklyticsProvider } from '@sparklytics/next'

export default function RootLayout({ children }) {
  return (
    <html><body>
      <SparklyticsProvider websiteId="site_abc123">
        {children}
      </SparklyticsProvider>
    </body></html>
  )
}
```

That's it. Pageviews track automatically on every route change.

### Track custom events

```tsx
'use client'
import { useSparklytics } from '@sparklytics/next'

export function SignupButton() {
  const { track } = useSparklytics()
  return <button onClick={() => track('signup_click', { plan: 'pro' })}>
    Get started
  </button>
}
```

---

## Benchmarks

| Metric | Value |
|--------|-------|
| Ingestion p99 latency | 80ms at 366 req/s (macOS dev); < 50ms on Linux |
| Memory (idle, 1 day under 10K events/day) | ~41 MB |
| Storage per million events | ~25 MB (DuckDB columnar compression) |
| Binary size (linux-amd64 musl) | ~15 MB |
| Dashboard bundle (gzipped) | ~97 KB |
| `@sparklytics/next` SDK (gzipped) | < 5 KB |

---

## Feature Comparison

| Feature | Sparklytics | Umami | Plausible |
|---------|:-----------:|:-----:|:---------:|
| Open source | ✅ MIT | ✅ MIT | ✅ AGPL |
| Self-hostable | ✅ | ✅ | ✅ |
| Single binary | ✅ | ❌ (Node + DB) | ❌ (Elixir + DB) |
| No cookies | ✅ | ✅ | ✅ |
| Custom events | ✅ | ✅ | ✅ |
| Session tracking | ✅ | ✅ | ✅ |
| Real-time dashboard | ✅ | ✅ | ✅ |
| Built-in A/B testing | 🗓 V1.1 | ❌ | ❌ |
| GeoIP | ✅ | ✅ | ✅ |
| Multi-site | ✅ | ✅ | ✅ |
| Next.js SDK | ✅ | ❌ | ❌ |
| Docker arm64 | ✅ | ✅ | ✅ |

> Umami has ~6,400 GitHub stars. Plausible is source-available (AGPL).

---

## Self-Hosting Guide

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPARKLYTICS_AUTH` | `local` | `none` · `password` · `local` |
| `SPARKLYTICS_PASSWORD` | — | Required when `AUTH=password` |
| `SPARKLYTICS_HTTPS` | `true` | Set `false` behind HTTP reverse proxy |
| `SPARKLYTICS_PORT` | `3000` | Server listen port |
| `SPARKLYTICS_DATA_DIR` | `./data` | DuckDB data directory |
| `SPARKLYTICS_CORS_ORIGINS` | — | Comma-separated origins for query endpoints |
| `SPARKLYTICS_RETENTION_DAYS` | `365` | Event retention period |
| `SPARKLYTICS_GEOIP_PATH` | `./dbip-city-lite.mmdb` | DB-IP City Lite database path (bundled in Docker) |

### GeoIP

**Docker images bundle the DB-IP City Lite database automatically — no setup required.**

For bare-metal installs, download the free database (no license key needed):

```bash
./scripts/download-geoip.sh
export SPARKLYTICS_GEOIP_PATH=./dbip-city-lite.mmdb
```

> Attribution: This product includes IP geolocation data by [DB-IP.com](https://db-ip.com) (CC BY 4.0).
>
> You can also use MaxMind GeoLite2-City.mmdb — point `SPARKLYTICS_GEOIP_PATH` at it.

### Pre-built Binaries

Download from the [Releases](https://github.com/sparklytics/sparklytics/releases) page:

- `sparklytics-linux-amd64`
- `sparklytics-linux-arm64`
- `sparklytics-darwin-arm64`

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, Axum 0.8, Tokio |
| Storage | DuckDB (self-hosted) |
| Dashboard | Next.js 16, TailwindCSS, shadcn/ui |
| SDK | `@sparklytics/next` (npm) |
| Auth | Argon2id + JWT HttpOnly cookies |

---

## Development

```bash
# Backend
cargo build
cargo test
cargo run

# Dashboard (dev server at :3001, proxies /api to :3000)
cd dashboard && npm run dev

# SDK
cd sdk/next && npm run dev
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for full setup instructions.

---

## Documentation

- [Architecture Overview](docs/04-TECHNICAL-ARCHITECTURE.md)
- [API Specification](docs/07-API-SPECIFICATION.md)
- [Database Schema](docs/08-DATABASE-SCHEMA.md)
- [Self-Hosted Auth](docs/13-SELF-HOSTED-AUTH.md)
- [SDK (`@sparklytics/next`)](https://github.com/Sparklytics/sparklytics-next)
- [Sprint Plan](docs/sprints/sprint-index.md)

---

## License

MIT — see [LICENSE](LICENSE).
