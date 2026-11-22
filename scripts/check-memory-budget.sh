#!/usr/bin/env bash
set -euo pipefail

ROTAMETER_BIN="${ROTAMETER_BIN:-target/release/rotameter}"
IDLE_LIMIT_KB="${IDLE_LIMIT_KB:-40960}"   # 40 MB
PEAK_LIMIT_KB="${PEAK_LIMIT_KB:-131072}"  # 128 MB

if [ ! -x "$ROTAMETER_BIN" ]; then
  echo "ERROR: executable not found at ${ROTAMETER_BIN} (run cargo build --release first)" >&2
  exit 1
fi

port="$(node -e 'const n=require("net");const s=n.createServer();s.listen(0,"127.0.0.1",()=>{console.log(s.address().port);s.close();})')"
tmp_dir="$(mktemp -d)"
db="${tmp_dir}/mem.db"
admin="rm_adm_mem_budget_token_123456"
pid=""

cleanup() {
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    kill -TERM "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

"$ROTAMETER_BIN" serve --bind "127.0.0.1:${port}" --db "$db" --bootstrap-admin-key "$admin" --no-self-site >/dev/null 2>&1 &
pid=$!

base="http://127.0.0.1:${port}"
for _ in $(seq 1 50); do
  if curl -fsS "${base}/v1/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.05
done
curl -fsS "${base}/v1/health" >/dev/null

idle_kb="$(ps -o rss= -p "$pid" | tr -d '[:space:]')"
echo "Memory gate: idle RSS=${idle_kb} KiB ($((idle_kb / 1024)) MB, limit < $((IDLE_LIMIT_KB / 1024)) MB)"
if [ "$idle_kb" -ge "$IDLE_LIMIT_KB" ]; then
  echo "ERROR: idle RSS ${idle_kb} KiB exceeds ${IDLE_LIMIT_KB} KiB" >&2
  exit 1
fi

node -e '
const base = process.argv[1], admin = process.argv[2];
(async () => {
  const hdrs = { authorization: `Bearer ${admin}`, "content-type": "application/json" };
  await fetch(`${base}/v1/admin/sites`, { method: "POST", headers: hdrs, body: JSON.stringify({ slug: "mem.dev" }) });
  const ts = new Date().toISOString();
  for (let b = 0; b < 10; b++) {
    const events = Array.from({ length: 500 }, (_, i) => ({
      site_id: "mem.dev", url: `https://mem.dev/p/${(b * 500 + i) % 50}`, ts, visitor_hint: `u-${b}-${i}`
    }));
    const p1 = fetch(`${base}/v1/events/batch`, { method: "POST", headers: hdrs, body: JSON.stringify({ events }) });
    const p2 = fetch(`${base}/v1/query`, { method: "POST", headers: hdrs, body: JSON.stringify({ site_id: "mem.dev", range: { preset: "24h" }, metrics: ["visitors", "pageviews"], group_by: ["path"] }) });
    await Promise.all([p1, p2]);
  }
})();
' "$base" "$admin" &
burst_pid=$!

peak_kb="$idle_kb"
while kill -0 "$burst_pid" 2>/dev/null; do
  cur_kb="$(ps -o rss= -p "$pid" 2>/dev/null | tr -d '[:space:]' || echo 0)"
  if [ -n "$cur_kb" ] && [ "$cur_kb" -gt "$peak_kb" ]; then
    peak_kb="$cur_kb"
  fi
  sleep 0.02
done
wait "$burst_pid"

echo "Memory gate: peak RSS=${peak_kb} KiB ($((peak_kb / 1024)) MB, limit < $((PEAK_LIMIT_KB / 1024)) MB)"
if [ "$peak_kb" -ge "$PEAK_LIMIT_KB" ]; then
  echo "ERROR: peak RSS ${peak_kb} KiB exceeds ${PEAK_LIMIT_KB} KiB" >&2
  exit 1
fi

kill -TERM "$pid"
wait "$pid"
pid=""
echo "OK: idle and burst memory budgets passed."
