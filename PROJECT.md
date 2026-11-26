# PROJECT.md — Rotameter

## 1. Executive Summary & Purpose

**Rotameter** is a single-binary, privacy-first, headless web analytics engine written in Rust (`MIT OR Apache-2.0`). It is engineered from a clean sheet to deliver modern web analytics—real session metrics, custom events, clean breakdown queries, CLI automation, an MCP server for AI agents, and a flight-instrument TUI—inside the footprint of a single **`1 vCPU / 1 GB RAM`** virtual machine (`VM.Standard.E2.1.Micro`).

It fills the structural gap between ultra-lightweight page counters (which run in `< 40 MB` RAM without session metrics or structured query APIs) and heavyweight product analytics suites (which require 2–4 GB+ RAM for ClickHouse, PostgreSQL, or Node/Elixir stacks). Rotameter invents its own clean `/v1/*` contract rather than inheriting legacy wire protocols, while keeping an event-stream-first storage layout and `ADR-0008` OLTP/OLAP + Apache Arrow seams ready to scale from a single VM to multi-node clusters and data lakes without schema rewrites.

---

## 2. Core Philosophy & Hard Engineering Budgets

| Budget / Law | Target (`0.1.0`–`1.0.0`) | Rationale |
| --- | --- | --- |
| **Idle memory (RSS)** | `< 40 MB` | Coexist comfortably with `cloudflared`, Litestream, and OS services on a `1 GB` VM. |
| **Peak memory (RSS)** | `< 128 MB` | Bounded ring buffer + bounded session LRU + SQLite cache caps prevent OOM under traffic spikes or query storms. |
| **Static single binary** | `< 25 MB` (`musl` stripped) | Zero runtime dependencies (`glibc`-independent Linux binaries for `amd64` and `arm64`, plus macOS `arm64`/`amd64`). |
| **Client collector (`collect.js`)** | `< 1.5 KB` gzip | Sub-kilobyte footprint; zero cookies, zero `localStorage` tracking, zero dependencies (optional extensions stay separate). |
| **Privacy invariant** | Zero PII on disk | Daily CSPRNG salt (`HMAC-SHA256`), zero persistent raw IPs in DB or access logs. |
| **Storage abstraction** | OLTP/OLAP split + Arrow seams | SQLite WAL ships in `0.1.0`; `0.2.0` locks in OLTP/OLAP split, feature-gated `rotameter-arrow`, & DuckDB; `0.5.0`–`0.6.0` add SQL AST (`PG`/`MySQL`), ClickHouse, ADBC/Flight, MongoDB, CF D1/AE, & DataFusion (`ADR-0008`). |
| **Core purity** | `rotameter-core` is OS/async-free | Zero `tokio`, `rusqlite`, `arrow`, or filesystem calls inside `rotameter-core` for fast property testing and backend portability. |
| **Safety** | `#![forbid(unsafe_code)]` | Enforced across application crates; zero `.unwrap()` / `.expect()` in non-test paths. |
| **License** | `MIT OR Apache-2.0` | Standard permissive dual license across the Rust ecosystem. |

---

## 3. Phased Roadmap & Explicit Boundaries

Following Rust/Cargo SemVer practices, pre-GA feature phases advance through `0.1.0` → `0.6.0` before locking long-term stability at `1.0.0` (while keeping HTTP routes on `/v1/*`) and evolving distributed federation in `2.0.0`.

### `0.1.0` — Headless Engine, API, CLI, MCP & TUI (Current Target)
- **Engine & Storage:** Bounded async ingest ring-buffer + batched SQLite WAL writer, graceful `SIGTERM` drain, interned dimension dictionary, ordered per-session event log + hourly/daily/path rollups.
- **Privacy & Accuracy:** Cookieless daily CSPRNG salt with 30-minute midnight UTC grace window (configurable `daily` | `weekly` | `monthly` per site), 30-minute sliding session tracker (visits, bounce rate, visit duration, entry/exit), compiled-in bot matcher, `CF-IPCountry` 0 MB GeoIP + optional `.mmdb`, UTM extraction, URL normalization & cardinality bomb protection, optional `Sec-GPC` handling, `rotameter_ignore` opt-out.
- **Collection (`/v1/*`):** `GET /v1/collect.js`, `GET /v1/pixel.gif`, `POST /v1/events`, `POST /v1/events/batch`, and first-party **Astro / Cloudflare Workers SSR integration** (`waitUntil()` proxy + server-side hit helper).
- **Auth & Control Plane:** Multi-site tenancy, scoped hashed API keys (`ingest`, `read`, `admin`), idempotent Admin HTTP API (IaC/OpenTofu-friendly), `rotameter` CLI (`serve`, `site`, `key`, `query`, `realtime`, `import`, `seed`, `backup`, `doctor`, `mcp`, `tui`).
- **Interfaces:** `stdio` read-only MCP server (`rotameter mcp`) and interactive `ratatui` TUI (`rotameter tui`) with cursor-scrubbing ASCII block chart & `Enter`-to-filter drill-down.
- **Operations:** Multi-arch static `musl` & macOS builds, OCI container images, `systemd` hardening, Cloudflare Tunnel + Access integration, `VACUUM INTO` backups + Litestream → R2 replication.

**Explicit `0.1.0` Non-Goals:**
- No built-in Web dashboard UI (`0.3.0`), multi-user RBAC (`0.3.0`/`0.6.0`), or GTM/Zaraz templates (`0.6.0`).
- No legacy Plausible/GoatCounter/Matomo wire-API compatibility shims.
- No multi-step conversion funnels, user flows, cohorts, or heatmaps (`0.4.0`).
- No distributed multi-node clustering, Arrow/DuckDB/network DB adapters (`0.2.0`/`0.5.0`/`0.6.0`), or webhooks/WASM plugins (`0.5.0`).

