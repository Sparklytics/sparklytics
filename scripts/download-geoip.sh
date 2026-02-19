#!/usr/bin/env bash
# download-geoip.sh — Download the DB-IP City Lite database (free, no license key).
#
# Usage:
#   ./scripts/download-geoip.sh
#   ./scripts/download-geoip.sh /custom/path/dbip-city-lite.mmdb
#
# DB-IP City Lite is free to use under the Creative Commons Attribution 4.0
# International License. Attribution: https://db-ip.com
#
# The downloaded file is compatible with the MaxMind MMDB format used by
# Sparklytics. Set SPARKLYTICS_GEOIP_PATH to point to the downloaded file.

set -euo pipefail

OUTPUT="${1:-./dbip-city-lite.mmdb}"
BASE_URL="https://download.db-ip.com/free"

# Try current month, fall back to previous month if not yet published.
YEAR=$(date +%Y)
MONTH=$(date +%m)
PREV=$(date -d "-1 month" "+%Y-%m" 2>/dev/null || \
       python3 -c "import datetime; d=datetime.date.today().replace(day=1)-datetime.timedelta(days=1); print(d.strftime('%Y-%m'))")

CURRENT_URL="${BASE_URL}/dbip-city-lite-${YEAR}-${MONTH}.mmdb.gz"
FALLBACK_URL="${BASE_URL}/dbip-city-lite-${PREV}.mmdb.gz"

TMP_GZ="$(mktemp).mmdb.gz"

echo "Downloading DB-IP City Lite (${YEAR}-${MONTH})..."
if ! curl -fsSL "$CURRENT_URL" -o "$TMP_GZ" 2>/dev/null; then
  echo "Current month not available, trying ${PREV}..."
  curl -fsSL "$FALLBACK_URL" -o "$TMP_GZ"
fi

echo "Extracting..."
gunzip -c "$TMP_GZ" > "$OUTPUT"
rm -f "$TMP_GZ"

SIZE=$(du -sh "$OUTPUT" | cut -f1)
echo "Saved to: $OUTPUT (${SIZE})"
echo ""
echo "Set in your environment:"
echo "  export SPARKLYTICS_GEOIP_PATH=$(realpath "$OUTPUT")"
echo ""
echo "Attribution: This product includes IP geolocation data by DB-IP.com (https://db-ip.com)"
