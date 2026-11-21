# Architecture Specification (`v1.0`)

**Rotameter** is structured as a modular Rust workspace that compiles into a single static binary (`rotameter`), keeping pure domain logic isolated from runtime I/O and storage behind explicit trait boundaries.

*Related specs: [`../PROJECT.md`](../PROJECT.md) · [`SPEC-PRIVACY.md`](./SPEC-PRIVACY.md) · [`SPEC-API.md`](./SPEC-API.md) · [`SPEC-OPERATIONS.md`](./SPEC-OPERATIONS.md)*

---

## 1. Workspace Crates & Dependency DAG

```text
                       ┌────────────────────┐
                       │   rotameter-cli    │  (feature-gated unified binary)
                       └──┬───┬────┬────┬───┘
            ┌─────────────┘   │    │    └──────────────┐
            ▼                 ▼    ▼                   ▼
┌─────────────────┐  ┌──────────┐ ┌───────────┐  ┌──────────────┐
│rotameter-server │  │rotameter │ │rotameter  │  │rotameter-    │
│    (axum)       │  │  -mcp    │ │  -tui     │  │store-sqlite  │
└────────┬────────┘  └────┬─────┘ └─────┬─────┘  └──────┬───────┘
         │                │             │               │
         │                └──────┬──────┘               │
         │                       ▼                      │
         │              ┌─────────────────┐             │
         │              │rotameter-client │             │
         │              └────────┬────────┘             │
         ▼                       ▼                      ▼
┌────────────────────────────────────────────────────────────────┐
│                         rotameter-core                         │
│   (Pure Rust: zero tokio, zero rusqlite, zero axum, zero I/O)  │
└────────────────────────────────────────────────────────────────┘
```

### Crate Responsibilities
- **`crates/rotameter-core`** — Domain types (`Site`, `EventKind`, `SessionSnapshot`, `Metric`, `Dimension`, `QuerySpec`, `ErrorCode`), `HMAC-SHA256` visitor hashing & salt-window helpers, 30-minute sliding session state machine, URL/referrer/UTM normalizers, compiled-in bot matcher, User-Agent classifier, cardinality overflow guard, and **`Storage` traits**. Must compile cleanly with zero OS/network/async dependencies (`cargo tree -p rotameter-core`).
- **`crates/rotameter-store-sqlite`** — Bundled SQLite WAL implementation (`rusqlite` + `bundled`) implementing all `Storage` traits, numbered forward schema migrations, batched `BEGIN IMMEDIATE` ingest writer, and online `VACUUM INTO` backup routine.
- **`crates/rotameter-server`** — `axum` HTTP server (`tokio` runtime), static embedded `/v1/collect.js` and `/v1/pixel.gif`, bounded ingest MPSC ring buffer + background WAL flusher, per-IP token-bucket rate limiter, `tracing` structured logs (zero raw IPs), and graceful `SIGTERM` drain.
- **`crates/rotameter-client`** — Typed HTTP client wrapping `/v1/*` query, realtime, and admin endpoints, with first-class support for Bearer API keys and Cloudflare Access headers (`CF-Access-Client-Id` / `CF-Access-Client-Secret`).
- **`crates/rotameter-mcp`** — `stdio` JSON-RPC MCP server exposing strictly read-only analytics tools backed by `rotameter-client`.
- **`crates/rotameter-tui`** — `ratatui` + `crossterm` interactive terminal dashboard backed by `rotameter-client`.
- **`crates/rotameter-cli`** — Unified binary entrypoint (`rotameter`) with Cargo features `default = ["server", "sqlite", "mcp", "tui"]`, allowing slim server-only (`--no-default-features --features server,sqlite`) or client-only (`--no-default-features --features mcp,tui`) builds.

---

## 2. Pluggable Storage Trait Boundaries

Storage sits behind narrow async/blocking traits so `rotameter-store-sqlite` can be swapped or supplemented without altering `rotameter-core` or `rotameter-server`:

