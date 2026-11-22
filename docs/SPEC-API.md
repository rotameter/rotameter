# HTTP API & Collection Contract (`/v1`)

Rotameter exposes a clean-sheet, strongly typed JSON HTTP API under `/v1/*`. No legacy compatibility shims for Plausible, GoatCounter, Matomo, or GA4 exist.

*Related specs: [`SPEC-ARCHITECTURE.md`](./SPEC-ARCHITECTURE.md) · [`SPEC-PRIVACY.md`](./SPEC-PRIVACY.md) · [`SPEC-INTERFACES.md`](./SPEC-INTERFACES.md)*

---

## 1. Conventions & Error Envelope

- **Base prefix:** `/v1/*`
- **Identifiers (`ADR-0007`):** Organization, site, and API key entities use immutable TypeID-style prefixed UUIDv7 UIDs (`org_<26-char base32>`, `site_<26-char base32>`, `key_<26-char base32>`). Sites pair an immutable `site_id` with an org-unique `slug` (e.g., `"example.com"`) and mutable `display_name`; query/admin paths accept `site_id` or `slug` within the authenticated org.
- **Field naming:** `snake_case` everywhere in JSON request/response bodies.
- **Timestamps:** RFC 3339 UTC strings (`"2026-11-20T19:00:00Z"`) with explicit `timezone` field when reporting local-date buckets.
- **Uniform Error Envelope:** Every `4xx` / `5xx` API response returns:
  ```json
  {
    "error": {
      "code": "invalid_query_range",
      "message": "The 'from' timestamp must precede 'to'.",
      "details": { "field": "from" }
    }
  }
  ```

---

## 2. Authentication & Rate Limiting

### Scoped Hashed API Keys
Keys are randomly generated (`32` bytes), prefixed by role (`rm_ing_...`, `rm_rd_...`, `rm_adm_...`), shown **once** at creation, and stored exclusively as `SHA-256` hashes in `api_keys`:

| Scope | Prefix | Permitted Operations |
| --- | --- | --- |
| `ingest` | `rm_ing_` | `POST /v1/events`, `POST /v1/events/batch` for the bound `site_id` (required for server-to-server batching; optional for browser `collect.js` / `pixel.gif` matching `allowed_origins`). |
| `read` | `rm_rd_` | `POST /v1/query`, `GET /v1/sites/{site}/realtime` for the bound `site_id` (used by external dashboards, TUI, MCP). |
| `admin` | `rm_adm_` | Full read + `/v1/admin/sites/*`, `/v1/admin/keys/*` across sites (used by CLI, OpenTofu, setup scripts). |

- **Public browser ingest guardrails:** Unauthenticated `POST /v1/events`, `GET /v1/pixel.gif`, and `GET /v1/collect.js` must match a registered `site_id` and (when configured) `Origin`/`Referer` host in `allowed_origins`, subject to a per-IP in-memory token bucket (e.g., `60 req/min`, burst `20`).

---

## 3. Collection Endpoints

### 3.1 `GET /v1/collect.js` (`<= 1.5 KB` gzip)
Serves the immutable/cacheable (`Cache-Control: public, max-age=86400`) first-party browser tracker:
- Zero cookies; aborts immediately if `localStorage.getItem("rotameter_ignore") === "true"`.
- Reads configuration from script attributes (`data-site`, `data-endpoint`, optional `data-outbound`, `data-downloads`, `data-not-found`, `data-spa`).
- Auto-tracks initial `pageview`, optional SPA history transitions (`pushState` / `popstate`), outbound link clicks (`outbound_click`), file downloads (`file_download`), and `404` pages (`not_found`).
- Exposes a tiny imperative global:
  ```js
  window.rotameter("signup_clicked", { props: { plan: "pro" } });
  ```
- Transports via `navigator.sendBeacon` with `fetch(..., { keepalive: true })` fallback.

