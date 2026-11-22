---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Adopt opaque prefixed UUIDv7 UIDs, site-scoped dictionaries, and SaaS seams

## Context and Problem Statement

Rotameter `v1.0` starts as a single-binary self-hosted engine on a `1 vCPU / 1 GB RAM` VM, while remaining cleanly extensible into a multi-tenant hosted SaaS (`rotameter-cloud`) in the future without breaking public APIs or requiring table rewrites. Using mutable domain strings as primary keys (`site_id = "example.com"`) and sharing a global `dim_strings` dictionary across sites breaks when domains change, causes cross-tenant collisions, and widens composite `WITHOUT ROWID` indexes.

## Decision Drivers

* Immutable site identity so domain or slug renames never rewrite `events`, `sessions`, or `rollups_*`.
* Cross-tenant isolation for future multi-tenant SaaS (`org_id` tenancy and per-site `dim_strings`).
* Compact fixed-width foreign keys (`16`-byte `BLOB` UUIDv7) in hot `WITHOUT ROWID` tables on `1 GB` VMs.
* Effortless single-operator self-hosting UX (transparent default org and `<site>` resolution by `site_id` or `slug`).
* Open-source workspace remains `MIT OR Apache-2.0` and strictly zero-phone-home; any future SaaS control plane (`rotameter-cloud`) stays external.

## Considered Options

* TypeID-style prefixed UUIDv7 UIDs (`org_...`, `site_...`, `key_...`) + `slug` + `display_name`, site-scoped `dim_strings`, `SaltProvider`, and per-site `IngestPolicy` seams
* Keep domain strings (`TEXT`) as `site_id` primary keys with a global `dim_strings` dictionary

## Decision Outcome

Chosen option: "TypeID-style prefixed UUIDv7 UIDs (`org_...`, `site_...`, `key_...`) + `slug` + `display_name`, site-scoped `dim_strings`, `SaltProvider`, and per-site `IngestPolicy` seams", because `16`-byte binary UUIDv7 keys shrink `events` and `sessions` storage while guaranteeing tenant isolation, clean renames, and zero-effort self-hosted defaults.

### Consequences

* Good, because `orgs`, `sites`, and `api_keys` serialize as self-describing Crockford base32 strings (`org_<26>`, `site_<26>`, `key_<26>`) in JSON/CLI while storing as compact `16`-byte `BLOB` (SQLite) or `UUID` (`v2.0` PostgreSQL).
* Good, because `sites` enforces `UNIQUE(org_id, slug)` while allowing mutable `slug` and `display_name` updates in place.
* Good, because `dim_strings(id, site_id, kind, value)` enforces `UNIQUE(site_id, kind, value)` with `ON DELETE CASCADE`, eliminating cross-tenant leakage and simplifying erasure.
* Good, because `SaltProvider` (`rotameter-core`, `RM-4`) and per-site `IngestPolicy` (`rotameter-server`, `RM-8`) allow multi-node salt sharing and per-plan quotas in `rotameter-cloud` without altering OSS core logic.
* Neutral, because first-boot self-hosted initialization seeds one transparent local default organization (`slug = "default"`), and CLI/MCP/TUI accept either `site_id` or `slug`.

### Confirmation

* `RM-3` and `RM-4` implement prefixed UUIDv7 `OrgId`/`SiteId`/`KeyId` types and the `SaltProvider` trait in `rotameter-core`.
* `RM-5` implements `orgs`, `sites`, `api_keys`, and site-scoped `dim_strings` in `0001_initial.sql`.
* `RM-7` and `RM-8` implement `POST`/`PUT` `/v1/admin/sites`, `site_id`/`slug` lookup, and the `IngestPolicy` hook.

## Pros and Cons of the Options

### Prefixed UUIDv7 UIDs + `slug`/`display_name` + SaaS seams

* Good, zero migration pain between single-node self-hosting and multi-tenant cloud hosting.
* Bad, adds small base32 encode/decode boilerplate in `rotameter-core` (`RM-3`).

### Mutable domain strings as primary keys

* Bad, renames break history, two orgs cannot track the same domain, and `dim_strings` leaks across sites.

## More Information

* Storypole: `RM-3`, `RM-4`, `RM-5`, `RM-7`, `RM-8`, `RM-23`.
* Specs: [`../SPEC-ARCHITECTURE.md`](../SPEC-ARCHITECTURE.md), [`../SPEC-API.md`](../SPEC-API.md), [`../SPEC-PRIVACY.md`](../SPEC-PRIVACY.md), [`../SPEC-INTERFACES.md`](../SPEC-INTERFACES.md).