```rust
pub struct StorageCapabilities {
    pub supports_raw_event_log: bool,
    pub supports_arbitrary_distinct_visitors: bool,
    pub supports_multi_node_concurrent_writes: bool,
    pub supports_sql_funnels: bool,
}

pub trait ControlPlaneStore: Send + Sync { /* upsert/get/list/delete sites & hashed API keys */ }
pub trait EventIngestStore: Send + Sync  { /* write_batch(events, session_deltas, rollup_deltas) */ }
pub trait AnalyticsQueryStore: Send + Sync { /* summary, timeseries, breakdown, realtime, purge */ }
```

### Future Backend Compatibility Notes (`v2.0+`)
- **PostgreSQL / PlanetScale (MySQL):** Map `dim_strings` and `sessions` directly; partition `events` by month (`RANGE (ts_ms)`).
- **ClickHouse / DuckDB:** `events` denormalizes string dimensions into `LowCardinality(String)` columns; `rollups_*` become materialized views / `AggregatingMergeTree` with `uniqState`.
- **Cloudflare Analytics Engine / D1:** `EventIngestStore` writes analytics points directly (`StorageCapabilities { supports_raw_event_log: false, ... }`) while `ControlPlaneStore` stays in D1/KV.

---

## 3. Ingestion & Ring-Buffer Pipeline

```text
HTTP Handler (/v1/events, /pixel.gif)
  │  1. Domain & auth / rate-limit check (< 0.1 ms)
  │  2. Parse, strip query/fragments, bot filter, drop if ignored
  │  3. Normalize URL, referrer, UTM, UA, GeoIP
  │  4. try_send() into bounded MPSC channel (cap: 16,384 events)
  ▼
  202 Accepted (immediately returned — never waits for SQLite fsync)
  │
  ▼
Single Dedicated Writer Task (owns Session LRU + SQLite write connection)
  │  - Drains up to 512 events OR wakes every 1,000 ms
  │  - Computes visitor_id (HMAC-SHA256) & updates 30m Session LRU
  │  - Resolves / interns string dimensions in dim_strings (in-memory LRU cache)
  │  - Executes 1 batched transaction: BEGIN IMMEDIATE ... COMMIT
  │    (inserts events + upserts sessions + upserts rollups_hourly / rollups_daily)
  ▼
SQLite WAL File (rotameter.db + rotameter.db-wal)
```

### Backpressure & Shutdown Invariants
- **Bounded memory under burst/DDoS:** The MPSC ingest queue is hard-capped at **`16,384` items** (`~4–6 MB`). On queue saturation (`TrySendError::Full`), the HTTP handler **never blocks unboundedly or allocates extra queues**; it drops the incoming event, increments `rotameter_ingest_dropped_total`, and responds `429 Too Many Requests` (`Retry-After: 5`) or `503 Service Unavailable`.
- **Graceful `SIGTERM` / `SIGINT` drain:** On signal, the HTTP listener stops accepting new connections, closes the MPSC sender, flushes every remaining buffered event + session delta inside a `10s` deadline, checkpoints `PRAGMA wal_checkpoint(TRUNCATE)` cleanly, and exits `0`.

---

## 4. Event-Stream-First SQLite WAL Schema

### Connection PRAGMAs
- **Writer connection (1 singleton):**
  - `PRAGMA journal_mode = WAL;`
  - `PRAGMA synchronous = NORMAL;` (safe against app crashes in WAL; minimizes fsync stalls on micro-VM disks)
  - `PRAGMA foreign_keys = ON;`
  - `PRAGMA busy_timeout = 5000;`
  - `PRAGMA cache_size = -16000;` (16 MB page cache)
  - `PRAGMA mmap_size = 67108864;` (64 MB max mmap ceiling — prevents unbounded page-cache inflation)
  - `PRAGMA temp_store = MEMORY;`
- **Reader pool (2–4 read-only connections):**
  - `PRAGMA query_only = ON;` + same `cache_size`, `mmap_size`, `busy_timeout`.

### Core Tables (`0001_initial.sql`)

