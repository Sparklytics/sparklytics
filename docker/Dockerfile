# syntax=docker/dockerfile:1
# Multi-stage build: compile for the target platform, then copy into distroless.
FROM rust:slim AS builder

# Build dependencies: C/C++ toolchain + Node for dashboard build + curl for DB-IP.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Node 20 for dashboard build.
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build for the native target platform selected by Buildx.
RUN cargo build --release --locked && cp target/release/sparklytics /sparklytics

# Build the Next.js dashboard (static export → dashboard/out/).
WORKDIR /app/dashboard
RUN set -eux; \
    npm config set registry https://registry.npmjs.org/; \
    npm config set fetch-retries 5; \
    npm config set fetch-retry-mintimeout 20000; \
    npm config set fetch-retry-maxtimeout 120000; \
    for attempt in 1 2 3; do \
      timeout 10m npm ci --no-audit && break; \
      if [ "$attempt" -eq 3 ]; then exit 1; fi; \
      echo "npm ci failed on attempt ${attempt}; retrying..."; \
      sleep 15; \
    done; \
    npm run build

# Download DB-IP City Lite (free, CC BY 4.0 — no license key required).
# Falls back to the previous month if this month's file isn't published yet.
# Attribution: This product includes IP geolocation data by DB-IP.com (https://db-ip.com)
WORKDIR /tmp
RUN YEAR=$(date +%Y) && MONTH=$(date +%m) && \
    PREV_MONTH=$(date -d "-1 month" +%m 2>/dev/null || \
                 python3 -c "import datetime; d=datetime.date.today().replace(day=1)-datetime.timedelta(days=1); print(d.strftime('%m'))") && \
    PREV_YEAR=$(date -d "-1 month" +%Y 2>/dev/null || \
                python3 -c "import datetime; d=datetime.date.today().replace(day=1)-datetime.timedelta(days=1); print(d.strftime('%Y'))") && \
    BASE="https://download.db-ip.com/free" && \
    ( curl -fsSL "${BASE}/dbip-city-lite-${YEAR}-${MONTH}.mmdb.gz" -o dbip.mmdb.gz || \
      curl -fsSL "${BASE}/dbip-city-lite-${PREV_YEAR}-${PREV_MONTH}.mmdb.gz" -o dbip.mmdb.gz ) && \
    gunzip dbip.mmdb.gz && \
    mv dbip.mmdb /dbip-city-lite.mmdb

# ─── Runtime stage ────────────────────────────────────────────
# distroless/cc-debian12:nonroot ships glibc + /etc/passwd + /etc/ssl/certs.
# uid 65532 / username "nonroot" — satisfies Docker best practices.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /sparklytics /sparklytics
COPY --from=builder /app/dashboard/out /dashboard/out
# DB-IP City Lite database — bundled, no user action needed.
# Override with SPARKLYTICS_GEOIP_PATH if you prefer MaxMind GeoLite2.
COPY --from=builder /dbip-city-lite.mmdb /geoip/dbip-city-lite.mmdb

ENV SPARKLYTICS_DATA_DIR=/data
ENV SPARKLYTICS_PORT=3000
ENV SPARKLYTICS_AUTH=local
ENV SPARKLYTICS_GEOIP_PATH=/geoip/dbip-city-lite.mmdb

EXPOSE 3000
VOLUME ["/data"]

# distroless/static:nonroot already sets USER nonroot (uid 65532).
USER nonroot

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/sparklytics", "health"]

ENTRYPOINT ["/sparklytics"]
