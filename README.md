# Rotameter

A single-binary, privacy-first, headless web analytics engine in Rust (`MIT OR Apache-2.0`), engineered to run comfortably on a `1 vCPU / 1 GB RAM` virtual machine (`< 40 MB` idle RSS, `< 128 MB` peak RSS) with zero cookies, zero persisted raw IPs, and zero ClickHouse/PostgreSQL overhead.

> **Status:** Specification & Storypole backlog complete (`v1.0` implementation starts at `RM-2`).

---

## Highlights (`v1.0` Target)

- **Headless-first engine:** Clean-sheet `/v1/*` JSON HTTP API (`POST /v1/events`, `POST /v1/events/batch`, `POST /v1/query`, `GET /v1/sites/{site}/realtime`, idempotent `/v1/admin/*`).
- **Multiple collection paths:** `<= 1.5 KB` gzip `collect.js`, zero-JS `1x1` `/v1/pixel.gif`, and first-party **Astro / Cloudflare Workers SSR** proxy & `waitUntil()` helper.
- **Privacy & session accuracy:** Cookieless `HMAC-SHA256` daily CSPRNG salt (+ 30m midnight UTC grace window, per-site `daily`/`weekly`/`monthly` opt-in), 30m sliding session metrics (visits, bounce rate, visit duration, entry/exit), cardinality caps into `(other)`, and local-only preconfigured self-site dogfooding (zero phone-home).
- **Event-stream SQLite WAL:** Integer-interned ordered `events` + `sessions` + `rollups_*` behind pluggable `Storage` traits—ready for `v2.0` funnels & user flows without schema rewrites.
- **Operator interfaces:** Unified `rotameter` CLI, strictly read-only `stdio` MCP server (`rotameter mcp`), and interactive `ratatui` TUI (`rotameter tui`) with cursor-scrubbing ASCII block chart.

---

## Planned CLI Surface

| Command | Summary |
| --- | --- |
| `rotameter serve` | Run the HTTP ingest/query/admin daemon (`--bind`, `--db`, `--public-url`, `--no-self-site`, `--self-site-id`, `--geoip`, `--mmdb-path`, `--trusted-proxy`) |
| `rotameter site` | `upsert`, `list`, `show`, `purge`, `delete` |
| `rotameter key` | `create`, `list`, `revoke` (`ingest`, `read`, `admin` scopes) |
| `rotameter query` | Query summary, timeseries, and breakdowns (`--range`, `--group-by`, `--filter`, `--compare`, `--format table\|json\|csv`) |
| `rotameter realtime` | Stream 5-minute active visitors and top active pages (`--watch`) |
| `rotameter import` | Backfill JSONL or Nginx/Caddy CLF logs (`--dry-run`) |
| `rotameter backup` | Online SQLite `VACUUM INTO` snapshot |
| `rotameter doctor` | Verify local DB WAL/pragmas or remote `/v1/health` + Access credentials |
| `rotameter mcp` | Start `stdio` read-only Model Context Protocol server |
| `rotameter tui` | Launch interactive `ratatui` flight-instrument dashboard |

---

## Documentation

- [`PROJECT.md`](./PROJECT.md) — Scope, engineering budgets, and phased `v1.0` / `v1.5` / `v2.0` boundaries
- [`docs/ROADMAP.md`](./docs/ROADMAP.md) — Release roadmap and Storypole ticket map
- [`docs/SPEC-ARCHITECTURE.md`](./docs/SPEC-ARCHITECTURE.md) — Workspace DAG, `Storage` traits, ring buffer, SQLite schema
- [`docs/SPEC-PRIVACY.md`](./docs/SPEC-PRIVACY.md) — Salt rotation, sessions, bots, GeoIP, cardinality protection
- [`docs/SPEC-API.md`](./docs/SPEC-API.md) — `/v1/*` wire contract and Astro/Cloudflare integration
- [`docs/SPEC-INTERFACES.md`](./docs/SPEC-INTERFACES.md) — CLI, MCP tools, and TUI layout
- [`docs/SPEC-OPERATIONS.md`](./docs/SPEC-OPERATIONS.md) — Multi-arch builds, `systemd`, Litestream R2 backups, release gates
- [`docs/COMPARISON-FEATURES.md`](./docs/COMPARISON-FEATURES.md) & [`docs/COMPARISON-METRICS.md`](./docs/COMPARISON-METRICS.md) — Benchmarks against GoatCounter, Plausible CE, Umami, Matomo, and GA4
- [`AGENTS.md`](./AGENTS.md) & [`CONTRIBUTING.md`](./CONTRIBUTING.md) — Engineering contract and workflow

## License

Dual-licensed under **MIT** or **Apache-2.0** (`MIT OR Apache-2.0`).