```sql
CREATE TABLE sites (
  site_id            TEXT PRIMARY KEY,               -- e.g. "colinwolff.dev" or local self_site
  timezone           TEXT NOT NULL DEFAULT 'UTC',
  salt_rotation      TEXT NOT NULL DEFAULT 'daily',  -- daily | weekly | monthly
  respect_gpc        INTEGER NOT NULL DEFAULT 0,
  is_self_site       INTEGER NOT NULL DEFAULT 0,     -- 1 = local preconfigured dogfood site
  allowed_origins    TEXT NOT NULL DEFAULT '[]',     -- JSON array of allowed origins
  allowed_query_keys TEXT NOT NULL DEFAULT '[]',     -- JSON allowlist kept in normalized path
  raw_retention_days INTEGER,                        -- NULL = retain raw events indefinitely
  created_at_ms      INTEGER NOT NULL,
  updated_at_ms      INTEGER NOT NULL
);

CREATE TABLE api_keys (
  key_id        TEXT PRIMARY KEY,
  key_prefix    TEXT NOT NULL,                       -- first 12 chars for lookup/display
  key_hash      BLOB NOT NULL UNIQUE,                -- SHA-256(token)
  scope         TEXT NOT NULL,                       -- ingest | read | admin
  site_id       TEXT REFERENCES sites(site_id) ON DELETE CASCADE, -- NULL only for admin/multi-site
  label         TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  last_used_ms  INTEGER,
  revoked_at_ms INTEGER
);

-- Integer-interned dimension strings keep events & rollups compact on 1 GB VMs
CREATE TABLE dim_strings (
  id     INTEGER PRIMARY KEY,
  kind   INTEGER NOT NULL, -- 1=path, 2=ref_host, 3=ref_url, 4=event_name, 5=utm_*, 6=ua_browser, 7=ua_os, 8=geo_country, 9=geo_region, 10=channel
  value  TEXT NOT NULL,
  UNIQUE(kind, value)
);

-- One row per 30m sliding session (powers visits, bounce rate, visit duration, entry/exit)
CREATE TABLE sessions (
  site_id        TEXT NOT NULL,
  session_id     BLOB NOT NULL,       -- 16-byte random ID assigned when session opens
  visitor_id     BLOB NOT NULL,       -- 16-byte HMAC-SHA256 truncated hash
  started_at_ms  INTEGER NOT NULL,
  last_event_ms  INTEGER NOT NULL,
  event_count    INTEGER NOT NULL,
  pageview_count INTEGER NOT NULL,
  entry_path_id  INTEGER NOT NULL,
  exit_path_id   INTEGER NOT NULL,
  ref_host_id    INTEGER NOT NULL,
  channel_id     INTEGER NOT NULL,
  utm_source_id  INTEGER NOT NULL,
  utm_medium_id  INTEGER NOT NULL,
  utm_campaign_id INTEGER NOT NULL,
  country_id     INTEGER NOT NULL,
  region_id      INTEGER NOT NULL,
  browser_id     INTEGER NOT NULL,
  os_id          INTEGER NOT NULL,
  device_type    INTEGER NOT NULL,    -- 0=unknown, 1=desktop, 2=mobile, 3=tablet
  viewport_class INTEGER NOT NULL,    -- 0=unknown, 1=xs, 2=sm, 3=md, 4=lg, 5=xl
  PRIMARY KEY (site_id, session_id)
) WITHOUT ROWID;

CREATE INDEX idx_sessions_range ON sessions(site_id, started_at_ms, visitor_id);
CREATE INDEX idx_sessions_realtime ON sessions(site_id, last_event_ms);

-- Ordered per-session event stream: unlocks v2.0 funnels & user flows with zero migration rewrite
CREATE TABLE events (
  site_id        TEXT NOT NULL,
  ts_ms          INTEGER NOT NULL,
  session_id     BLOB NOT NULL,
  seq_in_session INTEGER NOT NULL,    -- 1, 2, 3... strictly monotonic within a session
  visitor_id     BLOB NOT NULL,
  event_name_id  INTEGER NOT NULL,    -- interned "pageview", "not_found", "outbound_click", custom
  path_id        INTEGER NOT NULL,
  props_json     TEXT,                -- NULL for plain pageviews; capped JSON object otherwise
  PRIMARY KEY (site_id, ts_ms, session_id, seq_in_session)
) WITHOUT ROWID;

CREATE INDEX idx_events_session_seq ON events(site_id, session_id, seq_in_session);
CREATE INDEX idx_events_path_ts ON events(site_id, path_id, ts_ms);

-- Pre-aggregated time buckets for sub-10ms 24H/7D/30D/12M/ALL-TIME summary & timeseries reads
CREATE TABLE rollups_hourly (
  site_id        TEXT NOT NULL,
  bucket_ts_ms   INTEGER NOT NULL,    -- hour floor (UTC ms)
  pageviews      INTEGER NOT NULL,
  events_total   INTEGER NOT NULL,
  bounced_visits INTEGER NOT NULL,
  visits_started INTEGER NOT NULL,
  total_duration_ms INTEGER NOT NULL,
  PRIMARY KEY (site_id, bucket_ts_ms)
) WITHOUT ROWID;

CREATE TABLE rollups_daily (
  site_id        TEXT NOT NULL,
  day_utc        INTEGER NOT NULL,    -- YYYYMMDD
  pageviews      INTEGER NOT NULL,
  events_total   INTEGER NOT NULL,
  bounced_visits INTEGER NOT NULL,
  visits_started INTEGER NOT NULL,
  total_duration_ms INTEGER NOT NULL,
  PRIMARY KEY (site_id, day_utc)
) WITHOUT ROWID;
```

