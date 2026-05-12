# First Launch Runbook (Self-Hosted)

**Last Updated:** 2026-05-11
**Audience:** Operators launching Sparklytics for the first time
**Goal:** Get from zero to a working dashboard with verified data ingestion in one pass.

---

## What "Successful First Launch" Means

Your first launch is successful when all of these are true:

1. `GET /health` returns `200`.
2. You can reach the dashboard in a browser.
3. Auth flow matches your selected mode (`local`, `password`, or `none`).
4. You can reach onboarding, create your first website, and open its dashboard.
5. A test pageview appears in stats.

---

## 1. Choose Your Auth Mode First

`SPARKLYTICS_AUTH` controls first-run behavior:

| Mode | Value | First-run UX | Best for |
|---|---|---|---|
| Local auth (recommended) | `local` (default) | Redirects to `/setup` once, then `/login` | Most self-hosted production installs |
| Env password | `password` | Redirects directly to `/login`; no setup page | Simple single-password deployments |
| Open mode | `none` | No login/setup; dashboard opens directly | Local development behind trusted network only |

Important behavior:
- In `password` mode, `SPARKLYTICS_PASSWORD` is required at startup.
- In `none` mode, `/api/auth/status` returns `404` by design.
- In `local` mode, first run requires `POST /api/auth/setup` once with a bootstrap password.
- Set `SPARKLYTICS_BOOTSTRAP_PASSWORD` explicitly when possible. If unset, Sparklytics falls back to `sparklytics` and forces a password rotation after first login.

---

## 2. Pre-Flight Checklist

Before launch:

1. Pick a persistent data location (Docker volume or host directory).
2. Decide whether users access Sparklytics over HTTPS.
3. Keep port `3000` open (or set `SPARKLYTICS_PORT`).
4. Set explicit DuckDB memory if needed: `SPARKLYTICS_DUCKDB_MEMORY` (default `1GB`).

Cookie rule:
- If testing on plain `http://localhost`, set `SPARKLYTICS_HTTPS=false`.
- If users access Sparklytics over HTTPS, keep `SPARKLYTICS_HTTPS=true`.

Recommended first-time path:

1. Start with Docker and the provided compose file.
2. Complete setup on localhost first.
3. Confirm the onboarding flow creates a website and verifies the snippet.
4. Only then move to a public domain with HTTPS.

---

## 3. Launch Commands

### 3.1 Docker: Local Auth (Recommended)

```bash
git clone https://github.com/Sparklytics/sparklytics.git
cd sparklytics
# docker-compose.yml is localhost-ready by default:
# - SPARKLYTICS_HTTPS=false
# - SPARKLYTICS_PUBLIC_URL=http://localhost:3000
#
# Before production use, set:
# - SPARKLYTICS_BOOTSTRAP_PASSWORD
# - SPARKLYTICS_PUBLIC_URL=https://your-analytics-domain.example
# - optional: SPARKLYTICS_TRACKING_PUBLIC_BASE=https://example.com/_sl
docker compose up --build -d
```

The source-build compose files pass `CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-1}`
to the Docker build to keep small VPS builds stable. Use the default for
memory-constrained builders. Set `CARGO_BUILD_JOBS=4` before
`docker compose up --build -d` on larger builders when you want faster source
builds.

This is the preferred first-run flow because it keeps the persistent volume and auth defaults aligned with the docs.

### 3.1a Docker: Prebuilt Image on VPS (No local build)

Use this when you want the server to pull a ready-made image from GHCR instead of compiling the app on the VPS:

```bash
curl -O https://raw.githubusercontent.com/Sparklytics/sparklytics/main/docker-compose.image.yml
export SPARKLYTICS_IMAGE_TAG=latest
export SPARKLYTICS_BOOTSTRAP_PASSWORD=replace-this-bootstrap-password
export SPARKLYTICS_PUBLIC_URL=https://analytics.example.com
# Optional first-party proxy snippet base:
# export SPARKLYTICS_TRACKING_PUBLIC_BASE=https://example.com/_sl
docker compose -f docker-compose.image.yml pull
docker compose -f docker-compose.image.yml up -d
```

If you want a specific build instead of `latest`, set `SPARKLYTICS_IMAGE_TAG` to a published `sha-*` tag from GHCR. If GHCR returns `unauthorized`, use the default source-build compose flow above until the package visibility is public.

### 3.2 Docker: Password Mode

```bash
docker run -d \
  --name sparklytics \
  -p 3000:3000 \
  -v sparklytics-data:/data \
  -e SPARKLYTICS_AUTH=password \
  -e SPARKLYTICS_PASSWORD='replace-with-strong-password' \
  -e SPARKLYTICS_HTTPS=false \
  -e SPARKLYTICS_DUCKDB_MEMORY=1GB \
  ghcr.io/sparklytics/sparklytics:latest
```

### 3.3 Docker: Open Mode (`none`)

