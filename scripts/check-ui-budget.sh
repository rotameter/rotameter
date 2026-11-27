#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../ui"
exec bun scripts/check-budget.mjs