### Why This Layout Scales on `1 vCPU / 1 GB RAM`
- **Compact rows:** Integer-interned paths/dimensions + `BLOB(16)` IDs + `WITHOUT ROWID` keep raw `events` around **~55–75 bytes/row** (`1M` events ≈ `60–75 MB` on disk).
- **Unique visitors across arbitrary ranges:** `idx_sessions_range(site_id, started_at_ms, visitor_id)` is a **covering index** — counting distinct visitors across `7D`, `30D`, or `90D` never even touches the main `sessions` table pages.
- **Zero-rewrite `v2.0` funnels & user flows:** `events(site_id, session_id, seq_in_session)` already stores exact in-session event ordering (`seq_in_session = 1, 2, 3...`), so `v2.0` funnel windows and step transitions query existing historical data directly.
- **First-boot local `self_site`:** `rotameter serve` idempotently seeds a local site row (`is_self_site = 1`, site ID from `--public-url` host or `_self`) unless disabled via `--no-self-site`, `ROTAMETER_SELF_SITE=false`, or `[server] self_site = false`, enabling local smoke-testing (`v1.0`) and Web UI dogfooding (`v1.5`) without touching external networks.

---

## 5. Concrete Memory & Latency Budgets

| Component | Budget (`1 GB` VM) | Cap Mechanism |
| --- | --- | --- |
| Binary code + static assets | `~12 MB` | Single static `musl` binary |
| Ingest MPSC ring buffer | `<= 6 MB` | Hard capacity `16,384` items |
| Active 30m Session LRU cache | `<= 16 MB` | Hard cap `100,000` active entries (LRU eviction) |
| Dimension string intern LRU | `<= 8 MB` | Hard cap `50,000` cached `(kind, value) -> id` entries |
| SQLite page cache + mmap | `<= 80 MB` | `cache_size = -16000` + `mmap_size = 67108864` across pool |
| **Total steady-state / peak RSS** | **`< 40 MB` idle / `< 128 MB` peak** | Verified by `./scripts/check-memory-budget.sh` |
| Ingest `POST /v1/events` latency | `p99 < 2 ms` | Non-blocking enqueue (`try_send`) |
| `POST /v1/query` (`30D` breakdown) | `p95 < 25 ms` (at 1M events) | Covering index + rollup tables |