```bash
docker run -d \
  --name sparklytics \
  -p 3000:3000 \
  -v sparklytics-data:/data \
  -e SPARKLYTICS_AUTH=none \
  -e SPARKLYTICS_DUCKDB_MEMORY=1GB \
  ghcr.io/sparklytics/sparklytics:latest
```

### 3.4 Binary Launch (No Docker)

```bash
mkdir -p ./data
# Optional first-party proxy snippet base:
# export SPARKLYTICS_TRACKING_PUBLIC_BASE=https://example.com/_sl
SPARKLYTICS_DATA_DIR=./data \
SPARKLYTICS_AUTH=local \
SPARKLYTICS_BOOTSTRAP_PASSWORD=replace-this-bootstrap-password \
SPARKLYTICS_HTTPS=false \
SPARKLYTICS_PUBLIC_URL=http://localhost:3000 \
SPARKLYTICS_DUCKDB_MEMORY=1GB \
./sparklytics
```

---

## 4. Browser Flow You Should See

Open `http://localhost:3000`.

### 4.1 `SPARKLYTICS_AUTH=local`

Expected first-run sequence:

1. `/dashboard` redirects to `/setup`.
2. Enter the bootstrap password from install time and set the admin password.
3. App transitions to login flow.
4. Sign in with the new password.
5. If the fallback bootstrap password `sparklytics` was used, Sparklytics immediately requires one more password rotation before showing the dashboard.
6. If no websites exist yet, Sparklytics takes you to onboarding.
7. Create your first website, copy the snippet, and verify a pageview.
8. Land on `/dashboard/<your-website-id>`.

### 4.2 `SPARKLYTICS_AUTH=password`

Expected first-run sequence:

1. `/dashboard` redirects to `/login`.
2. Sign in with `SPARKLYTICS_PASSWORD`.
3. If no websites exist yet, Sparklytics takes you to onboarding.
4. Create your first website, copy the snippet, and verify a pageview.
5. Visiting `/setup` redirects back to `/login` (setup is local-only).

### 4.3 `SPARKLYTICS_AUTH=none`

Expected first-run sequence:

1. `/dashboard` opens directly.
2. If no websites exist yet, Sparklytics takes you to onboarding.
3. No login and no setup screens are shown.
4. `/api/auth/status` is not registered and returns `404`.

---

## 5. Verify First-Run API Path (Copy/Paste)

### 5.1 Health

```bash
curl -i http://localhost:3000/health
```

Expect: `HTTP/1.1 200` and `{"status":"ok", ...}`.

### 5.2 Setup (local mode only)

```bash
curl -i -X POST http://localhost:3000/api/auth/setup \
  -H 'content-type: application/json' \
  -d '{"bootstrap_password":"replace-this-bootstrap-password","password":"correct horse battery staple"}'
```

Expect first run: `201 Created`.
Expect later runs: `410 Gone` (setup is one-time).

### 5.3 Login (local/password modes)

```bash
curl -i -c /tmp/spk.cookies -X POST http://localhost:3000/api/auth/login \
  -H 'content-type: application/json' \
  -d '{"password":"correct horse battery staple"}'
```

Expect: `200 OK` and `Set-Cookie: spk_session=...`.
In `password` mode, this value must exactly match `SPARKLYTICS_PASSWORD`.

### 5.4 Confirm No Website Exists Yet

Fresh installs start with zero websites. You create the first one in onboarding.

```bash
curl -s -b /tmp/spk.cookies http://localhost:3000/api/websites
```

In `none` mode, the same request works without `-b /tmp/spk.cookies`.
Expect: `"data":[]` on a brand-new instance.

### 5.5 Create the First Website

```bash
curl -s -b /tmp/spk.cookies -X POST http://localhost:3000/api/websites \
  -H 'content-type: application/json' \
  -d '{"name":"Example Site","domain":"example.com","timezone":"UTC"}'
```

Save the returned `data.id` as `WEBSITE_ID`. In `none` mode, the same request works without `-b /tmp/spk.cookies`.

### 5.6 Send a Test Pageview

```bash
WEBSITE_ID=replace-with-created-id
curl -i -X POST http://localhost:3000/api/collect \
  -H 'content-type: application/json' \
  -d "{\"website_id\":\"${WEBSITE_ID}\",\"type\":\"pageview\",\"url\":\"/first-launch\",\"referrer\":\"https://example.com\",\"screen\":\"1440x900\",\"language\":\"en-US\"}"
```

Expect: `202 Accepted` with `{"ok":true}`.

`POST /e` is the supported short ingest alias and behaves the same as `POST /api/collect`.
Use it for first-party paths such as `/_sl/e`; Sparklytics serves that alias natively when the path reaches the service, and reverse proxies can map the same path from another app/domain.

### 5.7 Query Stats

```bash
TODAY=$(date +%F)
curl -s -b /tmp/spk.cookies \
  "http://localhost:3000/api/websites/${WEBSITE_ID}/stats?start_date=${TODAY}&end_date=${TODAY}"
```

Expect `pageviews >= 1` after your test event.

### 5.8 Automated Local API Smoke

For a deterministic local check of the same self-host API path:

