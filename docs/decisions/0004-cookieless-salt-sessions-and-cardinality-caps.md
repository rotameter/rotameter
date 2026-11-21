---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Enforce cookieless HMAC-SHA256 hashing, bounded 30m sessions, and daily cardinality caps

## Context and Problem Statement

Analytics engines face two structural risks on a `1 vCPU / 1 GB RAM` host: tracking designs that store cookies or raw IPs (creating GDPR/compliance liability), and hostile/bot traffic generating unbounded unique URL query strings or sessions that exhaust RAM and SQLite disk. Additionally, naive daily UTC salt rotation splits live visitor sessions that cross `00:00 UTC`.

## Decision Drivers

* Zero cookies, zero tracking browser storage, and zero raw IP persistence anywhere on disk or in logs.
* Accurate 30-minute session metrics (visits, bounce rate, visit duration) that do not fracture at midnight UTC.
* Deterministic upper bounds on RAM (`<= 16 MB` session cache) and SQLite `dim_strings` growth under query-string bombs.
* Zero-RAM GeoIP option when deployed behind Cloudflare (`CF-IPCountry`), without locking out non-Cloudflare hosts.

## Considered Options

* Cookieless `HMAC-SHA256` with default `daily` CSPRNG salt (+ 30m midnight grace, per-site `daily`/`weekly`/`monthly` option), 100k bounded Session LRU, `header`/`mmdb` GeoIP, and daily per-site cardinality caps overflowing into `(other)`
* Fixed 8-hour rotating hash without session state (GoatCounter-style)
* Unbounded session hash maps and verbatim full-URL storage

## Decision Outcome

Chosen option: "Cookieless `HMAC-SHA256` with default `daily` CSPRNG salt (+ 30m midnight grace, per-site `daily`/`weekly`/`monthly` option), 100k bounded Session LRU, `header`/`mmdb` GeoIP, and daily per-site cardinality caps overflowing into `(other)`", because it combines strict zero-PII privacy with accurate session metrics and hard resource ceilings.

### Consequences

* Good, because active `23:55 -> 00:05 UTC` visits remain intact without storing persistent cross-day identifiers.
* Good, because stripping arbitrary query parameters (after `utm_*` extraction and explicit per-site `allowed_query_keys`) plus daily `(other)` caps stops URL-fuzzing bots from bloating SQLite.
* Good, because `header` GeoIP reads `CF-IPCountry`/`CF-Region-Code` at `0 MB` RAM cost when `trusted_proxy_headers = true`, while `.mmdb` remains available standalone.
* Neutral, because under strict `daily` salt rotation, a visitor returning across multiple days counts once per UTC day (`weekly`/`monthly` opt-in available per site when multi-day deduplication is preferred).

### Confirmation

* `RM-4` unit/property tests verify salt rotation, 30-min midnight carryover, LRU eviction bounds, and `(other)` cardinality overflow.
* `RM-17` executes both the midnight rollover gate (`23:55 -> 00:05 UTC`) and the 100,000-URL cardinality-bomb gate against a live server.

## Pros and Cons of the Options

### Bounded Session LRU + CSPRNG Salt + Cardinality Overflow Caps

* Good, because memory and dimension table growth stay strictly bounded regardless of traffic shape.
* Good, because `localStorage.rotameter_ignore` and optional per-site `respect_gpc` give explicit visitor control without breaking accuracy on deprecated `DNT`.

### Counter-only 8h hash (no session state)

* Bad, cannot measure visits, bounce rate, visit duration, or ordered per-session events needed for `v2.0` funnels.

### Verbatim URL storage and unbounded caches

* Bad, invites instant OOM or disk exhaustion on a `1 GB` micro VM when scanned by crawlers.

## More Information

* Storypole: `RM-3`, `RM-4`, `RM-8`, `RM-17`.
* Specs: [`../SPEC-PRIVACY.md`](../SPEC-PRIVACY.md), [`../COMPARISON-METRICS.md`](../COMPARISON-METRICS.md).
