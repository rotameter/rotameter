---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Back v1.0 with bundled SQLite WAL behind pluggable Storage traits and an event-stream-first schema

## Context and Problem Statement

Rotameter targets unattended durability inside `< 40 MB` idle RSS (`< 128 MB` peak) on a `1 vCPU / 1 GB RAM` Oracle `VM.Standard.E2.1.Micro` instance, while also planning a clean path to PostgreSQL, ClickHouse, DuckDB, Cloudflare Analytics Engine/D1, and MySQL/PlanetScale in `v2.0+`—as well as multi-step funnels and user flows (`v2.0`) that require ordered per-session event histories. We had to choose both the `v1.0` storage engine and how deeply the engine couples to it.

## Decision Drivers

* Fits cleanly inside a `1 GB` VM alongside `cloudflared` and OS processes without OOM risk (`< 40 MB` idle / `< 128 MB` peak RSS).
* Zero separate database server or JVM/ClickHouse ops burden in `v1.0`.
* Crash-safe ACID ingestion capable of handling burst writes without blocking HTTP `202` responses.
* Must preserve ordered per-session event sequences (`seq_in_session`) so `v2.0` funnels and user flows run on historical `v1.0` data with zero rewrite.
* Must not hard-wire SQLite semantics into `rotameter-core` or query/ingest handlers so external databases can plug in later.

## Considered Options

* Bundled SQLite WAL (`rotameter-store-sqlite`) behind `Storage` traits, storing interned ordered `events`, `sessions`, and `rollups_*`
* SQLite storing counter rollups only (GoatCounter-style), deferring raw event streams
* Hand-rolled custom Rust append-only columnar TSDB crate (`rotameter-tsdb`) from day 1
* Embedded DuckDB or external ClickHouse / PostgreSQL in `v1.0`

## Decision Outcome

Chosen option: "Bundled SQLite WAL behind `Storage` traits, storing interned ordered `events`, `sessions`, and `rollups_*`", because it delivers battle-tested ACID recovery, streaming R2 backups via Litestream / `VACUUM INTO`, single-digit millisecond covering-index queries, and `v2.0`-ready event streams within `< 128 MB` peak RSS while keeping every future backend cleanly decoupled behind trait boundaries.

### Consequences

* Good, because single-node deployments have zero external DB containers or host `libsqlite3` version drift (`bundled` SQLite).
* Good, because `events(site_id, session_id, seq_in_session, ts_ms)` and `sessions` with covering indexes support both sub-25ms `v1.0` breakdowns and `v2.0` funnels/flows.
* Good, because `ControlPlaneStore`, `EventIngestStore`, `AnalyticsQueryStore`, and `StorageCapabilities` isolate `rotameter-core` and `rotameter-server` from SQLite specifics.
* Bad, because `v1.0` writes are single-node only (addressed via a single batched writer + bounded `16,384`-event ring buffer) until `v2.0` multi-node backends land.
* Neutral, because `dim_strings` integer interning and `WITHOUT ROWID` tables require forward migrations rather than schemaless JSON dumps.

### Confirmation

* `rotameter-core` defines `ControlPlaneStore`, `EventIngestStore`, `AnalyticsQueryStore`, and `StorageCapabilities` with zero `rusqlite` dependency.
* `RM-5` and `RM-6` verify WAL PRAGMAs (`cache_size = -16000`, `mmap_size = 67108864`), `BEGIN IMMEDIATE` batch writes, covering-index queries, and `VACUUM INTO` backups.
* `RM-17` proves `< 40 MB` idle / `< 128 MB` peak RSS and `p95 < 25 ms` 30d queries across 1M seeded events.

## Pros and Cons of the Options

### Bundled SQLite WAL behind `Storage` traits (event-stream + rollups)

* Good, because ACID crash safety, `VACUUM INTO`, and Litestream -> R2 streaming are proven in production.
* Good, because integer-interned `WITHOUT ROWID` rows keep ~1M events to ~60–75 MB on disk.
* Bad, because very high multi-hundred-million-event/year single-node volumes will eventually want ClickHouse/DuckDB (`v2.0` adapters).

### Counter rollups only (no raw `events` stream)

* Good, because storage footprint is tiny.
* Bad, because arbitrary multi-day filter combinations, user flows, and conversion funnels become impossible over historical data.

### Custom Rust columnar TSDB (`rotameter-tsdb`) in `v1.0`

* Good, because columnar storage excels at cold 100M+ row scans.
* Bad, because building crash-safe WAL recovery, compaction, corruption fuzzing, and backup tooling delays `v1.0` by weeks and risks silent data loss.

### DuckDB / ClickHouse / PostgreSQL in `v1.0`

* Bad, because ClickHouse/DuckDB blow past a `1 GB` micro VM's safe RAM headroom under analytical queries or concurrent ingest.

## More Information

* Storypole: `RM-4`, `RM-5`, `RM-6`, `RM-17`, `RM-21`, `RM-22`.
* Spec: [`../SPEC-ARCHITECTURE.md`](../SPEC-ARCHITECTURE.md).
