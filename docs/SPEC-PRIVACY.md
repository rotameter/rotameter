# Privacy, Session & Cardinality Specification (`v1.0`)

Rotameter is privacy-preserving by construction: no personal data touches disk, no tracking state lives on visitor browsers, and hostile URL/property cardinality cannot exhaust memory or storage on a `1 GB` VM.

*Related specs: [`SPEC-ARCHITECTURE.md`](./SPEC-ARCHITECTURE.md) · [`COMPARISON-METRICS.md`](./COMPARISON-METRICS.md) · [`SPEC-API.md`](./SPEC-API.md)*

---

## 1. Non-Negotiable Privacy Invariants

1. **Zero cookies** set by `collect.js`, `pixel.gif`, or `/v1/events`.
2. **Zero tracking storage** read or written on the visitor's browser (the sole exception is reading `localStorage.getItem("rotameter_ignore")` for voluntary opt-out).
3. **Zero raw IP addresses** written to SQLite, crash dumps, debug endpoints, or `tracing` logs.
4. **Zero third-party forwarding & zero phone-home** — all events remain strictly on operator-controlled infrastructure. Preconfigured `self_site` dogfooding operates **100% locally** inside the operator's own database—Rotameter never dials home or transmits telemetry to any external vendor/author server—and can be disabled completely via `--no-self-site`, `ROTAMETER_SELF_SITE=false`, or `[server] self_site = false`.

---

## 2. Visitor Hashing & Salt Rotation

For each incoming request, `rotameter-core` extracts:
- `client_ip`: from `CF-Connecting-IP` (when `trusted_proxy_headers = true`), else `X-Forwarded-For` rightmost trusted hop, else the TCP peer socket address.
- `user_agent`: HTTP `User-Agent` header (truncated to `512 B`).

It computes a 16-byte pseudonymous `visitor_id`:

$$\text{visitor\_id} = \text{HMAC-SHA256}(\text{salt}_{\text{window}}, \text{site\_id} \,\|\, \text{0x00} \,\|\, \text{client\_ip} \,\|\, \text{0x00} \,\|\, \text{user\_agent})[0..16]$$

### Rotation Windows & Midnight Grace Carryover
- **Default (`daily`):** Generates a fresh 32-byte CSPRNG salt at `00:00 UTC`.
- **Configurable per-site `salt_rotation`:** `daily` (default, Plausible-equivalent), `weekly` (ISO week boundary), or `monthly` (1st of UTC month) for operators wanting multi-day deduplication within one bounded calendar window.
- **Midnight 30-Minute Grace Window:** Active sessions do **not** artificially split at `23:59 -> 00:01 UTC`. For `30 minutes` after a salt rotation, the session resolver checks the in-memory LRU against both `HMAC(current_salt, ...)` and `HMAC(previous_salt, ...)`; if a matching active session is still open (`now - last_event_ms <= 1,800,000 ms`), the event attaches seamlessly to that existing `session_id` and `visitor_id` until the session closes naturally.
- **Salt persistence boundary:** Salts are kept **in process memory only** by default (zero salt persistence on disk, so a process restart rotates salts early as an extra privacy property) unless `--persist-salts` is explicitly enabled via an encrypted/0600 sidecar file solely for `weekly`/`monthly` windows across restarts; raw IPs are never persisted regardless.

---

## 3. 30-Minute Sliding Session State Machine

The single writer owns a bounded LRU/TTL map (`max_entries = 100,000`, `~16 MB` max RAM):
- **Key:** `(site_id, visitor_id)`
- **Value:** `{ session_id: [u8; 16], started_at_ms, last_event_ms, event_count, pageview_count, last_seq }`
- **Transitions:**
  1. **Hit within `1,800,000 ms` (`30 min`) of `last_event_ms`:** increment `event_count` (and `pageview_count` if `event_name == "pageview"`), increment `last_seq`, update `last_event_ms = ts_ms`, emit session delta (`bounce` flips `1 -> 0` on `event_count == 2`, `duration_delta = ts_ms - prev_last_event_ms`).
  2. **Miss or expired (`ts_ms - last_event_ms > 1,800,000 ms`):** mint a fresh random 16-byte `session_id`, set `seq = 1`, `visits_started += 1`, `bounced_visits += 1`, attach entry/acquisition/tech dimensions to the new `sessions` row.
- **Eviction:** When the LRU hits `100,000` entries, the oldest entry is evicted cleanly (if an evicted session happens to receive another hit within 30m, a fallback indexed lookup on `sessions(site_id, visitor_id, last_event_ms)` avoids splitting active sessions under heavy burst).

---

## 4. Bot & Crawler Filtering

