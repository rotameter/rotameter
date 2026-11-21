#!/usr/bin/env bash
set -euo pipefail

tree_output="$(cargo tree -p rotameter-core --edges normal --prefix none)"

forbidden_crates=(
  tokio
  rusqlite
  libsqlite3-sys
  axum
  hyper
  reqwest
  ratatui
  crossterm
  mio
  socket2
  libc
  sqlx
  diesel
  actix
  warp
  tonic
  async-std
  smol
  futures-io
)

for crate in "${forbidden_crates[@]}"; do
  if printf '%s\n' "$tree_output" | grep -Eq "(^|[[:space:]])${crate}([[:space:]]|$|-)"; then
    echo "ERROR: forbidden dependency '${crate}' found in rotameter-core dependency tree:" >&2
    printf '%s\n' "$tree_output" >&2
    exit 1
  fi
done

for entry in crates/*/src/lib.rs crates/*/src/main.rs; do
  [ -e "$entry" ] || continue
  if ! grep -q '#!\[forbid(unsafe_code)\]' "$entry"; then
    echo "ERROR: missing #![forbid(unsafe_code)] in ${entry}" >&2
    exit 1
  fi
done

printf '%s\n' "$tree_output"
echo "OK: rotameter-core purity and #![forbid(unsafe_code)] checks passed."
