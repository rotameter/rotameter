# Feature-by-Feature Comparison

An honest engineering comparison between **Rotameter** and established self-hosted and cloud analytics engines. Rotameter capabilities are tagged explicitly by release phase (**`v1.0`**, **`v1.5`**, **`v2.0`**) so planned work is never presented as already shipped.

*See [`COMPARISON-METRICS.md`](./COMPARISON-METRICS.md) for the metric-by-metric breakdown and formulas, and [`../PROJECT.md`](../PROJECT.md) for Rotameter's roadmap.*

---

## 1. Runtime, Footprint & Operations

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Implementation stack** | Rust (`axum` + `rusqlite`) (`v1.0`) | Go | Elixir / Phoenix | Next.js / Node.js | PHP | Google Cloud SaaS |
| **License** | `MIT OR Apache-2.0` (`v1.0`) | MIT / EUPL | AGPL-3.0 | MIT | GPLv3 | Proprietary SaaS |
| **Deployment unit** | Single static binary (`musl` / macOS) (`v1.0`) | Single Go binary | 3+ containers (App, Postgres, ClickHouse) | App + DB container | PHP + MySQL/MariaDB + cron | Managed SaaS only |
| **Minimum comfortable RAM** | **~35 MB idle / < 128 MB peak (1 GB VM)** (`v1.0`) | ~30–100 MB | 2–4 GB (ClickHouse OOM risk < 2 GB) | ~300–512 MB | 1–2 GB+ | N/A |
| **Multi-arch static releases** | `linux/amd64`, `linux/arm64`, `darwin` (`v1.0`) | Linux/macOS/Windows | Docker (`amd64`/`arm64`) | Docker (`amd64`/`arm64`) | Tarball / Docker | N/A |
| **Self-hosting control** | Full (`v1.0`) | Full | Full | Full | Full | None |
| **Horizontal / multi-node scale** | Single-node SQLite (`v1.0`); ClickHouse/Postgres (`v2.0`) | Single-node SQLite/PG | Multi-node ClickHouse | Postgres / MySQL | MySQL + Redis/Queue | Planet-scale |

---

## 2. Storage & Extensibility

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Embedded zero-ops DB** | Bundled SQLite WAL (`v1.0`) | SQLite (or Postgres) | No (requires PG + CH) | No (requires PG/MySQL) | No (requires MySQL) | Managed |
| **Pluggable storage abstraction** | Trait boundary (`v1.0`); PG/CH/DuckDB/CF AE/MySQL (`v2.0`) | SQLite / PostgreSQL | Fixed PG + ClickHouse | Prisma (PG / MySQL) | Fixed MySQL/MariaDB | BigQuery export |
| **Raw ordered per-session event log** | Yes (`events` + rollups) (`v1.0`) | No (rollup counters only) | Yes (ClickHouse) | Yes (session/event rows) | Yes (`log_visit` / `log_link_visit_action`) | Yes (BigQuery export) |
| **Streaming object-store backups** | `VACUUM INTO` + Litestream → R2/S3 (`v1.0`) | SQLite file backup | Volume / CH snapshots | DB dump | DB dump | N/A |
| **Cardinality bomb protection** | Path/prop caps + daily `(other)` overflow (`v1.0`) | Basic limits | CH memory limits | Minimal | Table growth risk | Automatic `(other)` row |

---

## 3. Privacy & Compliance

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Cookieless by default** | Yes (`v1.0`) | Yes | Yes | Yes | Optional (cookies default) | No (uses cookies) |
| **Zero raw IP stored on disk** | Yes — `HMAC-SHA256` only (`v1.0`) | Yes (8h rotating hash) | Yes (daily rotating salt) | Hash only | Truncation optional | Truncated/scrubbed in EU |
| **Salt rotation policy** | Daily + 30m grace default; per-site `daily`/`weekly`/`monthly` (`v1.0`) | 8h rotating session hash | Fixed daily UTC salt | Monthly/salt-based | Configurable | N/A (client ID cookie) |
| **GDPR banner usually unnecessary** | Yes (`v1.0`) | Yes | Yes | Yes | Only in cookieless mode | Usually required |
| **Zero third-party client requests** | First-party proxy / self-hosted (`v1.0`) | Self-hosted | Self-hosted | Self-hosted | Self-hosted | Sends to Google |
| **Local GeoIP without external API** | `CF-IPCountry` header (0 MB) or `.mmdb` (`v1.0`) | MaxMind / DB-IP | MaxMind / DB-IP | MaxMind / GeoLite | DB-IP / MaxMind | Google edge IP |
| **Privacy signal / opt-out** | `localStorage.rotameter_ignore` + optional `Sec-GPC` (`v1.0`) | `localStorage` opt-out + DNT | `localStorage.plausible_ignore` | DNT optional | DNT + opt-out iframe | Opt-out browser addon |

---