```bash
cd dashboard && npm run build && cd ..
cargo build --release
node scripts/selfhost-api-smoke.mjs
```

The script starts fresh `local`, `password`, and `none` mode instances on loopback ports, then verifies setup, login cookie flags, first website creation, `tenant_id: null`, `/s.js`, `/_sl/s.js`, `/e`, `/api/collect`, `/_sl/e`, stats after flush, password mode login, `none` mode 404 on `/api/auth/status`, snippet `SPARKLYTICS_TRACKING_PUBLIC_BASE`, and data persistence across restart.

### 5.9 Browser Release Smoke

For the full first-launch browser path, run the Playwright release smoke from the dashboard workspace:

```bash
cd dashboard
npm run test:release-smoke
```

This starts a fresh self-hosted backend, builds and serves the static dashboard, then verifies setup, forced password rotation, onboarding, snippet modes, collect, stats, and dashboard navigation.

By default the backend uses `http://127.0.0.1:3000`. To avoid a local port collision, set one backend origin and let the smoke wire the backend port, `SPARKLYTICS_PUBLIC_URL`, proxy target, and snippet assertion from it:

```bash
PLAYWRIGHT_RELEASE_BACKEND_URL=http://127.0.0.1:3333 npm run test:release-smoke
```

This smoke requires permission to bind local loopback ports. If your environment blocks `listen`/bind or Docker sockets, record that as an environment blocker and run the API smoke where local ports are allowed.

### Production minimum before sharing the URL

- HTTPS enabled and working end-to-end
- persistent `/data` volume verified across restart
- `SPARKLYTICS_PUBLIC_URL` set to the final public origin
- `SPARKLYTICS_TRACKING_PUBLIC_BASE` set when you want the generated snippet to use a first-party proxy path such as `https://example.com/_sl`
- `SPARKLYTICS_TRUSTED_PROXIES` set when behind Caddy/Nginx/Traefik
- explicit DuckDB memory cap set
- `SPARKLYTICS_CORS_ORIGINS` narrowed to expected origins
- backup process for the DuckDB file documented
- `SPARKLYTICS_AUTH=none` disabled unless the instance is private and trusted

---

## 6. Common First-Launch Failures

| Symptom | Likely cause | Fix |
|---|---|---|
| Login succeeds but user is sent back to login | Running over plain HTTP with `SPARKLYTICS_HTTPS=true`, so browser drops `Secure` cookie | Set `SPARKLYTICS_HTTPS=false` for localhost/non-TLS environments and restart |
| `SPARKLYTICS_AUTH=password` but startup fails | Missing `SPARKLYTICS_PASSWORD` | Set `SPARKLYTICS_PASSWORD` and restart |
| `POST /api/auth/setup` returns `410` | Setup already completed for this data directory | Log in normally, or reset data if you intentionally want a clean reinstall |
| `429 rate_limited` on login | Too many failed attempts | Wait for `Retry-After` seconds (15 min window) before retrying |
| Dashboard shows no data after snippet install | Wrong `website_id`, snippet not deployed to pages, or no traffic yet | Recheck snippet, hit your site once, then re-verify |
| Browser console shows `ERR_BLOCKED_BY_CLIENT` for `s.js` or collect requests | Extension or privacy tool blocked the analytics subdomain/path | Prefer a first-party proxy path such as `/_sl/s.js` and `/_sl/e`, then hard refresh |
| Browser receives Cloudflare challenge / `Just a moment...` for analytics requests | Cloudflare WAF or Managed Challenge is intercepting the analytics origin | Use a first-party proxy path on the tracked site, or add a bypass rule for the tracker and ingest endpoints |
| Script loads but no pageviews appear | Tracker path reaches the server, but the ingest endpoint does not | Make sure both `/_sl/s.js` and `/_sl/e` reach Sparklytics, then retry verification |
| Data disappears after container restart | No persistent volume mounted | Mount `/data` to a named volume or host path |

---

## 7. Safe Reset for a True "Fresh First Launch"

Only do this if you intentionally want to wipe local data.

### Docker with named volume

```bash
docker rm -f sparklytics
docker volume rm sparklytics-data
```

### Binary with local directory

```bash
rm -rf ./data
mkdir -p ./data
```

Then start again and repeat the mode-specific first-run flow.

---

## 8. Post-Launch Hardening (Recommended)

After first successful launch:

1. Put Sparklytics behind HTTPS (Nginx/Caddy/Traefik).
2. Keep `SPARKLYTICS_HTTPS=true` in HTTPS deployments.
3. Set `SPARKLYTICS_CORS_ORIGINS` explicitly (comma-separated allowlist).
4. Keep DuckDB memory explicit (`SPARKLYTICS_DUCKDB_MEMORY=1GB` or higher for larger VPS).
5. Back up the DuckDB file in `SPARKLYTICS_DATA_DIR`.
6. Use strong passwords in `local`/`password` mode and rotate periodically.
7. Prefer a first-party proxy path (`/_sl`) for public marketing/content sites behind Cloudflare or privacy tooling.

---

## Related Docs

- [README.md](README.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
