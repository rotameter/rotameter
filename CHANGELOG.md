# Changelog

All notable changes to **Rotameter** will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Authenticated (`rm_ing_` / `rm_adm_`) ingest requests now prefer `X-Rotameter-Client-IP`, `X-Rotameter-Country`, and `X-Rotameter-Region` before falling back to `CF-*` headers so Cloudflare Worker-to-Tunnel proxy hops preserve real visitor hashes and geo without opening unauthenticated header spoofing.

### Changed

- Adopted `ADR-0007` (TypeID-style prefixed UUIDv7 UIDs `org_...`/`site_...`/`key_...`, `orgs` tenancy with auto-seeded `default` org, `slug` + `display_name`, site-scoped `dim_strings`, `SaltProvider`, and per-site `IngestPolicy` seams) across architecture, privacy, API, and interface specs, and aligned workspace manifests to `0.1.0-alpha.1` with repository metadata and `reserve-staging` exclusions (`RM-23`).

### Added

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
