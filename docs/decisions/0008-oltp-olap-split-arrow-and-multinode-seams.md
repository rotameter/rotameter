---
status: accepted
date: 2026-11-24
decision-makers: Colin Wolff (owner)
---

# Split OLTP/OLAP storage, adopt feature-gated Arrow exchange, and establish multi-node seams

## Context and Problem Statement

Rotameter `0.1.0` ships with bundled SQLite WAL on a single `1 vCPU / 1 GB RAM` VM (`< 40 MB` idle RSS, `< 128 MB` peak RSS, `< 25 MB` static binary). Before investing in a Web UI (`0.3.0`), we must lock in storage, exchange, and scaling abstractions that span from a single-VM deployment to multi-node clusters, analytical warehouses (`DuckDB`, `ClickHouse`, `Pinot`, `PostgreSQL`, `MySQL`/`MariaDB` InnoDB, `MongoDB`), and data lakes without forcing heavyweight dependencies into the default binary or writing and maintaining seven separate bespoke query engines.

## Decision Drivers

* Preserve the `< 40 MB` idle RSS, `< 128 MB` peak RSS, and `< 25 MB` static single-binary budgets for default SQLite deployments.
* Keep `rotameter-core` pure Rust with zero `tokio`, `rusqlite`, `axum`, `arrow`, or OS I/O dependencies.
* Allow a single database default (SQLite, PostgreSQL, MySQL/MariaDB, MongoDB) while enabling operators to split Control Plane (OLTP) and Analytics Data Plane (OLAP) and migrate data across any combination at any time.
* Support broad OLTP/OLAP/lakehouse ecosystems without capping database-specific optimizations or duplicating query AST logic across backends.
* Provide a progressive `1 -> N` node scaling ladder that avoids Redis/Kafka prerequisites for small and medium deployments while preserving clean seams for active-active and stream-partitioned scale.

## Considered Options

* OLTP/OLAP trait split + feature-gated `rotameter-arrow` exchange + 3-layer adapter strategy + dual-layer backup/migrate + 4-rung multi-node ladder
* Single unified storage trait requiring every backend to serve both transactional control plane and analytical queries
* Mandatory Apache Arrow `RecordBatch` types directly inside `rotameter-core` traits
* Dedicated hand-written query engine and adapter crate per database

## Decision Outcome

Chosen option: "OLTP/OLAP trait split + feature-gated `rotameter-arrow` exchange + 3-layer adapter strategy + dual-layer backup/migrate + 4-rung multi-node ladder", because it keeps default single-VM SQLite deployments minimal while locking in zero-rewrite 5-to-10-year seams across storage engines, portable backups, plugins/sinks, and horizontal scaling.

1. **Control Plane (OLTP) vs. Analytics Data Plane (OLAP) Split:** Decouple `ControlPlaneStore` (orgs, sites, API keys, future users/RBAC, rotating salt state) from `AnalyticsDataPlane` (`EventIngestStore` + `AnalyticsQueryStore`). Backends capable of both default to one connection; operators can split them independently at any time. Storage traits evolve in `0.2.0` to return runtime-agnostic `std::future::Future`s (zero `tokio` dependency in `rotameter-core`; embedded SQLite/DuckDB execute via `spawn_blocking`).
2. **Feature-Gated `rotameter-arrow` Exchange Layer:** Define canonical Arrow/Parquet schemas for `events`, `sessions`, `rollups`, and `QueryResult` with zero-copy `WriteBatch <-> RecordBatch` boundary adapters inside an opt-in `rotameter-arrow` crate. Default SQLite builds exclude Arrow/Parquet/DuckDB dependencies cleanly.
3. **3-Layer Adapter Strategy ("3 shared engines -> 10+ DBs", zero optimization cap):**
   * *Layer 1 — Shared SQL AST (`rotameter-store-sql` via `sea-query` + `sqlx`):* Compiles filters, breakdowns, and pagination uniformly across SQLite, PostgreSQL, and MySQL/MariaDB (InnoDB), while preserving per-dialect fast paths (SQLite `WITHOUT ROWID` + covering indexes, Postgres monthly `RANGE (ts_ms)` partitions + `COPY`, MySQL InnoDB clustered primary keys).
   * *Layer 2 — Arrow / Parquet + DuckDB + ADBC / Flight SQL:* Zero-copy Arrow ingestion into `rotameter-store-duckdb`, ClickHouse (`FORMAT ArrowStream` + `AggregatingMergeTree`), and a generic ADBC / Arrow Flight SQL adapter covering Pinot, Snowflake, BigQuery, Trino, and BI tools.
   * *Layer 3 — Thin Dedicated Adapters:* Reserved strictly for non-SQL/non-Arrow engines (`MongoDB`, Cloudflare `D1`/Analytics Engine) and optional `DataFusion` S3/Iceberg lakehouse querying.
