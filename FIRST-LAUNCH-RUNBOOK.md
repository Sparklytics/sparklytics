# First Launch Runbook (Self-Hosted)

**Last Updated:** 2026-02-24
**Audience:** Operators launching Sparklytics for the first time
**Goal:** Get from zero to a working dashboard with verified data ingestion in one pass.

---

## What "Successful First Launch" Means

Your first launch is successful when all of these are true:

1. `GET /health` returns `200`.
2. You can reach the dashboard in a browser.
3. Auth flow matches your selected mode (`local`, `password`, or `none`).
4. You can access `/dashboard/site_default` (or your created website).
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
- In `local` mode, first run requires `POST /api/auth/setup` once.

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

---

## 3. Launch Commands

### 3.1 Docker: Local Auth (Recommended)

```bash
docker run -d \
  --name sparklytics \
  -p 3000:3000 \
  -v sparklytics-data:/data \
  -e SPARKLYTICS_AUTH=local \
  -e SPARKLYTICS_HTTPS=false \
  -e SPARKLYTICS_DUCKDB_MEMORY=1GB \
  sparklytics/sparklytics:latest
```

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
  sparklytics/sparklytics:latest
```

### 3.3 Docker: Open Mode (`none`)

```bash
docker run -d \
  --name sparklytics \
  -p 3000:3000 \
  -v sparklytics-data:/data \
  -e SPARKLYTICS_AUTH=none \
  -e SPARKLYTICS_DUCKDB_MEMORY=1GB \
  sparklytics/sparklytics:latest
```

### 3.4 Binary Launch (No Docker)

```bash
mkdir -p ./data
SPARKLYTICS_DATA_DIR=./data \
SPARKLYTICS_AUTH=local \
SPARKLYTICS_HTTPS=false \
SPARKLYTICS_DUCKDB_MEMORY=1GB \
./sparklytics
```

---

## 4. Browser Flow You Should See

Open `http://localhost:3000`.

### 4.1 `SPARKLYTICS_AUTH=local`

Expected first-run sequence:

1. `/dashboard` redirects to `/setup`.
2. Set admin password (minimum 12 chars, not whitespace-only).
3. App transitions to login flow.
4. Sign in with the new password.
5. Land on `/dashboard/site_default`.

### 4.2 `SPARKLYTICS_AUTH=password`

Expected first-run sequence:

1. `/dashboard` redirects to `/login`.
2. Sign in with `SPARKLYTICS_PASSWORD`.
3. Land on `/dashboard/site_default`.
4. Visiting `/setup` redirects back to `/login` (setup is local-only).

### 4.3 `SPARKLYTICS_AUTH=none`

Expected first-run sequence:

1. `/dashboard` opens directly.
2. No login and no setup screens are shown.
3. `/api/auth/status` is not registered and returns `404`.

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
  -d '{"password":"correct horse battery staple"}'
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

### 5.4 Confirm Website Exists

Sparklytics seeds a default website on startup: `site_default`.

```bash
curl -s -b /tmp/spk.cookies http://localhost:3000/api/websites
```

In `none` mode, the same request works without `-b /tmp/spk.cookies`.

### 5.5 Send a Test Pageview

```bash
curl -i -X POST http://localhost:3000/api/collect \
  -H 'content-type: application/json' \
  -d '{"website_id":"site_default","type":"pageview","url":"/first-launch","referrer":"https://example.com","screen":"1440x900","language":"en-US"}'
```

Expect: `202 Accepted` with `{"ok":true}`.

### 5.6 Query Stats

```bash
TODAY=$(date +%F)
curl -s -b /tmp/spk.cookies \
  "http://localhost:3000/api/websites/site_default/stats?start_date=${TODAY}&end_date=${TODAY}"
```

Expect `pageviews >= 1` after your test event.

---

## 6. Common First-Launch Failures

| Symptom | Likely cause | Fix |
|---|---|---|
| Login succeeds but user is sent back to login | Running over plain HTTP with `SPARKLYTICS_HTTPS=true`, so browser drops `Secure` cookie | Set `SPARKLYTICS_HTTPS=false` for localhost/non-TLS environments and restart |
| `SPARKLYTICS_AUTH=password` but startup fails | Missing `SPARKLYTICS_PASSWORD` | Set `SPARKLYTICS_PASSWORD` and restart |
| `POST /api/auth/setup` returns `410` | Setup already completed for this data directory | Log in normally, or reset data if you intentionally want a clean reinstall |
| `429 rate_limited` on login | Too many failed attempts | Wait for `Retry-After` seconds (15 min window) before retrying |
| Dashboard shows no data after snippet install | Wrong `website_id`, snippet not deployed to pages, or no traffic yet | Recheck snippet, hit your site once, then re-verify |
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

---

## Related Docs

- [README.md](README.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
