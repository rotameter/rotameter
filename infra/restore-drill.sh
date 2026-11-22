#!/usr/bin/env bash
set -euo pipefail

ROTAMETER_BIN="${ROTAMETER_BIN:-rotameter}"
MODE=""
BACKUP_FILE=""
LITESTREAM_CFG=""
SRC_DB="/var/lib/rotameter/rotameter.db"
DEST_DIR=""

usage() {
  cat <<'EOF'
Usage:
  infra/restore-drill.sh --self-test
  infra/restore-drill.sh --backup <snapshot.db> [--dest <restored.db>]
  infra/restore-drill.sh --litestream <litestream.yml> [--db <src_db_path>] [--dest <restored.db>]
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --self-test) MODE="self-test"; shift ;;
    --backup) MODE="backup"; BACKUP_FILE="$2"; shift 2 ;;
    --litestream) MODE="litestream"; LITESTREAM_CFG="$2"; shift 2 ;;
    --db) SRC_DB="$2"; shift 2 ;;
    --dest) DEST_DIR="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown flag: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[ -n "$MODE" ] || { usage >&2; exit 2; }

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

if [ "$MODE" = "self-test" ]; then
  src_db="${tmp_dir}/source.db"
  bak_db="${tmp_dir}/backup.db"
  restored_db="${tmp_dir}/restored.db"
  jsonl="${tmp_dir}/seed.jsonl"

  "$ROTAMETER_BIN" site upsert drill.dev --name "Drill Site" --db "$src_db" --quiet >/dev/null
  printf '{"url":"https://drill.dev/hello","ts":"2026-11-22T12:00:00Z","visitor_hint":"drill-1"}\n' > "$jsonl"
  "$ROTAMETER_BIN" import drill.dev "$jsonl" --db "$src_db" --quiet >/dev/null

  before_sites="$("$ROTAMETER_BIN" site list --db "$src_db" --format json)"
  "$ROTAMETER_BIN" backup "$bak_db" --db "$src_db" --quiet >/dev/null

  cp "$bak_db" "$restored_db"
  "$ROTAMETER_BIN" doctor --local --db "$restored_db" --quiet >/dev/null
  after_sites="$("$ROTAMETER_BIN" site list --db "$restored_db" --format json)"

  if [ "$before_sites" != "$after_sites" ]; then
    echo "ERROR: restored sites mismatch source sites" >&2
    exit 1
  fi
  echo "OK: restore drill self-test verified backup, WAL doctor, and site parity."
  exit 0
fi

restored_db="${DEST_DIR:-${tmp_dir}/restored.db}"
if [ "$MODE" = "backup" ]; then
  [ -f "$BACKUP_FILE" ] || { echo "ERROR: backup not found: $BACKUP_FILE" >&2; exit 1; }
  cp "$BACKUP_FILE" "$restored_db"
elif [ "$MODE" = "litestream" ]; then
  command -v litestream >/dev/null 2>&1 || { echo "ERROR: litestream binary not found" >&2; exit 1; }
  litestream restore -if-replica-exists -o "$restored_db" -config "$LITESTREAM_CFG" "$SRC_DB"
fi

"$ROTAMETER_BIN" doctor --local --db "$restored_db"
"$ROTAMETER_BIN" site list --db "$restored_db" --format table
echo "OK: restore drill succeeded at ${restored_db}"
