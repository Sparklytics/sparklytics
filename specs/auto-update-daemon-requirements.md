# Self-Hosted Auto-Update Daemon Requirements

**Version:** 1.0
**Target:** V1.2 (Post-cloud launch)
**Status:** Planning
**Priority:** Medium-High (unique differentiator, no competitor has this)

## Problem

Self-hosted users want privacy and control but hate maintenance. Current update process for Umami/Plausible:

```bash
git pull
docker-compose down
docker-compose build   # 5-10 minutes
docker-compose up -d
# Something broke? Debug for 2 hours
# Forgot to backup? Data loss
```

Zero open-source analytics tools offer auto-updates, built-in health monitoring, or automated backups. This is a genuine gap in the market.

## Solution

A lightweight daemon process built into the Sparklytics binary that handles updates, health monitoring, and backups automatically.

## Architecture

```
┌──────────────────────────────────────────────┐
│           sparklytics binary                  │
│                                              │
│  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Main Server  │  │  Daemon (background)  │  │
│  │  (analytics)  │  │                      │  │
│  │              │  │  - Update checker     │  │
│  │  Port 3000   │  │  - Health monitor     │  │
│  │              │  │  - Backup scheduler   │  │
│  │              │  │  - Notification sender │  │
│  └──────────────┘  └──────────────────────┘  │
│                                              │
│  Health Dashboard: GET /health (port 3000)    │
└──────────────────────────────────────────────┘
```

The daemon runs as background tasks within the main process (tokio tasks), not a separate binary. This keeps deployment simple - still one binary, one container.

## Features

### 1. Auto-Update

**How it works:**

```
1. Daily check: GET https://api.github.com/repos/sparklytics/sparklytics/releases/latest
2. Compare: current_version vs latest_version (semver)
3. If update available:
   a. Download new binary for current arch (linux-amd64, linux-arm64, etc.)
   b. Verify checksum (SHA256, published in release)
   c. Run pre-update health check (is current instance healthy?)
   d. Run DuckDB backup (automatic, before any update)
   e. Replace binary (atomic rename)
   f. Restart process (exec syscall for zero-downtime)
   g. Run post-update health check (5 attempts, 2s apart)
   h. If post-update check fails: rollback to previous binary, notify user
4. Log result and send notification
```

**Configuration:**

```bash
# Environment variables
SPARKLYTICS_AUTO_UPDATE=true          # default: false (opt-in)
SPARKLYTICS_UPDATE_CHANNEL=stable     # stable | beta
SPARKLYTICS_UPDATE_SCHEDULE=daily     # daily | weekly | manual
SPARKLYTICS_UPDATE_TIME=03:00         # UTC time for update check
SPARKLYTICS_NOTIFICATION_URL=         # Webhook URL (Discord, Slack, etc.)
SPARKLYTICS_NOTIFICATION_EMAIL=       # Email for notifications
```

**Docker mode:**