## 4. Collection Methods & Integrations

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Lightweight JS tracker** | `< 1.5 KB` gzip `/v1/collect.js` (`v1.0`) | ~3.5 KB gzip `count.js` | `< 1 KB` gzip `plausible.js` | ~2 KB `script.js` | ~22 KB+ `matomo.js` | ~50 KB+ `gtag.js` |
| **1×1 zero-JS tracking pixel** | `/v1/pixel.gif` (`v1.0`) | Yes (`/count`) | No (JS/API only) | No (JS/API only) | Yes (`matomo.php`) | Measurement Protocol |
| **Server-side HTTP ingest & batch** | `POST /v1/events` & `/batch` (`v1.0`) | `POST /api/v0/count` | `POST /api/event` (single) | `POST /api/send` | Tracking HTTP API + bulk | Measurement Protocol |
| **First-class Astro / CF Workers SSR** | Reference proxy + `waitUntil()` helper (`v1.0`) | Manual HTTP POST | Manual HTTP POST | Manual HTTP POST | Manual HTTP POST | Server-side GTM |
| **Server log importer** | CLI `rotameter import` (`v1.0` JSONL/CLF) | Built-in (`goatcounter import`) | No | No | Python log importer | No |
| **Google Tag Manager & CF Zaraz** | Deferred (`v1.5`) | Manual custom HTML | Community docs | Community docs | Official templates | Native |

---

## 5. Interfaces & Developer Experience

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Clean typed Query HTTP API** | `POST /v1/query` + CSV (`v1.0`) | `/api/v0/stats/*` | Stats API v2 | REST API (paid on Cloud) | Reporting API | GA Data API |
| **Idempotent IaC / OpenTofu Admin API** | `PUT /v1/admin/sites/{id}` (`v1.0`) | Basic site endpoints | Provisioning API (Enterprise) | Teams/websites API | API (`SitesManager`) | Admin API |
| **First-class CLI (`query`, `backup`, `doctor`)** | Yes (`v1.0`) | Server/import CLI | Minimal mix tasks | DB scripts | `console` PHP CLI | None (gcloud/API) |
| **Built-in MCP server (AI agents)** | `stdio` read-only (`v1.0`); HTTP + opt-in admin (`v1.5`) | None | None | External community MCP | None | Official / community MCP |
| **Interactive Terminal UI (TUI)** | `ratatui` block chart + drill-down (`v1.0`) | None | None | None | None | None |
| **Built-in Web Dashboard UI** | Deferred (`v1.5`, `--ui=off` default) | Yes (HTML templates) | Yes (React/Tailwind) | Yes (Next.js) | Yes (Vue/PHP) | Yes |
| **Multi-user accounts / OIDC / RBAC** | Scoped API keys (`v1.0`); Users/RBAC (`v1.5`) | Basic logins | Yes (teams/roles) | Yes (teams/roles) | Granular RBAC | Google IAM roles |

---

## 6. Product Analytics & Enterprise Features

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Custom events & JSON properties** | Key-value props (`v1.0`) | Limited event paths | Custom events + props | Custom events + data | Events (`category/action/name`) | Event parameters |
| **Outbound links, 404s & downloads** | Built into `collect.js` (`v1.0`) | Event/path conventions | Script extensions | Manual / custom events | Built-in | Enhanced measurement |
| **Arbitrary compound filter AST (`AND/OR/NOT` + regex)** | Basic `eq`/`neq`/`in`/`contains`/`prefix` (`v1.0`); full AST (`v2.0`) | Basic search | Filter arrays (partial OR) | Basic filters | Segments | Comparisons & Explorations |
| **Multi-step conversion funnels** | Deferred (`v2.0`) | No | Yes (Enterprise / CE recent) | Yes (Pro / v2+) | Paid plugin / enterprise | Funnel Exploration |
| **User flows / Path transitions** | Deferred (`v2.0`) | No | No | User Journeys | Transitions plugin | Path Exploration |
| **Retention cohorts** | Deferred (`v2.0`) | No | No | Retention report | Cohorts plugin | Cohort Exploration |
| **Session replay / Heatmaps** | Out of scope | No | No | No | Paid plugin | No |

---

## 7. Candid Positioning

**Where Rotameter wins (`v1.0`):**
- You want **Plausible-caliber session metrics and structured querying** on a **\$0 `1 vCPU / 1 GB` VM** without babysitting ClickHouse or PostgreSQL.
- You want a **headless, inspectable analytics instrument** built for static/edge sites (Astro + Cloudflare Workers), CLI scripts, OpenTofu automation, AI workflows (`rotameter mcp`), and terminal operators (`rotameter tui`).
- You want **clean-sheet event-stream data on day 1** so adding a custom Web UI (`v1.5`) or funnels/user flows (`v2.0`) never forces a migration.

**When another tool is the better fit:**
- Need a polished **Web dashboard UI today** with zero frontend work? Use **GoatCounter** (`< 100 MB` RAM) or **Plausible CE** (if you have 2–4 GB+ RAM).
- Need **multi-step funnels, path explorations, or marketing ad-network attribution right now**? Use **Plausible**, **Matomo**, or **GA4** until Rotameter `v2.0`.
- Need **session replays, heatmaps, or 100M+ events/day across a multi-node cluster**? Use **PostHog** or a ClickHouse-native stack.