### 3.2 `GET /v1/pixel.gif` (Zero-JS Fallback)
Returns a `43`-byte transparent `1×1` GIF with `Cache-Control: no-store, max-age=0`, `X-Content-Type-Options: nosniff`:
- Query params: `?site=<site_id>&path=<encoded_path>&ref=<encoded_ref>&event=<optional_name>`.
- Ideal for RSS feeds, strict no-JS environments, `<img>` `<noscript>` fallbacks, and static assets.

### 3.3 `POST /v1/events`
Single-event ingest returning **`202 Accepted`** (empty body or `{"accepted": 1}`), validated and enqueued without blocking on disk:

```json
{
  "site_id": "site_01jb8x9k2m0000000000000000",
  "event": "pageview",
  "url": "https://example.com/projects/rotameter?utm_source=hn",
  "referrer": "https://news.ycombinator.com/",
  "viewport_width": 1440,
  "props": {
    "section": "projects"
  }
}
```

### 3.4 `POST /v1/events/batch`
Server-side / log-importer batch ingest (requires `rm_ing_` or `rm_adm_` Bearer token), accepting up to **`500` events** per request (`202 Accepted` with `{ "accepted": N, "rejected": M }`). Supports explicit `ts` (RFC 3339) and optional synthetic `visitor_hint` (hashed with the same daily salt, never stored raw) for offline log imports (`rotameter import`).

---

## 4. First-Class Astro & Cloudflare Workers Integration (`integrations/astro-cloudflare/`)

Designed specifically for static/hybrid Astro sites deployed on Cloudflare Workers (such as `example.com`) where Rotameter sits privately behind a Cloudflare Tunnel + Access Service Token:

1. **First-Party Worker Proxy Routes** (`src/pages/api/r.js.ts` + `src/pages/api/e.ts`):
   - `GET /api/r.js`: proxies and edge-caches `/v1/collect.js` from `ROTAMETER_URL` using `CF-Access-Client-Id` / `CF-Access-Client-Secret`, keeping the browser strictly `'self'`-only under strict CSP. Returns a harmless `200` empty comment if the origin is temporarily unreachable.
   - `POST /api/e`: validates origin (`example.com` only — silently drops `*.workers.dev` previews and `localhost` with `204`), immediately returns `202` to the browser, and forwards the payload inside **`ctx.waitUntil()`** with `CF-Connecting-IP`, `CF-IPCountry`, `CF-Region-Code`, `User-Agent`, and Access Service Token headers.
2. **Pure Server-Side SSR Helper** (`trackServerHit(request, env, ctx, opts)`):
   - Zero client JavaScript required for on-demand Astro routes (`/status`, `/analytics`, `/rss.xml`, `404.astro`).
   - Enqueues a `pageview` or `not_found` event asynchronously inside `ctx.waitUntil()` with a short timeout (`1500 ms`), never blocking SSR page latency and never throwing if the analytics VM restarts.

---

## 5. Query & Telemetry Endpoints

### 5.1 `POST /v1/query` (Requires `rm_rd_` or `rm_adm_`)
Single composable query endpoint powering dashboards, CLI, MCP, and TUI.

**Request:**
```json
{
  "site_id": "site_01jb8x9k2m0000000000000000",
  "range": { "preset": "30d" },
  "granularity": "day",
  "metrics": ["visitors", "pageviews", "visits", "views_per_visit", "bounce_rate", "avg_visit_duration_ms"],
  "group_by": ["path"],
  "filters": [
    { "dimension": "country", "op": "eq", "value": "US" },
    { "dimension": "path", "op": "prefix", "value": "/projects/" }
  ],
  "compare_previous": true,
  "sort": { "by": "visitors", "dir": "desc" },
  "limit": 20,
  "offset": 0,
  "format": "json"
}
```
- `range`: `{ "preset": "24h" | "7d" | "30d" | "12m" | "all" }` or `{ "from": "...", "to": "..." }`.
- `granularity`: `"hour" | "day" | "month" | null`.
- `filters[].op` (`v1.0`): `"eq" | "neq" | "in" | "contains" | "prefix"`.
- `format`: `"json"` (default) or `"csv"` (`text/csv`).

