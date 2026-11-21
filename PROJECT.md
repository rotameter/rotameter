# PROJECT.md — Rotameter

## 1. Executive Summary & Purpose

**Rotameter** is a single-binary, privacy-first, headless web analytics engine written in Rust (`MIT OR Apache-2.0`). It is engineered from a clean sheet to deliver modern web analytics—real session metrics, custom events, clean breakdown queries, CLI automation, an MCP server for AI agents, and a flight-instrument TUI—inside the footprint of a single **`1 vCPU / 1 GB RAM`** virtual machine (`VM.Standard.E2.1.Micro`).

It fills the structural gap between ultra-lightweight page counters (which run in `< 40 MB` RAM without session metrics or structured query APIs) and heavyweight product analytics suites (which require 2–4 GB+ RAM for ClickHouse, PostgreSQL, or Node/Elixir stacks). Rotameter invents its own clean `/v1/*` contract rather than inheriting legacy wire protocols, while keeping an event-stream-first storage layout ready to support a decoupled Web UI (`v1.5`) and multi-step conversion funnels & user flows (`v2.0`) without schema rewrites.

---

## 2. Core Philosophy & Hard Engineering Budgets

| Budget / Law | Target (`v1.0`) | Rationale |
| --- | --- | --- |
| **Idle memory (RSS)** | `< 40 MB` | Coexist comfortably with `cloudflared`, Litestream, and OS services on a `1 GB` VM. |
| **Peak memory (RSS)** | `< 128 MB` | Bounded ring buffer + bounded session LRU + SQLite cache caps prevent OOM under traffic spikes or query storms. |
| **Static single binary** | `< 25 MB` (`musl` stripped) | Zero runtime dependencies (`glibc`-independent Linux binaries for `amd64` and `arm64`, plus macOS `arm64`/`amd64`). |
| **Client collector (`collect.js`)** | `< 1.5 KB` gzip | Sub-kilobyte footprint; zero cookies, zero `localStorage` tracking, zero dependencies. |
| **Privacy invariant** | Zero PII on disk | Daily CSPRNG salt (`HMAC-SHA256`), zero persistent raw IPs in DB or access logs. |
| **Storage abstraction** | Pluggable `Storage` traits | SQLite WAL ships in `v1.0`; Postgres, ClickHouse, DuckDB, Cloudflare Analytics Engine/D1, and MySQL/PlanetScale plug in cleanly later. |
| **Core purity** | `rotameter-core` is OS/async-free | Zero `tokio`, zero `rusqlite`, zero filesystem calls inside `rotameter-core` for fast property testing and future backend/WASM portability. |
| **Safety** | `#![forbid(unsafe_code)]` | Enforced across application crates; zero `.unwrap()` / `.expect()` in non-test paths. |
| **License** | `MIT OR Apache-2.0` | Standard permissive dual license across the Rust ecosystem. |

---

## 3. Phased Roadmap & Explicit Boundaries

### `v1.0` — Headless Engine, API, CLI, MCP & TUI (Current Target)
- **Engine & Storage:** Bounded async ingest ring-buffer + batched SQLite WAL writer, graceful `SIGTERM` drain, interned dimension dictionary, ordered per-session event log + hourly/daily rollups.
- **Privacy & Accuracy:** Cookieless daily CSPRNG salt with 30-minute midnight UTC grace window (configurable `daily` | `weekly` | `monthly` per site), 30-minute sliding session tracker (visits, bounce rate, visit duration, entry/exit), compiled-in bot matcher, `CF-IPCountry` 0 MB GeoIP + optional `.mmdb`, UTM extraction, URL normalization & cardinality bomb protection, optional `Sec-GPC` handling, `rotameter_ignore` opt-out.
- **Collection (`/v1/*`):** `GET /v1/collect.js`, `GET /v1/pixel.gif`, `POST /v1/events`, `POST /v1/events/batch`, and first-party **Astro / Cloudflare Workers SSR integration** (`waitUntil()` proxy + server-side hit helper).
- **Auth & Control Plane:** Multi-site tenancy, scoped hashed API keys (`ingest`, `read`, `admin`), idempotent Admin HTTP API (IaC/OpenTofu-friendly), `rotameter` CLI (`serve`, `site`, `key`, `query`, `realtime`, `import`, `backup`, `doctor`, `mcp`, `tui`).
- **Interfaces:** `stdio` read-only MCP server (`rotameter mcp`) and interactive `ratatui` TUI (`rotameter tui`) with cursor-scrubbing ASCII block chart & `Enter`-to-filter drill-down.
- **Operations:** Multi-arch static `musl` & macOS builds, OCI container images, `systemd` hardening, Cloudflare Tunnel + Access integration, `VACUUM INTO` backups + Litestream → R2 replication.

