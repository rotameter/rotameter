# Rotameter Roadmap

Rotameter ships in three deliberate phases so the core engine stays small, fast, and durable on a `1 vCPU / 1 GB RAM` VM before adding UI or distributed product-analytics complexity.

*See [`../PROJECT.md`](../PROJECT.md) for engineering budgets and non-goals.*

---

## Phase 1 — `v1.0` Headless Analytics Engine (`RM-1`)

Target: unattended production durability on `VM.Standard.E2.1.Micro` (`< 40 MB` idle RSS, `< 128 MB` peak RSS) with clean API, CLI, MCP, and TUI interfaces.

| Milestone / Ticket | Scope |
| --- | --- |
| `RM-2` | 7-crate Cargo workspace, `MIT OR Apache-2.0`, lints, `rotameter-core` purity check, CI |
| `RM-3`, `RM-4` | Pure-Rust domain types, URL/referrer/UTM normalization, bot/UA classifiers, `HMAC-SHA256` hashing (midnight grace), 30m session LRU, cardinality caps, `Storage` traits |
| `RM-5`, `RM-6` | Bundled SQLite WAL schema (`events`, `sessions`, `rollups_*`), interning, batched writer, query engine, realtime, retention pruner, `VACUUM INTO` backups |
| `RM-7`, `RM-8`, `RM-9` | `axum` `/v1/*` server, scoped hashed Bearer keys, rate limits, idempotent Admin REST API, preconfigured local `self_site` bootstrap (`--no-self-site`), `16,384`-event ring buffer, GeoIP, `SIGTERM` drain, query & realtime endpoints |
| `RM-10`, `RM-11` | `<= 1.5 KB` gzip `collect.js`, `1x1` `/v1/pixel.gif`, first-party Astro & Cloudflare Workers proxy + SSR `trackServerHit()` helper |
| `RM-12` .. `RM-15` | `rotameter-client`, unified feature-gated CLI, `stdio` read-only MCP server, interactive `ratatui` TUI (ASCII block chart + drill-down) |
| `RM-16`, `RM-17` | Multi-arch `musl`/macOS builds, OCI container, `systemd` hardening, Litestream R2 backups, `v1.0` memory/load/`SIGTERM`/rollover/cardinality release gates |

---

## Phase 2 — `v1.5` Decoupled Web UI, Tag Managers & Remote MCP (Parked)

- `RM-18`: Optional static Web dashboard UI (`--ui=on|off`, default `off`) over `/v1/*`, dogfooding into the local `self_site` when enabled, plus multi-user accounts/OIDC/RBAC.
- `RM-19`: Packaged Google Tag Manager (`.tpl` + sGTM) and Cloudflare Zaraz integrations.
- `RM-20`: Optional Streamable HTTP `/mcp` server endpoint and opt-in `--allow-admin` MCP tools.

---

## Phase 3 — `v2.0` Funnels, User Flows & Multi-Backend Scale (Parked)

- `RM-21`: Multi-step conversion funnels, user flows / transition graphs, retention cohorts, engaged-time & scroll-depth heartbeats, revenue attribution, Google Search Console sync, and compound `AND`/`OR`/`NOT` + regex filter AST.
- `RM-22`: Pluggable `Storage` adapters for PostgreSQL, ClickHouse, DuckDB, Cloudflare Analytics Engine / D1, and MySQL / MariaDB / PlanetScale.
