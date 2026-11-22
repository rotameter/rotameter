<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/brand/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="./assets/brand/logo-light.svg">
    <img alt="Rotameter" src="./assets/brand/logo-light.svg" width="56" height="56">
  </picture>

  <h1>Rotameter</h1>
  <p><strong>A single-binary, privacy-first, headless web analytics engine in Rust.</strong></p>
  <p><code>MIT OR Apache-2.0</code> · Rust edition <code>2024</code> (MSRV <code>1.99</code>) · Current target: <code>0.1.0</code></p>
</div>

Rotameter is engineered for unattended durability on a **`1 vCPU / 1 GB RAM`** VM with zero cookies, zero persisted raw IPs, zero ClickHouse/PostgreSQL dependencies, and a clean-sheet `/v1/*` contract backed by bundled SQLite WAL + daily rollups.

---

## Verified Budgets & Benchmarks

Measured on release builds via [`scripts/run-release-gates.sh`](./scripts/run-release-gates.sh):

| Metric | Budget | Measured (`0.1.0` pre-release) |
| --- | --- | --- |
| **Idle RSS** | `< 40 MB` | **`5 MB`** (`5,684 KiB`) |
| **Peak RSS** (5k burst + query) | `< 128 MB` | **`14 MB`** (`14,640 KiB`) |
| **Collector [`assets/collect.js`](./assets/collect.js)** | `<= 1,536 B` gzip | **`735 B`** gzip (`1,385 B` raw) |
| **`30d` query SLA (`1M` events / `250k` sessions)** | `p95 < 25 ms` | **`p50 = 2.30 ms`, `p95 = 5.36 ms`, `p99 = 7.98 ms`** |
| **Memory & runtime safety** | `#![forbid(unsafe_code)]` | Enforced workspace-wide, zero non-test `unwrap`/`expect`/`panic!` |

---

## Key Capabilities (`0.1.0`)

- **Headless `/v1/*` engine:** Scoped SHA-256 Bearer tokens (`rm_ing_`, `rm_rd_`, `rm_adm_`), bounded `16,384`-event MPSC ingest ring buffer, graceful `SIGTERM` drain, `/v1/query` (`JSON` & `RFC 4180` CSV), `/v1/sites/{site}/realtime`, idempotent `/v1/admin/*`, and per-IP rate limiting.
- **Privacy & session accuracy:** Cookieless `HMAC-SHA256` rotating salt (`daily` with 30m midnight UTC grace carryover, opt-in `weekly`/`monthly`), 30m sliding session metrics (visits, bounce rate, visit duration, entry/exit), compiled-in bot filter, `CF-IPCountry` header + optional `.mmdb` GeoIP, URL/UTM normalization, cardinality caps into `(other)`, and local-only `self_site` initialization (never phones home).
- **Multi-path collection:** Sub-kilobyte `GET /v1/collect.js`, zero-JS `1x1` `/v1/pixel.gif`, `POST /v1/events/batch`, and first-party **Astro + Cloudflare Workers** proxy & SSR helper ([`@rotameter/astro-cloudflare`](./integrations/astro-cloudflare/README.md)).
- **SQLite WAL event stream + rollups:** Ordered per-session `events`, `sessions`, `dim_strings`, hourly/daily rollups (`rollups_hourly`, `rollups_daily`, `seen_visitors_daily`, `rollups_path_daily`), retention pruner, and online `VACUUM INTO` backups behind pluggable `Storage` traits.
- **Operator interfaces:** Unified `rotameter` CLI (plus `rotameter seed` demo generator), strictly read-only `stdio` MCP server (`rotameter mcp`, 7 tools), and interactive `ratatui` flight-instrument TUI (`rotameter tui`).

---

## Quickstart

Build the unified `rotameter` binary and audition locally with synthetic traffic (`rotameter seed`):

```bash
cargo build --release --bin rotameter
export PATH="$PWD/target/release:$PATH"

# 1. Seed 30 days of realistic synthetic curves + live visitors into /tmp/demo.db
rotameter --db /tmp/demo.db seed --site demo.local --days 30 --sessions-per-day 240 --realtime-visitors 18

# 2. Mint a read-only token or boot the daemon with an admin bootstrap key
export ROTAMETER_URL=http://127.0.0.1:8000
export ROTAMETER_TOKEN=rm_adm_dev_local_bootstrap
rotameter --db /tmp/demo.db serve --bind 127.0.0.1:8000 --bootstrap-admin-key "$ROTAMETER_TOKEN" &

# 3. Query from CLI, launch the interactive TUI, or connect an MCP client
rotameter query demo.local --range 30d --group-by path --compare
rotameter tui
rotameter mcp   # newline-delimited JSON-RPC 2.0 over stdio
```

---

## CLI Commands