Before hashing or counting, `rotameter-core` filters synthetic traffic:
1. **Prefetch & Prerender headers:** Drop requests carrying `Sec-Purpose: prefetch`, `Purpose: prefetch`, `X-Moz: prefetch`.
2. **Missing or empty `User-Agent`** on browser `/v1/collect.js` or `/v1/pixel.gif` calls: dropped.
3. **Compiled-in curated matcher:** Fast Aho-Corasick / prefix matcher covering common search crawlers, AI scrapers, uptime monitors, social unfurlers, and headless automation signatures (`Googlebot`, `Bingbot`, `GPTBot`, `ClaudeBot`, `Bytespider`, `HeadlessChrome`, `curl/`, `python-requests/`, etc.).
4. **Server-side explicit override:** Trusted server-to-server batch calls with a scoped `ingest` key may pass `"allow_automated": false` (default — still filters bots) or `true` only if explicitly ingesting synthetic/webhook metrics.
5. **Diagnostic counter only:** Dropped bot count is held as an ephemeral Prometheus/health metric (`rotameter_bots_dropped_total`) so operators can observe crawler load without polluting site metrics.

---

## 5. GeoIP, User-Agent & Referrer Enrichment

### GeoIP Modes
- **`header` (default behind Cloudflare):** Reads `CF-IPCountry` (ISO 3166-1 alpha-2, dropping `XX`/`T1` to `(unknown)`) and optional `CF-Region-Code`. **0 MB RAM overhead.** Trusted **only** when `trusted_proxy_headers = true` (e.g., behind `cloudflared` on `127.0.0.1` or configured trusted CIDR); spoofed headers on untrusted sockets are stripped.
- **`mmdb` (standalone / non-Cloudflare):** Optional local GeoLite2/DB-IP `.mmdb` file (`maxminddb` crate), memory-mapped (`mmap`), resolving Country + Subdivision + optional City (coarse city name only, never lat/long coordinates stored).

### User-Agent Classification
Parsed in `rotameter-core` into coarse family + major version (`Firefox 132`, `Chrome 131`, `Safari 18`, `macOS 15`, `Linux`, `iOS 18`, `Android 15`) and `device_type` (`desktop` | `mobile` | `tablet`). The raw `User-Agent` string is discarded immediately after classification and hashing.

### Referrer & UTM Normalization
- Extracts `utm_source`, `utm_medium`, `utm_campaign`, `utm_content`, `utm_term` (each capped at `128 B`).
- Strips self-referrals matching `site_id` or configured `allowed_origins` -> treated as Direct (`(direct)`).
- Normalizes known social/search aliases (e.g. `l.instagram.com` -> `instagram.com`, `t.co` -> `x.com`, `www.google.co.uk` -> `google.com`) and assigns `channel` (`Direct`, `Organic Search`, `Organic Social`, `Referral`, `Paid / Campaign`, `Email`).

---

## 6. URL Normalization & Cardinality Protection

To prevent bot query-string bombs or hostile custom properties from bloating SQLite on a `1 GB` VM:

1. **URL normalization:**
   - Reject non-HTTP(S) schemes; require valid host matching registered site.
   - Strip URL `#fragment` completely.
   - Extract `utm_*` query parameters, then strip **all** remaining query parameters except keys explicitly listed in `site.allowed_query_keys` (sorted deterministically, e.g., `/analytics?range=7d`).
   - Normalize trailing slashes consistently (preserve `/` root, trim duplicate slashes).
2. **Hard byte limits (truncate/reject cleanly):**
   - `path`: max `512` bytes.
   - `event_name`: max `64` bytes (ASCII `[a-zA-Z0-9_.:-]`).
   - `props`: max `10` keys per event; max `64` bytes per key; max `128` bytes per scalar string value; nested objects/arrays rejected/flattened cleanly.
3. **Daily Per-Site Dimension Overflow Cap:**
   - Each site caps new unique `dim_strings` creations per UTC day per dimension kind (defaults: `10,000` unique paths/day, `5,000` unique referrer URLs/day, `2,000` unique custom prop values/day).
   - Once a site hits its daily cap for a dimension kind, any novel unseen value maps deterministically to **`(other)`** in `dim_strings`—preserving total pageviews/counts while guaranteeing bounded disk growth.

---

## 7. Privacy Signals & Retention

- **`DNT` (Do Not Track):** Ignored by default (deprecated W3C header that over-filters technical audiences).
- **`Sec-GPC` (Global Privacy Control):** Configurable per site (`respect_gpc = false` default because cookieless first-party Rotameter neither sells nor shares personal data; set `respect_gpc = true` to drop events when `Sec-GPC: 1` is present).
- **Client Opt-Out:** `collect.js` skips sending when `window.localStorage?.getItem("rotameter_ignore") === "true"`.
- **Retention & Erasure:**
  - `raw_retention_days` (nullable): nightly background pruner deletes `events` and `sessions` older than `N` days while preserving `rollups_hourly` and `rollups_daily` indefinitely.
  - `rotameter site purge <site> --before <date>` removes raw + rollup data before a cutoff; `rotameter site delete <site>` removes all data via `ON DELETE CASCADE`.
