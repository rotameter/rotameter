# Rotameter Roadmap

Rotameter ships in deliberate phases so a lean live MVP (`v0.1`) reaches production on a `1 vCPU / 1 GB RAM` VM first, while full `v1.0` polish, UI, scale engines, and post-`v2.0` extensibility layer cleanly on top.

*See [`../PROJECT.md`](../PROJECT.md) for engineering budgets and non-goals.*

---

## Phase 0 — `v0.1` Live Headless MVP (`RM-7` .. `RM-9`)

Target: unattended working single binary (`rotameter serve`), `<script defer src="/v1/collect.js">`, `curl` admin provisioning, live ingestion behind Cloudflare header GeoIP, and `/v1/query` + `/v1/sites/{site}/realtime` JSON/CSV reads.

| Milestone / Ticket | Scope |
| --- | --- |
| `RM-2` .. `RM-6`, `RM-23` *(Verified)* | Workspace scaffold, `rotameter-core` (UIDs, normalizers, `HMAC-SHA256`, 30m session LRU, cardinality guard, traits), and `rotameter-store-sqlite` WAL writer, query engine, realtime, and retention pruner |
| `RM-7` *(MVP)* | Slim `axum` HTTP core, `/v1` error envelope, scoped SHA-256 Bearer auth (`rm_ing_`/`rm_rd_`/`rm_adm_`), `/v1/health`, `/v1/admin/sites/*`, `/v1/admin/keys/*`, and `ROTAMETER_BOOTSTRAP_ADMIN_KEY` |
| `RM-8` *(MVP)* | Slim ingest pipeline: bounded `16,384`-event MPSC channel + batched SQLite flusher, `POST /v1/events`, header GeoIP (`CF-IPCountry`, `CF-Region-Code`), `Sec-GPC`, origin checks, and `SIGTERM` drain |
| `RM-9` *(MVP)* | `POST /v1/query` (JSON + CSV), `GET /v1/sites/{site}/realtime`, `GET /v1/collect.js` (`<= 1,536 B` gzip), `GET /v1/pixel.gif`, and bare `rotameter serve` entrypoint in `rotameter-cli` |

---

## Phase 1 — `v1.0` Full Headless Analytics Engine (`RM-1`, Parked post-MVP)

Remaining polish deferred until after the live `v0.1` MVP ships:

- `RM-25`: Per-IP token-bucket rate limiter, idempotent local `self_site` first-boot provisioning (`--no-self-site`), `POST /v1/events/batch`, and local MaxMind `.mmdb` GeoIP fallback.
- `RM-10`: Automated `<= 1,536 B` gzip CI gate script and expanded collector edge-case harness.
- `RM-11`: Packaged first-party Astro & Cloudflare Workers proxy + SSR `trackServerHit()` integration.
- `RM-12`, `RM-13`: Typed `rotameter-client` (`config.toml` profiles) and full CLI (`site`, `key`, `query`, `realtime`, `import`, `backup`, `doctor`).
- `RM-14`, `RM-15`: `stdio` read-only MCP server (`rotameter-mcp`) and interactive `ratatui` TUI (`rotameter-tui`).
- `RM-16`, `RM-17`: Multi-arch `musl`/macOS release builds, OCI image, `systemd` unit, Litestream R2 backups, and `1M`-event / `< 40 MB` RSS release gates.

---

## Phase 2 — `v1.5` Decoupled Web UI, Tag Managers & Remote MCP (Parked)

- `RM-18`: Optional static Web dashboard UI (`--ui=on|off`, default `off`) over `/v1/*`, plus multi-user accounts/OIDC/RBAC.
- `RM-19`: Packaged Google Tag Manager (`.tpl` + sGTM) and Cloudflare Zaraz integrations.
- `RM-20`: Optional Streamable HTTP `/mcp` server endpoint and opt-in `--allow-admin` MCP tools.

---

## Phase 3 — `v2.0` Funnels, User Flows & Multi-Backend Scale (Parked)

- `RM-21`: Multi-step conversion funnels, user flows / transition graphs, retention cohorts, engaged-time & scroll-depth heartbeats, revenue attribution, Google Search Console sync, and compound filter AST.
- `RM-22`: Pluggable `Storage` adapters for PostgreSQL, ClickHouse, DuckDB, Cloudflare Analytics Engine / D1, and MySQL / MariaDB / PlanetScale.

---

## Phase 4 — Post-`v2.0` Plugins, Webhooks & Event Sinks (Parked)

- `RM-24`: Outbound webhooks and event sinks (Slack/Discord/`ntfy` alerts, scheduled digests, S3/R2/warehouse exports) plus optional feature-gated sandboxed WASM transforms (`BeforeIngest`/`AfterFlush`) with zero baseline RSS regression.