**Response (`200 OK`):**
```json
{
  "site_id": "site_01jb8x9k2m0000000000000000",
  "slug": "example.com",
  "timezone": "America/Los_Angeles",
  "window": { "from": "2026-10-22T07:00:00Z", "to": "2026-11-21T07:00:00Z" },
  "summary": {
    "visitors": 1842,
    "pageviews": 4190,
    "visits": 2110,
    "views_per_visit": 1.99,
    "bounce_rate": 0.482,
    "avg_visit_duration_ms": 94200
  },
  "previous_summary": {
    "visitors": 1510,
    "pageviews": 3300,
    "visits": 1705,
    "views_per_visit": 1.94,
    "bounce_rate": 0.510,
    "avg_visit_duration_ms": 88100
  },
  "timeseries": [
    { "bucket": "2026-11-20T08:00:00Z", "visitors": 84, "pageviews": 190, "visits": 92 }
  ],
  "rows": [
    { "key": { "path": "/projects/rotameter" }, "visitors": 412, "pageviews": 780, "visits": 450, "bounce_rate": 0.34 }
  ],
  "meta": { "total_rows": 47, "query_ms": 4.2, "truncated_by_cardinality_cap": false }
}
```

### 5.2 `GET /v1/sites/{site}/realtime` (Requires `rm_rd_` or `rm_adm_`)
`{site}` accepts either `site_id` (`site_...`) or `slug` within the authenticated org. Returns active visitors in the last `5 minutes`, current top active pages, and minute-by-minute pulse over the last `30 minutes`.

### 5.3 `GET /v1/health` (Unauthenticated, non-sensitive)
Returns service liveness, storage readiness, schema version, queue depth (`%` full), and uptime—zero secrets, zero site metrics.

---

## 6. Idempotent Admin Endpoints (`v1.0`, Requires `rm_adm_`)

Designed for CLI scripts and future OpenTofu / IaC providers (`rm_adm_` keys are scoped to an `org_id`; single-tenant self-hosted deployments bind to the seeded `default` org):
- `POST /v1/admin/sites` — Creates a site with `slug`, `display_name`, and settings, returning the generated immutable `site_id` (`site_...`).
- `PUT /v1/admin/sites/{site}` — Idempotently creates or updates site settings (`slug`, `display_name`, `timezone`, `salt_rotation`, `respect_gpc`, `allowed_origins`, `allowed_query_keys`, `raw_retention_days`) matched by `site_id` or `slug`.
- `GET /v1/admin/sites` & `GET /v1/admin/sites/{site}` — List/inspect site configuration (including `site_id`, `org_id`, `slug`, `display_name`).
- `DELETE /v1/admin/sites/{site}` — Deletes site and cascades data (`?confirm=true`).
- `POST /v1/admin/keys` — Mints a scoped key (`ingest | read | admin`), returning plaintext token once + `key_id` + `key_prefix`.
- `GET /v1/admin/keys` & `DELETE /v1/admin/keys/{key_id}` — List metadata and revoke keys.

### 6.1 Preconfigured Self-Site Bootstrap (`self_site`)
On first boot / fresh DB initialization, `rotameter serve` idempotently seeds the local `default` org and a preconfigured local site (`is_self_site = 1`) so operators can smoke-test `v1.0` ingest/queries immediately and let the `v1.5` Web UI dogfood itself locally:
- **Site slug & identifier:** Generates a persistent immutable `site_id` (`site_...`) with `display_name = "Self (Local)"` and `slug` derived from `--public-url` host (e.g., `analytics.wolff.industries`) when set, defaulting otherwise to `_self`, overrideable via `--self-site-id` / `ROTAMETER_SELF_SITE_ID` / `[server] self_site_id`.
- **Three-way disable switch (precedence `CLI flag > env var > config`):** `--no-self-site` (or `--self-site=false`), `ROTAMETER_SELF_SITE=false`, or `[server] self_site = false` skips creating/enabling the self-site and omits any self-tracking in `v1.5` UI responses.
- **Loop guard:** Internal `/v1/query`, `/v1/sites/*/realtime`, `/v1/admin/*`, and `/v1/health` requests against the self-site never synthesize self-referential ingest events.
