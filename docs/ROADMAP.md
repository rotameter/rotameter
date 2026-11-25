# Rotameter Roadmap

Rotameter follows Rust/Cargo SemVer conventions (`0.x` minor milestones keep single-VM simplicity first while locking in 5-to-10-year seams ahead of `1.0.0`, while the HTTP `/v1/*` API version stays decoupled):

*See [`../PROJECT.md`](../PROJECT.md) for engineering budgets and [`decisions/0008-oltp-olap-split-arrow-and-multinode-seams.md`](./decisions/0008-oltp-olap-split-arrow-and-multinode-seams.md) (`ADR-0008`) for storage, Apache Arrow, and multi-node design.*

---

## Release Progression (`0.1.0` → `1.0.0` → `2.0.0`)

| Version | Milestone | Summary |
| --- | --- | --- |
| **`0.1.0`** *(target)* | **Headless Analytics Engine** | All `RM-1` child tickets (`RM-2`..`RM-17`, `RM-23`, `RM-25`, `RM-26`, `RM-27`) plus pre-release polish before live soak |
| **`0.1.x`** | **Live VM Soak Fixes** | Non-breaking bugfixes and tuning on `1 vCPU / 1 GB RAM` |
| **`0.2.0`** | **Storage Architecture & Arrow Portability** | OLTP/OLAP async trait split, multi-node seams (`SaltProvider` DB persistence, `--role`), `rotameter-arrow` portable Parquet/JSON `backup`/`restore`/`migrate`, opt-in **DuckDB** (`RM-28`) |
| **`0.3.0`** | **Initial Web UI & Self-Hoster Essentials** | Optional static Web UI (`--ui=on\|off`) covering `0.1.0`–`0.2.0`, simple admin/OIDC auth (`RM-18`), Streamable HTTP `/mcp` (`RM-20`) |
| **`0.4.0`** | **Product Analytics & Web UI v2** | Funnels, user flows/streams, cohorts, compound AST, opt-in privacy-safe click/scroll heatmaps & engaged-time module, Web UI builders (`RM-21`) |
| **`0.5.0`** | **Multi-Node Scale, Tier 2 DBs & Sinks** | Shared SQL AST (`PostgreSQL`, `MySQL`/`MariaDB` InnoDB), `ClickHouse`, generic ADBC/Flight SQL (`Pinot`, BI/warehouses) (`RM-22`); webhooks, Arrow/Parquet sinks & WASM/plugin seams (`RM-24`) |
| **`0.6.0`** | **Enterprise Multi-Tenancy, Tags & Tier 3** | Full multi-org RBAC, active-active `SessionStateStore`, Tier 3 adapters on demand (`MongoDB`, CF `D1`/AE, `DataFusion` lakehouse) (`RM-29`), packaged GTM/Zaraz templates (`RM-19`) |
| **`1.0.0`** | **Stable GA Freeze** | Long-term SemVer lock for `/v1/*`, `Storage` traits, Arrow canonical schema v1, migration lineage, and CLI/MCP |
| **`2.0.0`** | **Distributed & Multi-Region Horizon** | Stateless edge collectors → partitioned Arrow IPC stream (Kafka/NATS/R2) → stateful worker federation and v2 evolutions |

---

## `0.1.0` — Headless Engine, SQLite WAL, API, CLI, MCP & TUI (`RM-1`)

| Milestone / Ticket | Scope |
| --- | --- |
| `RM-2` .. `RM-6`, `RM-23` *(Verified)* | Workspace scaffold, `rotameter-core` (TypeID UIDs, normalizers, `HMAC-SHA256`, 30m session LRU, cardinality guard, `Storage`/`SaltProvider` traits), and `rotameter-store-sqlite` WAL writer, daily rollups (`0002_query_rollups`), query engine, realtime, and retention pruner |
| `RM-7` .. `RM-9`, `RM-25` *(Verified)* | `axum` `/v1/*` server, scoped Bearer auth (`rm_ing_`/`rm_rd_`/`rm_adm_`), bounded `16,384`-event MPSC ingest + `SIGTERM` drain, `/v1/query` (JSON + CSV), `/v1/sites/{site}/realtime`, `/v1/collect.js` (`<= 1,536 B` gzip), `/v1/pixel.gif`, per-IP rate limiting, local `self_site` bootstrap, `/v1/events/batch`, and `.mmdb` GeoIP |
| `RM-10`, `RM-11`, `RM-27` | Automated collector size/DOM harness and first-party Astro + Cloudflare Workers proxy (`@rotameter/astro-cloudflare`) & SSR `trackServerHit()` integration |
| `RM-12` .. `RM-15`, `RM-26` *(Verified)* | Typed `rotameter-client` (`config.toml` + CF Access headers), unified `rotameter` CLI (`serve`, `site`, `key`, `query`, `realtime`, `import`, `seed`, `backup`, `doctor`), read-only `stdio` MCP (`rotameter-mcp`), and interactive `ratatui` TUI (`rotameter-tui`) |
| `RM-16`, `RM-17` *(Verified)* | Multi-arch `musl`/macOS release builds, OCI image, `systemd` unit, Litestream R2 restore drill, and `1M`-event (`p95 < 25 ms`) / `< 40 MB` idle RSS gates |

