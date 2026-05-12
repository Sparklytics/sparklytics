# syntax=docker/dockerfile:1
# Multi-stage build: compile for the target platform, then copy into distroless.
# Use a Debian 12 builder so glibc/libstdc++ symbols match the Debian 12 runtime.
FROM rust:1.93.0-slim-bookworm@sha256:776861219cd851131c1cec3bbd7cbeb16b99a794048097eb69ad9682a8ed0d57 AS builder

# Build dependencies: C/C++ toolchain + curl for Node setup.
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

# Build the Next.js dashboard (static export → dashboard/out/) before compiling
# Rust. `sparklytics-server` embeds dashboard/out at compile time via include_dir.
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

# DuckDB's bundled C++ build can exceed small VPS/Docker Desktop memory when
# Cargo fans out all cores. Default to a conservative build, with an override
# for CI runners that have more memory.
WORKDIR /app
ARG CARGO_BUILD_JOBS=1
RUN cargo build --release --locked -j "${CARGO_BUILD_JOBS}" && cp target/release/sparklytics /sparklytics

# GeoIP enrichment is optional at runtime. Keep the image build deterministic:
# users can mount a DB-IP or MaxMind MMDB file and set SPARKLYTICS_GEOIP_PATH.
# /data must be owned by the distroless nonroot UID so fresh named volumes are
# writable on first boot.
RUN mkdir -p /geoip /data && chown -R 65532:65532 /geoip /data

# ─── Runtime stage ────────────────────────────────────────────
# distroless/cc-debian12:nonroot ships glibc + /etc/passwd + /etc/ssl/certs.
# uid 65532 / username "nonroot" — satisfies Docker best practices.
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:7e5b8df2f4d36f5599ef4ab856d7d444922531709becb03f3368c6d797d0a5eb

COPY --from=builder /sparklytics /sparklytics
COPY --from=builder /app/dashboard/out /dashboard/out
COPY --from=builder --chown=65532:65532 /geoip /geoip
COPY --from=builder --chown=65532:65532 /data /data

ENV SPARKLYTICS_DATA_DIR=/data
ENV SPARKLYTICS_PORT=3000
ENV SPARKLYTICS_AUTH=local
# Default is plain HTTP for local/container smoke. Set true whenever users
# access Sparklytics over HTTPS, including behind Caddy/Nginx/Traefik.
ENV SPARKLYTICS_HTTPS=false
# DuckDB's default memory behavior is too broad for server use; keep an
# explicit runtime cap even when operators do not pass the env var.
ENV SPARKLYTICS_DUCKDB_MEMORY=1GB
# Optional: mount a DB-IP or MaxMind MMDB file here. If absent, Sparklytics
# starts normally and stores NULL country/region/city fields.
ENV SPARKLYTICS_GEOIP_PATH=/geoip/dbip-city-lite.mmdb

EXPOSE 3000
VOLUME ["/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/sparklytics", "health"]

ENTRYPOINT ["/sparklytics"]