| Command | Mode | Summary |
| --- | --- | --- |
| `rotameter serve` | Local DB | Run the HTTP ingest/query/admin daemon (`--bind`, `--db`, `--public-url`, `--no-self-site`, `--self-site-id`, `--geoip`, `--mmdb-path`, `--trusted-proxy`) |
| `rotameter site` | Local or Remote | `upsert`, `list`, `show`, `purge`, `delete` |
| `rotameter key` | Local or Remote | `create`, `list`, `revoke` (`ingest`, `read`, `admin` scopes) |
| `rotameter query` | Remote API | Query summary, timeseries, and breakdowns (`--range`, `--group-by`, `--filter`, `--compare`, `--format table\|json\|csv`) |
| `rotameter realtime` | Remote API | Stream 5-minute active visitors and 30-minute pulse (`--watch`) |
| `rotameter import` | Local or Remote | Backfill JSONL or Nginx/Caddy CLF logs (`--dry-run`) |
| `rotameter seed` | Local DB | Generate repeatable multi-day diurnal traffic & live visitors (`--site`, `--days`, `--sessions-per-day`, `--realtime-visitors`, `--seed`, `--reset`) |
| `rotameter backup` | Local DB | Online SQLite `VACUUM INTO` snapshot |
| `rotameter doctor` | Local or Remote | Verify local WAL health/pragmas or remote `/v1/health` + Cloudflare Access auth |
| `rotameter mcp` | Remote API (`stdio`) | Start `stdio` read-only MCP server (`7` tools) |
| `rotameter tui` | Remote API | Launch interactive `ratatui` dashboard (`s`, `1`–`5`, `←`/`→`, `Tab`, `Enter`/`Esc`, `?`, `q`) |

---

## Deployment, Integrations & Brand Assets

- **Production deployment:** Multi-arch `musl`/macOS release workflow, distroless `Dockerfile`, hardened `systemd` unit (`MemoryMax=256M`), Litestream → R2 replication, and verified restore drill — see [`infra/README.md`](./infra/README.md).
- **Astro & Cloudflare Workers:** Zero-dependency edge proxy (`/api/r.js`, `/api/e` via `waitUntil()`) + SSR `trackServerHit()` helper — see [`integrations/astro-cloudflare/README.md`](./integrations/astro-cloudflare/README.md).
- **Brand & favicons:** SVG symbol sprite ([`assets/brand/icons-sprite.svg`](./assets/brand/icons-sprite.svg)), standalone icons ([`assets/brand/icons/`](./assets/brand/icons)), adaptive light/dark favicon ([`assets/brand/favicon.svg`](./assets/brand/favicon.svg)), multi-size [`favicon.ico`](./assets/brand/favicon.ico), Apple/PWA PNGs (`16`, `32`, `180`, `192`, `512`), and [`site.webmanifest`](./assets/brand/site.webmanifest).

---

## Documentation & Roadmap

SemVer progression: **`0.1.0`** *(headless engine)* → **`0.2.0`** *(optional Web UI, OIDC/RBAC, GTM/Zaraz, HTTP `/mcp`)* → **`0.3.0`** *(funnels, cohorts, multi-backend `Storage`)* → **`0.4.0`** *(webhooks, sinks, WASM transforms)* → **`1.0.0`** *(stable GA freeze)*.

- [`PROJECT.md`](./PROJECT.md) — Scope, engineering budgets, and phased boundaries
- [`docs/ROADMAP.md`](./docs/ROADMAP.md) — SemVer roadmap and Storypole ticket map
- [`docs/SPEC-ARCHITECTURE.md`](./docs/SPEC-ARCHITECTURE.md) — Crate DAG, `Storage` traits, ring buffer, SQLite WAL + rollup schema
- [`docs/SPEC-PRIVACY.md`](./docs/SPEC-PRIVACY.md) — Salt rotation, sessions, bots, GeoIP, cardinality protection
- [`docs/SPEC-API.md`](./docs/SPEC-API.md) — `/v1/*` HTTP contract and Astro/Cloudflare SSR integration
- [`docs/SPEC-INTERFACES.md`](./docs/SPEC-INTERFACES.md) — CLI, MCP tools, and TUI specification
- [`docs/SPEC-OPERATIONS.md`](./docs/SPEC-OPERATIONS.md) — Multi-arch builds, `systemd`, Litestream R2 backups, release gates
- [`docs/COMPARISON-FEATURES.md`](./docs/COMPARISON-FEATURES.md) & [`docs/COMPARISON-METRICS.md`](./docs/COMPARISON-METRICS.md) — Feature and metric comparisons
- [`CONTRIBUTING.md`](./CONTRIBUTING.md), [`AGENTS.md`](./AGENTS.md), and [`CHANGELOG.md`](./CHANGELOG.md)

## License

Dual-licensed under **MIT** ([`LICENSE-MIT`](./LICENSE-MIT)) or **Apache-2.0** ([`LICENSE-APACHE`](./LICENSE-APACHE)) (`MIT OR Apache-2.0`).
