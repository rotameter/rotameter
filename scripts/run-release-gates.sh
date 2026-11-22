#!/usr/bin/env bash
set -euo pipefail

./scripts/check-core-purity.sh
./scripts/check-collector-size.sh
node --test scripts/test-collect-js.mjs
node --test integrations/astro-cloudflare/tests/proxy.test.ts integrations/astro-cloudflare/tests/ssr_astro.test.ts

cargo build --workspace --release --bin rotameter
ROTAMETER_BIN=./target/release/rotameter ./scripts/check-memory-budget.sh
ROTAMETER_BIN=./target/release/rotameter ./infra/restore-drill.sh --self-test
ROTAMETER_BIN=./target/release/rotameter node scripts/test-astro-e2e.mjs

cargo test -p rotameter-cli --release --test release_query_sla -- --ignored --nocapture

echo "OK: all v1.0 release gates passed."
