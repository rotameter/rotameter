# Changelog

All notable changes to **Rotameter** will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Adopted `ADR-0007` (TypeID-style prefixed UUIDv7 UIDs `org_...`/`site_...`/`key_...`, `orgs` tenancy with auto-seeded `default` org, `slug` + `display_name`, site-scoped `dim_strings`, `SaltProvider`, and per-site `IngestPolicy` seams) across architecture, privacy, API, and interface specs, and aligned workspace manifests to `0.1.0-alpha.1` with repository metadata and `reserve-staging` exclusions (`RM-23`).

### Added

- Pure-Rust domain models in `rotameter-core`: TypeID-style prefixed UUIDv7 UIDs (`OrgId`, `SiteId`, `KeyId`), slug/ref validation (`SiteRef`), `Org`/`Site`/`ApiKeyRecord`/`EventKind`/`EventProps`/`QuerySpec`, URL/UTM/referrer/channel normalizers, Aho-Corasick bot/prefetch filter, and coarse User-Agent classifier (`RM-3`).
- 7-crate Cargo workspace (`rotameter-core`, `rotameter-store-sqlite`, `rotameter-server`, `rotameter-client`, `rotameter-mcp`, `rotameter-tui`, `rotameter-cli`) with `MIT OR Apache-2.0` license files, shared `target/` + `sccache` configuration in `.cargo/config.toml`, workspace Clippy/`rustfmt` lints, `scripts/check-core-purity.sh`, and GitHub Actions CI (`RM-2`).
- Initial specification suite (`PROJECT.md`, `docs/SPEC-*.md`, `docs/COMPARISON-*.md`), MADR 4.0 decisions (`ADR-0001`–`ADR-0006`), Storypole backlog (`RM-1`–`RM-22`), and Gaugeblock suite workflow configuration (`RM-1`).
