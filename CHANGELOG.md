# Changelog

All notable changes to **Rotameter** will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added the canonical Web UI architecture and design system specification (`docs/SPEC-UI.md` and `ADR-0009`) covering the `Bun` workspace monorepo, CAD/flight-instrument `tokens.css`, WCAG 2.2 AA rules, hybrid charting + deferred WebGL policy, dynamic `--density-table-row-h` virtualization, `/v1/capabilities` + `/v1/auth/*` contracts, and lazy-loaded `@rotameter/plugin-*` seams across `0.3.0`–`0.5.0` (`RM-18`, `RM-37`..`RM-40`).

- Added zero-downtime `POST /v1/admin/backup` live portable `.tar.zst` export (`LiveBackupExporter` in `rotameter-server` + `--features arrow` / `--features duckdb` adapters in `rotameter-cli`) and `DuckDB` chunked Arrow/Parquet export (`export_analytics_dir_sync` + offline `--analytics-db duckdb://...` support in `rotameter backup --format portable`) so running split `SQLite` + `DuckDB` deployments can be backed up without stopping ingest (`RM-34`).
- Added VM-safe `DuckDB` defaults (`256MB` memory limit, `1` thread) with `--duckdb-memory-limit`/`--duckdb-threads`, `ROTAMETER_DUCKDB_MEMORY_LIMIT`/`ROTAMETER_DUCKDB_THREADS`, and `[storage]` TOML overrides, plus `Dockerfile.duckdb` (`debian` glibc + `distroless/cc-debian12:nonroot`) and `THIRD_PARTY_NOTICES` for bundled `DuckDB` attribution.
- Added the opt-in `rotameter-store-duckdb` OLAP backend crate (`AnalyticsDataPlane` backed by zero-copy Arrow `RecordBatch` appenders, full summary/timeseries/breakdown/realtime/purge parity with `SQLite`, and feature-gated behind `--features duckdb` in `rotameter-cli` for `--analytics-db duckdb://...`, `rotameter migrate --plane analytics`, and `rotameter restore --plane analytics`, plus a dedicated CI job while keeping default builds free of `duckdb`/`arrow`/`parquet`) (`RM-34`).
- Added the feature-gated `rotameter-arrow` exchange crate defining canonical Arrow Schema v1 for `events`, `sessions`, `rollups_hourly`, `rollups_daily`, `rollups_path_daily`, and `QueryResult` plus `WriteBatch <-> RecordBatch` adapters, Zstd Parquet I/O, SHA-256 `manifest.json` verification, and `.tar.zst` archive packing; added opt-in `arrow` feature wiring in `rotameter-cli` for `rotameter backup --format portable` (`--archive` / `.tar.zst`), `rotameter restore`, and `rotameter migrate --from <dsn> --to <dsn> --plane all|control|analytics [--stage-dir]` while keeping default `SQLite` builds free of Arrow/Parquet crates (`RM-33`).
- Added cached DB-backed rotating `HMAC-SHA256` visitor salts (`0004_salts` migration with `rotating_salts` table and atomic `ControlPlaneStore::{get_or_create_salt, prune_salts_before}` methods, enabled by default with automatic expiry pruning past the 30-minute grace window and `--salt-store=db|memory` / `ROTAMETER_SALT_STORE` opt-out) and `--role=all|ingest|query` (`ROTAMETER_ROLE`) endpoint/pipeline gating for multi-node `rotameter serve` deployments (`RM-32`).
- Added streaming log receiver support to `rotameter import` (`BufRead` line streaming, `--stdin` / `-` for Apache piped logs, `--unix-socket` datagram/stream listener for Nginx `syslog` and Caddy `net unix//`, `--follow` with inode/truncation log-rotation handling, bounded non-blocking flush queue with `--batch-size`/`--flush-interval` and `SIGTERM` drain, proxy noise filtering mapping `200` -> `pageview` and `404` -> `not_found`, and `caddy-json` format preset), extended `BatchEventItem` / `POST /v1/events/batch` with optional `user_agent`, `country`, and ephemeral `ip`, and documented copy-pasteable Apache/Nginx/Caddy snippets in `docs/SPEC-API.md` (`RM-30`).

### Changed