For Docker deployments, auto-update works differently:
- Checks for new Docker image tag
- Sends notification to user (can't auto-update a running container)
- Provides one-liner to update: `docker pull sparklytics/sparklytics:latest && docker restart sparklytics`

**Binary mode (recommended for auto-update):**

For binary deployments, the full auto-update flow works:
- Downloads new binary
- Atomic swap via rename
- exec() to restart with new binary
- Rollback if health check fails

### 2. Health Dashboard

Built into the main server at `GET /health` (extended version):

**Simple health check (for load balancers):**
```
GET /health
200 OK
{"status": "healthy"}
```

**Detailed health dashboard (for humans):**
```
GET /health?detail=true
200 OK
{
  "status": "healthy",
  "version": "0.3.2",
  "latest_version": "0.3.2",
  "update_available": false,
  "uptime_seconds": 4060800,
  "uptime_human": "47 days",

  "database": {
    "status": "connected",
    "backend": "duckdb",
    "size_bytes": 2576980377,
    "size_human": "2.4 GB",
    "event_count": 1234567,
    "oldest_event": "2025-08-15T00:00:00Z"
  },

  "performance": {
    "events_per_second": 12.4,
    "query_latency_p99_ms": 45,
    "error_rate_percent": 0.02,
    "buffer_size": 34,
    "buffer_capacity": 100
  },

  "backup": {
    "last_backup": "2026-02-17T01:00:00Z",
    "last_backup_human": "2 hours ago",
    "backup_size_bytes": 1288490188,
    "next_backup": "2026-02-18T01:00:00Z",
    "destination": "/mnt/backups"
  },

  "system": {
    "memory_used_mb": 78,
    "memory_limit_mb": 256,
    "cpu_percent": 2.3,
    "disk_free_gb": 45.2
  }
}
```

**HTML Dashboard (optional):**

When accessed via browser (Accept: text/html), render a simple HTML page:

```
┌─ Sparklytics Health ─────────────────────┐
│ Status: ✅ Healthy                        │
│ Version: 0.3.2 (latest)                  │
│ Uptime: 47 days                          │
│                                          │
│ Database: ✅ DuckDB Connected             │
│   - Size: 2.4 GB (1.2M events)          │
│   - Oldest event: Aug 15, 2025           │
│                                          │
│ Performance:                              │
│   - Events/sec: 12.4                     │
│   - Query p99: 45ms                      │
│   - Error rate: 0.02%                    │
│                                          │
│ Last backup: 2 hours ago ✅              │
│ Next update check: 4 hours               │
│                                          │
│ Memory: 78 MB / 256 MB                   │
│ Disk: 45.2 GB free                       │
└──────────────────────────────────────────┘
```

This is a single static HTML page embedded in the binary. No JavaScript, no external dependencies. Pure HTML + inline CSS.

### 3. Automated Backups

```bash
# Configuration
SPARKLYTICS_BACKUP_ENABLED=true       # default: false
SPARKLYTICS_BACKUP_SCHEDULE=daily     # daily | weekly
SPARKLYTICS_BACKUP_TIME=01:00         # UTC
SPARKLYTICS_BACKUP_DESTINATION=/mnt/backups   # local path or s3://bucket/path
SPARKLYTICS_BACKUP_RETAIN=7           # keep last N backups
```

**DuckDB backup process:**

```rust
// 1. Create consistent snapshot
conn.execute("CHECKPOINT")?;

// 2. Copy database file
let backup_name = format!("sparklytics-{}.db", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
fs::copy(&config.duckdb_path, &backup_dir.join(&backup_name))?;

// 3. Compress (optional, zstd)
zstd::encode(&backup_path, &compressed_path, 3)?;

// 4. Upload to destination (if S3)
if config.backup_destination.starts_with("s3://") {
    s3_upload(&compressed_path, &config.backup_destination).await?;
}

// 5. Cleanup old backups (keep last N)
cleanup_old_backups(&backup_dir, config.backup_retain)?;

// 6. Log and notify
info!("Backup complete: {} ({} compressed)", backup_name, compressed_size);
```

**ClickHouse backup (self-hosted ClickHouse on VPS):**

```bash
# Uses ClickHouse native backup
clickhouse-client --query "BACKUP TABLE sparklytics.events, sparklytics.sessions \
    TO Disk('backups', 'daily/$(date +%Y%m%d)')"
```

### 4. Notifications

Support multiple notification channels:

**Webhook (Discord, Slack, custom):**
```json
POST {SPARKLYTICS_NOTIFICATION_URL}
{
  "event": "update_available",
  "version": "0.3.3",
  "current_version": "0.3.2",
  "changelog_url": "https://github.com/sparklytics/sparklytics/releases/tag/v0.3.3",
  "timestamp": "2026-02-17T03:00:00Z"
}
```

**Events that trigger notifications:**
- Update available
- Update applied successfully
- Update failed (with rollback details)
- Health check failed (database down, high error rate)
- Backup completed
- Backup failed
- Disk space warning (<10% free)

**Email (optional, via SMTP or Resend):**
Simple text email with the same information.

## Implementation Notes

### Process Restart (Binary Mode)

For zero-downtime restarts on binary update:

```rust
use std::os::unix::process::CommandExt;

fn restart_with_new_binary(new_binary_path: &Path) -> ! {
    // 1. Flush event buffer
    buffer.force_flush().await;

    // 2. Close database connections gracefully
    backend.shutdown().await;

    // 3. exec() replaces current process with new binary
    // Same PID, same ports, same environment
    let err = std::process::Command::new(new_binary_path)
        .args(std::env::args().skip(1))
        .exec();

    // exec() only returns on error
    error!("Failed to restart: {}", err);
    std::process::exit(1);
}
```

### Docker Mode Limitations

In Docker, we can't replace the running binary. Instead:
- Notify user that update is available
- Provide `docker pull` command
- Optionally: Watchtower integration (popular Docker auto-updater)

### Security

- Binary downloads verified against SHA256 checksums from GitHub Release
- Checksums fetched over HTTPS from GitHub API
- No arbitrary code execution - only replace the sparklytics binary itself
- Rollback on any health check failure
- Update check uses read-only GitHub API (no authentication required for public repos)

## CLI Commands

```bash
# Check for updates manually
sparklytics update check

# Apply update now
sparklytics update apply

# Rollback to previous version
sparklytics update rollback

# Run backup now
sparklytics backup run

# List backups
sparklytics backup list

# Restore from backup
sparklytics backup restore --file sparklytics-20260217.db.zst

# Show health status
sparklytics health
```

## Marketing Value

This feature targets the r/selfhosted community specifically. The messaging:

> "Self-hosted. Without the ops."

> "Privacy-first analytics that updates itself, backs itself up, and tells you when something's wrong. No Grafana. No Prometheus. No 2am debugging sessions."

This is a loyalty feature - users who set up auto-updates and backups are extremely sticky. They won't switch to Umami because Umami doesn't have this.
