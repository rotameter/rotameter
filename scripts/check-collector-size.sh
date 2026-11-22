#!/usr/bin/env bash
set -euo pipefail

COLLECTOR_PATH="${1:-assets/collect.js}"
MAX_GZIP_BYTES="${MAX_GZIP_BYTES:-1536}"

if [ ! -f "$COLLECTOR_PATH" ]; then
  echo "ERROR: collector asset not found at ${COLLECTOR_PATH}" >&2
  exit 1
fi

raw_bytes=$(wc -c < "$COLLECTOR_PATH" | tr -d '[:space:]')
gzip_bytes=$(gzip -9 -c "$COLLECTOR_PATH" | wc -c | tr -d '[:space:]')

forbidden_patterns=(
  "document\.cookie"
  "sessionStorage"
  "localStorage\.setItem"
  "indexedDB"
)

for pattern in "${forbidden_patterns[@]}"; do
  if grep -Eq "$pattern" "$COLLECTOR_PATH"; then
    echo "ERROR: forbidden tracking/storage pattern '${pattern}' found in ${COLLECTOR_PATH}" >&2
    exit 1
  fi
done

echo "Collector stats (${COLLECTOR_PATH}): raw=${raw_bytes} B, gzip=${gzip_bytes} B (limit <= ${MAX_GZIP_BYTES} B)"

if [ "$gzip_bytes" -gt "$MAX_GZIP_BYTES" ]; then
  echo "ERROR: gzip size ${gzip_bytes} B exceeds ${MAX_GZIP_BYTES} B ceiling" >&2
  exit 1
fi

if [ "$COLLECTOR_PATH" = "assets/collect.js" ] && [ -f "crates/rotameter-server/assets/collect.js" ]; then
  if ! cmp -s "$COLLECTOR_PATH" "crates/rotameter-server/assets/collect.js"; then
    echo "ERROR: crates/rotameter-server/assets/collect.js drifted from assets/collect.js" >&2
    exit 1
  fi
fi

echo "OK: collector gzip budget and privacy invariants passed."