---

## `0.2.0` — Future-Proof Storage Seams, Arrow Portability & DuckDB (`RM-28`)

- Decouple `ControlPlaneStore` (OLTP) from `AnalyticsDataPlane` (`EventIngestStore` + `AnalyticsQueryStore`, OLAP) with a single-DB default and runtime-agnostic `std::future::Future` traits (zero `tokio` in `rotameter-core`).
- Lock in multi-node seams with zero Redis/Kafka requirement: DB-backed `SaltProvider` in `ControlPlaneStore`, `--role=all|ingest|query`, and raw-append capability flags.
- Add feature-gated `rotameter-arrow` canonical schemas (`events`, `sessions`, `rollups`, `QueryResult`) and dual-layer portable Parquet/JSON `backup`, `restore`, and cross-DB `migrate`.
- Ship opt-in `rotameter-store-duckdb` to validate the OLTP/OLAP split and zero-copy Arrow ingestion while leaving the default SQLite binary untouched (`< 25 MB`, `< 40 MB` idle RSS).

---

## `0.3.0` — Initial Web UI & Self-Hoster Essentials (`RM-18`, `RM-20`)

- Optional static Web dashboard UI served behind `--ui=on|off` (default `off`) over `/v1/*` covering `0.1.0`–`0.2.0` capabilities (overview, realtime, breakdowns, Parquet/CSV export, site/key admin).
- Simple self-hoster admin authentication and OIDC/SSO (full multi-org RBAC deferred to `0.6.0`).
- Optional Streamable HTTP `/mcp` server endpoint and opt-in `--allow-admin` MCP tools (`RM-20`).

---

## `0.4.0` — Product Analytics, Advanced Tracking & Web UI v2 (`RM-21`)

- Multi-step conversion funnels (ordered & unordered), user flows/streams, retention cohorts, revenue/goal attribution, Google Search Console sync, and compound boolean filter AST (`AND`/`OR`/`NOT` + regex).
- Opt-in privacy-safe click/scroll heatmaps and engaged-time heartbeats shipped as a separate collector extension module so base `collect.js` stays `<= 1,536 bytes` gzip.
- Major Web UI expansion: interactive event/goal definitions, funnel builders, path stream explorers, and heatmap overlays.

---

## `0.5.0` — Multi-Node Scale, Tier 2 DBs, Plugins & Arrow Sinks (`RM-22`, `RM-24`)

- **Shared SQL AST (`rotameter-store-sql` via `sea-query` + `sqlx`):** Multi-node **PostgreSQL** (monthly partitions + `COPY`) and **MySQL / MariaDB (InnoDB)** with per-dialect fast paths.
- **Columnar & Warehouse Adapters:** **ClickHouse** (`FORMAT ArrowStream` + `AggregatingMergeTree`) and generic **ADBC / Arrow Flight SQL** (Apache Pinot, Snowflake, BigQuery, Trino, BI tools).
- **Dual-Granularity Extensibility:** Lightweight hot-path webhooks (`BeforeIngest`/`AfterFlush`, Slack/Discord/`ntfy`, optional WASM) plus batched Arrow IPC/Parquet sinks (S3/R2/Iceberg) and out-of-process Arrow Flight/ADBC storage plugins.

---

## `0.6.0` — Enterprise Multi-Tenancy, Tag Managers & Tier 3 / Lakehouse (`RM-19`, `RM-29`)

- Full multi-organization RBAC across API and Web UI, plus optional Redis/Valkey `SessionStateStore` for active-active multi-node ingest without sticky routing.
- On-demand Tier 3 dedicated adapters (**MongoDB**, Cloudflare **D1/Analytics Engine**, optional **DataFusion** S3/Iceberg lakehouse query engine).
- Packaged Google Tag Manager (Web `.tpl` + sGTM) and Cloudflare Zaraz Managed Component templates (`RM-19`).

---

## `1.0.0` & `2.0.0` — GA Freeze & Distributed Horizon

- **`1.0.0` — Stable GA Freeze:** Lock SemVer backward compatibility across `/v1/*`, `Storage` traits, Arrow canonical schema v1, migration lineage, and CLI/MCP interfaces after production soak.
- **`2.0.0` — Distributed & Multi-Region Horizon:** Rung 4 stateless edge collectors → partitioned Arrow IPC streams (Kafka/NATS/R2) → stateful sessionizer worker federation and next-generation protocol evolutions.