- Bumped workspace Rust MSRV from `1.99` to `1.100` across `Cargo.toml`, CI/release workflows, Dockerfiles, and contributor docs.
- Converted `ControlPlaneStore`, `EventIngestStore`, and `AnalyticsQueryStore` in `rotameter-core` to runtime-agnostic `BoxFuture` async traits (`std::future::Future` with zero `tokio` in core), introduced the `AnalyticsDataPlane` supertrait and OLTP/OLAP/raw-append capability flags on `StorageCapabilities`, moved `format_query_result_csv` into `rotameter-core`, added `0003_split_dim_strings` to decouple `dim_strings` from `sites` for split OLTP/OLAP `SQLite` stores, split `rotameter-server` `AppState` via `ServerStores::{unified, split}` with `tokio::task::spawn_blocking` in `SqliteServerStore`, and wired `--db`/`--control-db`, `--analytics-db`, and `[storage]` TOML DSN resolution into `rotameter-cli` (`RM-31`).

## [0.1.1] - 2026-11-25

### Added

- Exported a zero-dependency, Astro-bundlable CSP-safe browser beacon (`@rotameter/astro-cloudflare/beacon`) supporting `localStorage.rotameter_ignore`, `not_found`, SPA navigation, `outbound_click`, and `file_download`, and honored explicitly configured preview/staging origins (`*.workers.dev`, `*.pages.dev`) in `ROTAMETER_ALLOWED_ORIGINS` while keeping unlisted previews blocked (`RM-27`).

### Fixed

- Preferred `ROTAMETER_ACCESS_CLIENT_ID` / `ROTAMETER_ACCESS_CLIENT_SECRET` over generic `CF_ACCESS_CLIENT_*` credentials in `buildAccessHeaders` so Rotameter-scoped Cloudflare Access headers win when both are bound (`RM-27`).
- Treated `data-rotameter-not-found="false"` consistently with `<meta name="rotameter-not-found" content="false">` in `@rotameter/astro-cloudflare/beacon`, and extended `.github/workflows/release.yml` to smoke-import `./dist/beacon.js` (`RM-27`).

### Changed

- Adopted `ADR-0008` (OLTP/OLAP storage trait split with single-DB default, feature-gated `rotameter-arrow` canonical exchange and portable Parquet/JSON backup/restore/migrate, 3-layer DB adapter strategy, dual-granularity webhook/Arrow sink seams, and 4-rung multi-node scaling ladder) and re-sequenced the roadmap across `0.1.0` -> `0.6.0` -> `1.0.0` -> `2.0.0` across `docs/ROADMAP.md`, `PROJECT.md`, `docs/SPEC-ARCHITECTURE.md`, `README.md`, and `AGENTS.md`.

## [0.1.0] - 2026-11-22

### Fixed

- Bundled `assets/collect.js` inside `crates/rotameter-server/` with a drift check in `scripts/check-collector-size.sh` so `rotameter-server` crate archives on crates.io compile self-contained without workspace-relative path traversal.
- Authenticated (`rm_ing_` / `rm_adm_`) ingest requests now prefer `X-Rotameter-Client-IP`, `X-Rotameter-Country`, and `X-Rotameter-Region` before falling back to `CF-*` headers so Cloudflare Worker-to-Tunnel proxy hops preserve real visitor hashes and geo without opening unauthenticated header spoofing.

### Changed

- Upgraded workspace dependencies to current releases (`maxminddb 0.33`, `ratatui 0.30`, `crossterm 0.29`, `rusqlite 0.43`, `clap 4.6`, `getrandom 0.4`, `toml 1.2`, transitive `lru 0.18.5`) and resolved `maxminddb`/`lru` security advisories alongside Rust `1.100.0` Clippy/manifest lint cleanups.
- Configured `@rotameter/astro-cloudflare` (`integrations/astro-cloudflare/`) for compiled ESM + `.d.ts` npm packaging (`dist/`, `exports`, `publishConfig` provenance), wired automated pure-OIDC npm publishing into `.github/workflows/release.yml` and CI validation into `.github/workflows/ci.yml`, pointed repository metadata to `github.com/rotameter/rotameter`, and replaced remaining internal domain examples in docs and tests with `example.com` placeholders.
- Aligned release roadmap across `docs/ROADMAP.md`, `PROJECT.md`, and `AGENTS.md` to Rust/Cargo SemVer progression (`0.1.0` headless engine -> `0.2.0` Web UI/tags/remote MCP -> `0.3.0` funnels/multi-backend -> `0.4.0` plugins/webhooks -> `1.0.0` stable GA freeze) while keeping HTTP routes on `/v1/*`.
- Adopted `ADR-0007` (TypeID-style prefixed UUIDv7 UIDs `org_...`/`site_...`/`key_...`, `orgs` tenancy with auto-seeded `default` org, `slug` + `display_name`, site-scoped `dim_strings`, `SaltProvider`, and per-site `IngestPolicy` seams) across architecture, privacy, API, and interface specs, and aligned workspace manifests to `0.1.0-alpha.1` with repository metadata and `reserve-staging` exclusions (`RM-23`).

