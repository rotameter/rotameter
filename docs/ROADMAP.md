# Rotameter Roadmap

Rotameter follows Rust/Cargo SemVer conventions (`0.x` minor milestones isolate breaking changes across pre-`1.0` phases, while the HTTP `/v1/*` API version stays decoupled):

*See [`../PROJECT.md`](../PROJECT.md) for engineering budgets and non-goals.*

---

## Release Progression (`0.1.0` → `1.0.0`)

| Version | Milestone | Summary |
| --- | --- | --- |
| **`0.1.0`** *(target)* | **Headless Analytics Engine** | All `RM-1` child tickets (`RM-2`..`RM-17`, `RM-23`, `RM-25`, `RM-26`) plus pre-release polish before live soak |
| **`0.1.x`** | Live VM Soak Fixes | Non-breaking bugfixes and tuning on `1 vCPU / 1 GB RAM` |
| **`0.2.0`** | Web UI, Tags & Remote MCP | Optional static Web UI (`--ui=on\|off`), OIDC/RBAC, GTM/Zaraz, HTTP `/mcp` |
| **`0.3.0`** | Product Analytics & Scale | Funnels, user flows, cohorts, compound AST, multi-backend `Storage` |
| **`0.4.0`** | Plugins, Webhooks & Sinks | Outbound alerts/exports and optional sandboxed WASM transforms |
| **`1.0.0`** | Stable GA Freeze | Long-term stability lock once `/v1/*`, `Storage` traits, and CLI/MCP soak |

---

## `0.1.0` — Headless Engine, API, CLI, MCP & TUI (`RM-1`, Child Tickets Verified)

| Milestone / Ticket | Scope |
| --- | --- |
| `RM-2` .. `RM-6`, `RM-23` *(Verified)* | Workspace scaffold, `rotameter-core` (TypeID UIDs, normalizers, `HMAC-SHA256`, 30m session LRU, cardinality guard, `Storage`/`SaltProvider` traits), and `rotameter-store-sqlite` WAL writer, daily rollups (`0002_query_rollups`), query engine, realtime, and retention pruner |
| `RM-7` .. `RM-9`, `RM-25` *(Verified)* | `axum` `/v1/*` server, scoped Bearer auth (`rm_ing_`/`rm_rd_`/`rm_adm_`), bounded `16,384`-event MPSC ingest + `SIGTERM` drain, `/v1/query` (JSON + CSV), `/v1/sites/{site}/realtime`, `/v1/collect.js` (`<= 1,536 B` gzip), `/v1/pixel.gif`, per-IP rate limiting, local `self_site` bootstrap, `/v1/events/batch`, and `.mmdb` GeoIP |
| `RM-10`, `RM-11` *(Verified)* | Automated collector size/DOM harness and first-party Astro + Cloudflare Workers proxy & SSR `trackServerHit()` integration |
| `RM-12` .. `RM-15`, `RM-26` *(Verified)* | Typed `rotameter-client` (`config.toml` + CF Access headers), unified `rotameter` CLI (`serve`, `site`, `key`, `query`, `realtime`, `import`, `seed`, `backup`, `doctor`), read-only `stdio` MCP (`rotameter-mcp`), and interactive `ratatui` TUI (`rotameter-tui`) |
| `RM-16`, `RM-17` *(Verified)* | Multi-arch `musl`/macOS release builds, OCI image, `systemd` unit, Litestream R2 restore drill, and `1M`-event (`p95 < 25 ms`) / `< 40 MB` idle RSS gates |

---

## `0.2.0` — Decoupled Web UI, Tag Managers & Remote MCP (Planned)

- `RM-18`: Optional static Web dashboard UI (`--ui=on|off`, default `off`) over `/v1/*`, plus multi-user accounts/OIDC/RBAC.
- `RM-19`: Packaged Google Tag Manager (`.tpl` + sGTM) and Cloudflare Zaraz integrations.
- `RM-20`: Optional Streamable HTTP `/mcp` server endpoint and opt-in `--allow-admin` MCP tools.

---

## `0.3.0` — Funnels, User Flows & Multi-Backend Scale (Planned)

- `RM-21`: Multi-step conversion funnels, user flows / transition graphs, retention cohorts, engaged-time & scroll-depth heartbeats, revenue attribution, Google Search Console sync, and compound filter AST.
- `RM-22`: Pluggable `Storage` adapters for PostgreSQL, ClickHouse, DuckDB, Cloudflare Analytics Engine / D1, and MySQL / MariaDB / PlanetScale.

---

## `0.4.0` — Plugins, Webhooks & Event Sinks (Planned)

- `RM-24`: Outbound webhooks and event sinks (Slack/Discord/`ntfy` alerts, scheduled digests, S3/R2/warehouse exports) plus optional feature-gated sandboxed WASM transforms (`BeforeIngest`/`AfterFlush`) with zero baseline RSS regression.

---

## `1.0.0` — Stable GA Freeze

- Declared once `0.x` production soak completes across `/v1/*`, `Storage` traits, SQLite migration lineage, and CLI/MCP interfaces, locking backward compatibility under strict SemVer `1.0` guarantees.