**Explicit `v1.0` Non-Goals:**
- No built-in Web dashboard UI (`v1.5`).
- No user accounts, passwords, OIDC, or team RBAC (`v1.5`).
- No Google Tag Manager (`.tpl`/sGTM) or Cloudflare Zaraz templates (`v1.5`).
- No legacy Plausible/GoatCounter/Matomo wire-API compatibility shims.
- No multi-step conversion funnels, Sankey/path user flows, or retention cohorts (`v2.0`).
- No distributed multi-node clustering or custom hand-rolled TSDB storage engine (`Storage` traits carry SQLite WAL in `v1.0`).

### `v1.5` — Decoupled Web UI, Tag Managers & Remote MCP
- Optional static Web UI served behind `--ui=on|off` (default `off`), talking strictly to the `/v1/*` API.
- Multi-user accounts, session auth, OIDC/SSO, and role-based site permissions.
- Packaged Google Tag Manager (Web `.tpl` + Server-Side sGTM) and Cloudflare Zaraz Managed Component.
- Optional Streamable HTTP `/mcp` server endpoint and opt-in `--allow-admin` MCP write tools.

### `v2.0` — Product Analytics & Multi-Backend Scale
- Multi-step conversion funnels (ordered & unordered, per-session window, drop-off inspection) built directly over the `v1.0` ordered `events(site_id, session_id, seq_in_session, ...)` stream.
- User flows / page transition graphs, retention cohorts, engaged-time & scroll-depth heartbeats, revenue/goal attribution, and Google Search Console sync.
- Arbitrary compound boolean filter ASTs (`AND` / `OR` / `NOT`, regex matchers) and dynamic query planning.
- Additional `Storage` crate backends: PostgreSQL, ClickHouse, DuckDB, Cloudflare Analytics Engine / D1, MySQL / MariaDB / PlanetScale, and an optional custom columnar segment store if single-node SQLite limits are ever reached.

---

## 4. Specification Index

| Document | Scope |
| --- | --- |
| [`AGENTS.md`](./AGENTS.md) | Engineering contract, Storypole discipline, Rust/size/memory gates |
| [`docs/COMPARISON-FEATURES.md`](./docs/COMPARISON-FEATURES.md) | Feature-by-feature matrix vs. GoatCounter, Plausible CE, Umami, Matomo, GA4 |
| [`docs/COMPARISON-METRICS.md`](./docs/COMPARISON-METRICS.md) | Metric-by-metric matrix and exact Rotameter mathematical definitions |
| [`docs/SPEC-ARCHITECTURE.md`](./docs/SPEC-ARCHITECTURE.md) | Workspace crates, `Storage` traits, ring-buffer pipeline, SQLite WAL schema |
| [`docs/SPEC-PRIVACY.md`](./docs/SPEC-PRIVACY.md) | Hashing, salt windows, session state machine, bots, GeoIP, cardinality caps |
| [`docs/SPEC-API.md`](./docs/SPEC-API.md) | `/v1/*` HTTP contract, auth scopes, `collect.js`, `pixel.gif`, Astro/CF Workers SSR |
| [`docs/SPEC-INTERFACES.md`](./docs/SPEC-INTERFACES.md) | CLI commands, `stdio` MCP tool schemas, `ratatui` TUI anatomy & keybindings |
| [`docs/SPEC-OPERATIONS.md`](./docs/SPEC-OPERATIONS.md) | Multi-arch builds, E2.1.Micro `systemd`, Tunnel/Access, Litestream/R2, benchmarks |
| [`docs/decisions/README.md`](./docs/decisions/README.md) | MADR 4.0 architectural decision index |