### Added

- Brand SVG icon sprite, standalone icons, adaptive light/dark `favicon.svg`, multi-size `favicon.ico`, Apple/PWA PNG icons, and `site.webmanifest` under `assets/brand/`, alongside refreshed `0.1.0` publishing documentation (`README.md`, `CONTRIBUTING.md`, `SPEC-INTERFACES.md`, `SPEC-ARCHITECTURE.md`).
- Interactive `ratatui` flight-instrument terminal UI (`rotameter-tui` / `rotameter tui`) with site switcher (`s`), `1`–`5` range hotkeys (`24H`/`7D`/`30D`/`12M`/`ALL`), 5m live ticker, KPI strip with period deltas, ASCII block chart with dense cap, today accent, horizontal cursor scrubber (`←`/`→`/`h`/`l`), breakdown tabs (`PAGES`/`SOURCES`/`GEO`/`TECH`/`EVENTS`), `Enter`/`Esc` filter drill-down, and non-blocking `30s` polling (`RM-15`).
- Read-only `stdio` MCP server (`rotameter-mcp` / `rotameter mcp`) speaking JSON-RPC 2.0 and exposing all 7 read-only analytics tools (`rotameter_sites` with `default_site` fallback on `rm_rd_` keys, `rotameter_summary`, `rotameter_timeseries`, `rotameter_breakdown`, `rotameter_realtime`, `rotameter_compare_periods`, and `rotameter_query`) (`RM-14`).
- `rotameter seed` subcommand (`--site`, `--days`, `--sessions-per-day`, `--realtime-visitors`, `--seed`, `--reset`) synthesizing deterministic multi-day diurnal curves, weighted referrers/channels/UTMs, geo/browser/device breakdowns, and trailing 30-minute realtime pulse into local SQLite stores (`RM-26`).
- `v1.0` release verification, load gates & daily query rollup acceleration (`0002_query_rollups` migration adding `seen_visitors_daily`, `rollups_daily.visitors`, and `rollups_path_daily`): `10,000`-event `SIGTERM` zero-loss drain test, midnight salt rollover + `100,000`-path cardinality bomb tests, `1,000,000`-event `30d` query SLA (`p95 < 25 ms`), `< 40 MB` idle / `< 128 MB` peak RSS check (`scripts/check-memory-budget.sh`), Astro/CF Workers live E2E test (`scripts/test-astro-e2e.mjs`), and unified `scripts/run-release-gates.sh` (`RM-17`).
- Multi-arch release workflow (`.github/workflows/release.yml` for `x86_64`/`aarch64` `musl` and Apple Silicon/Intel macOS), multi-arch distroless `Dockerfile`, hardened `systemd` unit (`infra/rotameter.service`), Litestream R2 replication manifests (`infra/litestream.yml`), and verified restore drill script (`infra/restore-drill.sh`) (`RM-16`).
- Zero-dependency TypeScript Astro & Cloudflare Workers integration package (`@rotameter/astro-cloudflare` under `integrations/astro-cloudflare/`) providing edge-cached `/api/r.js` collector proxy, `waitUntil`-backed `/api/e` event proxy with Cloudflare Access Service Token headers and preview host dropping, and pure SSR `trackServerHit()` helper (`RM-11`).
- Automated `scripts/check-collector-size.sh` (`<= 1,536 B` gzip + zero cookies/tracking storage CI gate), `scripts/test-collect-js.mjs` DOM harness, and expanded `GET /v1/collect.js` & `GET /v1/pixel.gif` edge-case integration tests (`RM-10`).
- Unified feature-gated `rotameter` CLI (`serve` with `--public-url`/`--no-self-site`/`--self-site-id` `CLI > env > config` precedence, `site`, `key`, `query`, `realtime --watch`, `import` JSONL/CLF with `--dry-run`, `backup`, and `doctor`) with `table`/`json`/`csv` output and documented exit codes (`RM-13`).
- Server-side per-IP token-bucket rate limiter (`429` + `Retry-After`) on unauthenticated collection endpoints, idempotent local `self_site` first-boot initialization with `/v1/` loop guard, `POST /v1/events/batch` (`<= 500` events with `visitor_hint` and historical `ts`), and optional local MaxMind `.mmdb` GeoIP fallback (`RM-25`).
- Typed async and blocking `/v1/*` HTTP client (`rotameter-client`), `config.toml` profile resolution (`--url`/`--token`/`--profile` > `ROTAMETER_*` > `~/.config/rotameter/config.toml`), and automatic `CF-Access-Client-Id` / `CF-Access-Client-Secret` header support (`RM-12`).
- Slim MVP HTTP server (`rotameter-server`) and bare `rotameter serve` CLI entrypoint (`rotameter-cli`): scoped SHA-256 Bearer auth (`rm_ing_`/`rm_rd_`/`rm_adm_`), `ROTAMETER_BOOTSTRAP_ADMIN_KEY` bootstrap, `/v1/health`, `/v1/admin/sites/*`, `/v1/admin/keys/*`, bounded `16,384`-event MPSC ingest (`POST /v1/events`, Cloudflare GeoIP headers, `Sec-GPC`, origin guardrails, `SIGTERM` WAL checkpoint drain), `POST /v1/query` (`JSON`/`CSV`), `GET /v1/sites/{site}/realtime`, `GET /v1/collect.js` (`<= 1,536 B` gzip), and `GET /v1/pixel.gif` (`RM-7`, `RM-8`, `RM-9`).
- `rotameter-store-sqlite` analytics query engine (`AnalyticsQueryStore`), `IANA` timezone timeseries bucketing, multi-dimension breakdowns, `eq`/`neq`/`in`/`contains`/`prefix` SQL filters, `RFC 4180` CSV projection (`format_query_result_csv`), 5-minute realtime snapshot + 30-minute pulse, `raw_retention_days` raw-row pruner, and `purge_before` (`RM-6`).
- `rotameter-store-sqlite` WAL persistence backend: bundled `rusqlite`, `0001_initial.sql` migrations (`orgs`, `sites`, `api_keys`, site-scoped `dim_strings`, `sessions`, `events`, `rollups_hourly`/`daily`), LRU-cached `DimInterner`, `ControlPlaneStore` & `EventIngestStore` batched `BEGIN IMMEDIATE` writer, WAL checkpoint, and online `VACUUM INTO` backups (`RM-5`).
- Privacy, session, cardinality, and storage abstractions in `rotameter-core`: `HMAC-SHA256` visitor hashing, `daily`/`weekly`/`monthly` salt window rotation with `30m` grace carryover, `SaltProvider` trait + `InMemorySaltProvider`, `O(1)` bounded `SessionTracker` LRU, daily per-site `CardinalityGuard` (`(other)` overflow), and `ControlPlaneStore`/`EventIngestStore`/`AnalyticsQueryStore` traits (`RM-4`).
- Pure-Rust domain models in `rotameter-core`: TypeID-style prefixed UUIDv7 UIDs (`OrgId`, `SiteId`, `KeyId`), slug/ref validation (`SiteRef`), `Org`/`Site`/`ApiKeyRecord`/`EventKind`/`EventProps`/`QuerySpec`, URL/UTM/referrer/channel normalizers, Aho-Corasick bot/prefetch filter, and coarse User-Agent classifier (`RM-3`).
- 7-crate Cargo workspace (`rotameter-core`, `rotameter-store-sqlite`, `rotameter-server`, `rotameter-client`, `rotameter-mcp`, `rotameter-tui`, `rotameter-cli`) with `MIT OR Apache-2.0` license files, shared `target/` + `sccache` configuration in `.cargo/config.toml`, workspace Clippy/`rustfmt` lints, `scripts/check-core-purity.sh`, and GitHub Actions CI (`RM-2`).
- Initial specification suite (`PROJECT.md`, `docs/SPEC-*.md`, `docs/COMPARISON-*.md`), MADR 4.0 decisions (`ADR-0001`–`ADR-0006`), Storypole backlog (`RM-1`–`RM-22`), and Gaugeblock suite workflow configuration (`RM-1`).