### `0.2.0` — Storage Architecture, Arrow Portability & Embedded OLAP (`ADR-0008`)
- Split `ControlPlaneStore` (OLTP) and `AnalyticsDataPlane` (OLAP) with single-DB default and runtime-agnostic `std::future::Future` traits; add DB-backed `SaltProvider`, `--role=all|ingest|query`, and raw-append capability seams.
- Feature-gated `rotameter-arrow` canonical exchange schemas + dual-layer portable Parquet/JSON `backup`, `restore`, and cross-DB `migrate`, validated by opt-in `rotameter-store-duckdb` without inflating the default SQLite binary.

### `0.3.0` — Initial Web UI & Self-Hoster Essentials
- Optional static Web UI (`ui/` Bun workspaces, CAD/flight-instrument `tokens.css`, `@base-ui/react` + TanStack Router/Query/Table/Virtual + `uPlot`, Cargo `ui` feature behind `--ui=on|off` default `off` + `--ui-dir`) covering `0.1.0`–`0.2.0` features (`overview`, `breakdowns`, `realtime`, `admin`, single-node `cluster` health, CSV/Parquet export), `/v1/capabilities`, simple admin login / OIDC (`/v1/auth/*` + CF Access), and optional Streamable HTTP `/mcp` with opt-in `--allow-admin` tools (`docs/SPEC-UI.md`, `ADR-0009`).

### `0.4.0` — Product Analytics, Advanced Tracking & Web UI v2
- Multi-step conversion funnels, user flows/streams, retention cohorts, goal/revenue attribution, Google Search Console sync, and compound filter AST (`AND`/`OR`/`NOT` + regex).
- Opt-in privacy-safe click/scroll heatmaps and engaged-time heartbeats shipped as a separate collector extension (preserving base `collect.js <= 1.5 KB` gzip), paired with lazy-loaded `@rotameter/plugin-*` builders, heatmaps, funnels, and freeze-on-interact live event streams (`docs/SPEC-UI.md`).

### `0.5.0` — Multi-Node Scale, Tier 2 DBs, Plugins & Arrow Sinks
- Shared SQL AST (`rotameter-store-sql`: PostgreSQL, MySQL/MariaDB InnoDB), ClickHouse (`FORMAT ArrowStream` / `AggregatingMergeTree`), and generic ADBC / Arrow Flight SQL (Pinot, warehouses, BI), expanding `@rotameter/plugin-cluster` with multinode roster and replication lag views.
- Dual-granularity webhooks/WASM hooks (`BeforeIngest`/`AfterFlush`), batched Arrow IPC/Parquet sinks (S3/R2/Iceberg), out-of-process Arrow Flight/ADBC storage plugins, and Tier 2 third-party runtime UI plugin loader (`RM-24`, `docs/SPEC-UI.md`).

### `0.6.0` — Enterprise Multi-Tenancy, Tag Managers & Tier 3 / Lakehouse Backends
- Full multi-org RBAC, optional Redis/Valkey `SessionStateStore` for active-active multi-node ingest, on-demand Tier 3 adapters (MongoDB, Cloudflare D1/Analytics Engine, optional DataFusion S3/Iceberg lakehouse), and packaged GTM `.tpl`/sGTM & Cloudflare Zaraz templates.

### `1.0.0` & `2.0.0` — Stable GA Freeze & Distributed Horizon
- **`1.0.0`:** Long-term SemVer lock for `/v1/*`, `Storage` traits, Arrow canonical schema v1, migration lineage, and CLI/MCP interfaces.
- **`2.0.0`:** Stateless edge collectors → partitioned Arrow IPC streams (Kafka/NATS/R2) → stateful sessionizer worker federation.

---

## 4. Specification Index

| Document | Scope |
| --- | --- |
| [`AGENTS.md`](./AGENTS.md) | Engineering contract, Storypole discipline, Rust/size/memory/UI gates |
| [`docs/COMPARISON-FEATURES.md`](./docs/COMPARISON-FEATURES.md) | Feature-by-feature matrix vs. GoatCounter, Plausible CE, Umami, Matomo, GA4 |
| [`docs/COMPARISON-METRICS.md`](./docs/COMPARISON-METRICS.md) | Metric-by-metric matrix and exact Rotameter mathematical definitions |
| [`docs/SPEC-ARCHITECTURE.md`](./docs/SPEC-ARCHITECTURE.md) | Workspace crates, `Storage` traits, ring-buffer pipeline, SQLite WAL schema |
| [`docs/SPEC-PRIVACY.md`](./docs/SPEC-PRIVACY.md) | Hashing, salt windows, session state machine, bots, GeoIP, cardinality caps |
| [`docs/SPEC-API.md`](./docs/SPEC-API.md) | `/v1/*` HTTP contract, auth scopes, `collect.js`, `pixel.gif`, Astro/CF Workers SSR |
| [`docs/SPEC-INTERFACES.md`](./docs/SPEC-INTERFACES.md) | CLI commands, `stdio` MCP tool schemas, `ratatui` TUI anatomy & keybindings |
| [`docs/SPEC-UI.md`](./docs/SPEC-UI.md) | Web UI architecture (`0.3.0+`), `tokens.css`, WCAG 2.2 AA, Plugin SDK & topology contract |
| [`docs/SPEC-OPERATIONS.md`](./docs/SPEC-OPERATIONS.md) | Multi-arch builds, E2.1.Micro `systemd`, Tunnel/Access, Litestream/R2, benchmarks |
| [`docs/decisions/README.md`](./docs/decisions/README.md) | MADR 4.0 architectural decision index |
