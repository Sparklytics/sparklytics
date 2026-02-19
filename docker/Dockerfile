# syntax=docker/dockerfile:1
# Multi-stage build: compile a static musl binary, then copy into distroless.
FROM --platform=$BUILDPLATFORM rust:slim AS builder

ARG TARGETARCH

# Build dependencies: musl cross-compiler + Node for dashboard build + curl for DB-IP.
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    gcc-aarch64-linux-gnu \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Node 20 for dashboard build.
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build for the target architecture.
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      rustup target add aarch64-unknown-linux-musl && \
      CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc \
      cargo build --release --target aarch64-unknown-linux-musl && \
      cp target/aarch64-unknown-linux-musl/release/sparklytics /sparklytics; \
    else \
      rustup target add x86_64-unknown-linux-musl && \
      cargo build --release --target x86_64-unknown-linux-musl && \
      cp target/x86_64-unknown-linux-musl/release/sparklytics /sparklytics; \
    fi

# Build the Next.js dashboard (static export → dashboard/out/).
WORKDIR /app/dashboard
RUN npm ci && npm run build

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
# distroless/static:nonroot ships /etc/passwd + /etc/ssl/certs.
# uid 65532 / username "nonroot" — satisfies Docker best practices.
FROM gcr.io/distroless/static:nonroot

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