4. **Dual-Layer Backup, Restore & Cross-DB Migration:** Support a backend-agnostic portable archive (`manifest.json` + `control_plane.json` + partitioned `.parquet` tables) powering `rotameter backup --format portable`, `rotameter restore`, and cross-backend `rotameter migrate --from <dsn> --to <dsn>`, alongside native fast paths (SQLite `VACUUM INTO` + Litestream -> R2).
5. **Dual-Granularity Plugin & Sink Seam:** Keep low-latency hot-path hooks (`BeforeIngest`, `AfterFlush`, Slack/Discord/`ntfy` webhooks, optional sandboxed WASM) lightweight and typed/JSON, while batched analytical exports (S3/R2/Iceberg/warehouses) and out-of-process storage plugins/BI integrations consume Arrow IPC/Parquet and Arrow Flight SQL / ADBC.
6. **4-Rung Progressive Multi-Node Scaling Ladder & `0.2.0` Seams:**
   * *Rung 1 (Single node, single DB):* `0.1.0` SQLite or `0.2.0` SQLite/DuckDB with `InMemorySaltProvider` and local `SessionTracker`.
   * *Rung 2 (Stateless query replicas + sticky/dedicated ingest, zero Redis/Kafka):* `--role=all|ingest|query` separates horizontally scalable stateless read/UI replicas from consistent-hashed ingest writers, backed by `ControlPlaneStore` salt persistence (`SaltProvider`) so all nodes compute identical `visitor_id` hashes.
   * *Rung 3 (Active-active ingest without sticky routing):* Optional `SessionStateStore` (Redis/Valkey) for SQL OLAP or append-only raw event writes with engine-side session/rollup materialization (declared via `StorageCapabilities`).
   * *Rung 4 (`2.0.0` distributed stream):* Stateless edge collectors emit partitioned Arrow IPC batches over Kafka/NATS/R2 to stateful sessionizer workers.

### Consequences

* Good, because the `0.1.0` single-VM SQLite deployment retains `< 25 MB` static binary size and `< 40 MB` idle RSS with zero Arrow/DuckDB/network-DB bloat.
* Good, because `0.2.0` validates the OLTP/OLAP split, `rotameter-arrow`, portable Parquet migration, and embedded DuckDB *before* the Web UI (`0.3.0`) or Tier 2 multi-node databases (`0.5.0`) are built.
* Good, because each storage backend remains feature-gated in its own workspace crate rather than forcing a monolithic binary.
* Bad, because transitioning `ControlPlaneStore`/`EventIngestStore`/`AnalyticsQueryStore` to async traits in `0.2.0` requires a coordinated trait signature update across server and store crates prior to `1.0.0` freeze.
* Neutral, because Tier 3 backends (`MongoDB`, Cloudflare `D1`/AE, `DataFusion`) wait for `0.6.0` on-demand delivery without blocking self-hoster adoption in `0.3.0`–`0.4.0`.

### Confirmation

* `cargo tree -p rotameter-core --edges normal` continues to prove zero `tokio`, `arrow`, or DB driver crates in `rotameter-core`.
* `./scripts/check-memory-budget.sh` and release binary size checks confirm default `SQLite` builds remain `< 25 MB` static / `< 40 MB` idle RSS.
* `RM-28` (`0.2.0`) verifies the OLTP/OLAP trait split, portable Parquet/JSON round-trip `backup`/`restore`/`migrate` across SQLite and DuckDB, DB-backed `SaltProvider`, and `--role=ingest|query`.
* `RM-22`, `RM-24` (`0.5.0`), and `RM-29` (`0.6.0`) verify Tier 2/3 adapters, dual-granularity sinks, and multi-node topologies behind the `0.2.0` seams.

## Pros and Cons of the Options

### OLTP/OLAP split + feature-gated Arrow + 3-layer adapters + 4-rung ladder

* Good, because single-DB simplicity and multi-DB/multi-node scale coexist cleanly with zero premature Redis/Kafka burden.
* Bad, because maintaining clean feature gates and portable Arrow schema versioning adds modest surface area in `0.2.0`.

### Unified single-DB trait

* Bad, because OLAP engines (`DuckDB`, `ClickHouse`, `Pinot`, Parquet lakes) cannot efficiently host transactional org/site/RBAC state or shared rotating salts.

### Mandatory Arrow `RecordBatch` in `rotameter-core`

* Bad, because pulling `arrow-*` crates into `rotameter-core` violates binary/memory budgets for micro-VM SQLite builds and breaks pure-Rust minimal builds.

### Bespoke query engine per database

* Bad, because duplicating filter AST compilation, pagination, and metric SQL across 7+ crates invites semantic drift and stalls product velocity.

## More Information

* Storypole: `RM-28` (`0.2.0`), `RM-18`/`RM-20` (`0.3.0`), `RM-21` (`0.4.0`), `RM-22`/`RM-24` (`0.5.0`), `RM-19`/`RM-29` (`0.6.0`).
* Specs: [`../ROADMAP.md`](../ROADMAP.md), [`../SPEC-ARCHITECTURE.md`](../SPEC-ARCHITECTURE.md), [`../../PROJECT.md`](../../PROJECT.md).
* Related ADR: [`0002-sqlite-wal-behind-storage-traits.md`](./0002-sqlite-wal-behind-storage-traits.md).
