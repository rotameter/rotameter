# Feature-by-Feature Comparison

An honest engineering comparison between **Rotameter** and established self-hosted and cloud analytics engines. Rotameter capabilities are tagged explicitly by SemVer milestone (**`0.1.0`**, **`0.2.0`**, **`0.3.0`**, **`0.4.0`**, **`0.5.0`**, **`0.6.0`**, **`2.0.0`**) so planned work is never presented as already shipped.

*See [`COMPARISON-METRICS.md`](./COMPARISON-METRICS.md) for the metric-by-metric breakdown and formulas, [`ROADMAP.md`](./ROADMAP.md) for the milestone map, and [`../PROJECT.md`](../PROJECT.md) for engineering budgets.*

---

## 1. Runtime, Footprint & Operations

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Implementation stack** | Rust (`axum` + `rusqlite`) (`0.1.0`) | Go | Elixir / Phoenix | Next.js / Node.js | PHP | Google Cloud SaaS |
| **License** | `MIT OR Apache-2.0` (`0.1.0`) | MIT / EUPL | AGPL-3.0 | MIT | GPLv3 | Proprietary SaaS |
| **Deployment unit** | Single static binary (`musl` / macOS) (`0.1.0`) | Single Go binary | 3+ containers (App, Postgres, ClickHouse) | App + DB container | PHP + MySQL/MariaDB + cron | Managed SaaS only |
| **Minimum comfortable RAM** | **5 MB idle / 14 MB peak** (`< 40 MB` / `< 128 MB` budget on 1 GB VM) (`0.1.0`) | ~30–100 MB | 2–4 GB (ClickHouse OOM risk < 2 GB) | ~300–512 MB | 1–2 GB+ | N/A |
| **Multi-arch static releases** | `linux/amd64`, `linux/arm64`, `darwin` (`0.1.0`) | Linux/macOS/Windows | Docker (`amd64`/`arm64`) | Docker (`amd64`/`arm64`) | Tarball / Docker | N/A |
| **Self-hosting control** | Full (`0.1.0`) | Full | Full | Full | Full | None |
| **Horizontal / multi-node scale** | Single-node SQLite (`0.1.0`); `--role=ingest\|query` + DB salts (`0.2.0`); Postgres/ClickHouse (`0.5.0`); active-active Redis/Valkey (`0.6.0`); federation (`2.0.0`) | Single-node SQLite/PG | Multi-node ClickHouse | Postgres / MySQL | MySQL + Redis/Queue | Planet-scale |

---

## 2. Storage & Extensibility

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Embedded zero-ops DB** | Bundled SQLite WAL (`0.1.0`); opt-in DuckDB OLAP (`0.2.0`) | SQLite (or Postgres) | No (requires PG + CH) | No (requires PG/MySQL) | No (requires MySQL) | Managed |
| **Pluggable storage abstraction** | `Storage` traits (`0.1.0`); OLTP/OLAP split + DuckDB (`0.2.0`); PG/MySQL/CH/ADBC (`0.5.0`); Mongo/D1/DataFusion (`0.6.0`) | SQLite / PostgreSQL | Fixed PG + ClickHouse | Prisma (PG / MySQL) | Fixed MySQL/MariaDB | BigQuery export |
| **Portable open-format export / migrate** | Arrow/Parquet + JSON `.tar.zst` `backup`/`restore`/`migrate` (`0.2.0`) | SQLite dump | Volume / CH snapshots | DB dump | DB dump | BigQuery export |
| **Raw ordered per-session event log** | Yes (`events` + rollups) (`0.1.0`) | No (rollup counters only) | Yes (ClickHouse) | Yes (session/event rows) | Yes (`log_visit` / `log_link_visit_action`) | Yes (BigQuery export) |
| **Streaming object-store backups** | `VACUUM INTO` + Litestream → R2/S3 (`0.1.0`); live `POST /v1/admin/backup` (`0.2.0`); Arrow sinks (`0.5.0`) | SQLite file backup | Volume / CH snapshots | DB dump | DB dump | N/A |
| **Cardinality bomb protection** | Path/prop caps + daily `(other)` overflow (`0.1.0`) | Basic limits | CH memory limits | Minimal | Table growth risk | Automatic `(other)` row |
| **Webhooks & plugin extensibility** | Webhooks (`BeforeIngest`/`AfterFlush`), WASM, Flight/ADBC plugins (`0.5.0`) | No | No | No | PHP plugins | Tagging / Pub/Sub |

---

## 3. Privacy & Compliance

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Cookieless by default** | Yes (`0.1.0`) | Yes | Yes | Yes | Optional (cookies default) | No (uses cookies) |
| **Zero raw IP stored on disk** | Yes — `HMAC-SHA256` only (`0.1.0`) | Yes (8h rotating hash) | Yes (daily rotating salt) | Hash only | Truncation optional | Truncated/scrubbed in EU |
| **Salt rotation policy** | Daily + 30m grace default; per-site `daily`/`weekly`/`monthly` (`0.1.0`); DB-backed (`0.2.0`) | 8h rotating session hash | Fixed daily UTC salt | Monthly/salt-based | Configurable | N/A (client ID cookie) |
| **GDPR banner usually unnecessary** | Yes (`0.1.0`) | Yes | Yes | Yes | Only in cookieless mode | Usually required |
| **Zero third-party client requests** | First-party proxy / self-hosted (`0.1.0`) | Self-hosted | Self-hosted | Self-hosted | Self-hosted | Sends to Google |
| **Local GeoIP without external API** | `CF-IPCountry` header (0 MB) or `.mmdb` (`0.1.0`) | MaxMind / DB-IP | MaxMind / DB-IP | MaxMind / GeoLite | DB-IP / MaxMind | Google edge IP |
| **Privacy signal / opt-out** | `localStorage.rotameter_ignore` + optional `Sec-GPC` (`0.1.0`) | `localStorage` opt-out + DNT | `localStorage.plausible_ignore` | DNT optional | DNT + opt-out iframe | Opt-out browser addon |

---

## 4. Collection Methods & Integrations

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Lightweight JS tracker** | `735 B` gzip (`/v1/collect.js`, `<= 1.5 KB` cap) (`0.1.0`) | ~3.5 KB gzip `count.js` | `< 1 KB` gzip `plausible.js` | ~2 KB `script.js` | ~22 KB+ `matomo.js` | ~50 KB+ `gtag.js` |
| **1×1 zero-JS tracking pixel** | `/v1/pixel.gif` (`0.1.0`) | Yes (`/count`) | No (JS/API only) | No (JS/API only) | Yes (`matomo.php`) | Measurement Protocol |
| **Server-side HTTP ingest & batch** | `POST /v1/events` & `/batch` (`0.1.0`) | `POST /api/v0/count` | `POST /api/event` (single) | `POST /api/send` | Tracking HTTP API + bulk | Measurement Protocol |
| **First-class Astro / CF Workers SSR** | Reference proxy + beacon + `waitUntil()` helper (`0.1.0`) | Manual HTTP POST | Manual HTTP POST | Manual HTTP POST | Manual HTTP POST | Server-side GTM |
| **Server log importer & streaming** | CLI `rotameter import` (`0.1.0` JSONL/CLF; `0.2.0` `--stdin`, `--unix-socket`, `--follow`, `caddy-json`) | Built-in (`goatcounter import`) | No | No | Python log importer | No |
| **Google Tag Manager & CF Zaraz** | Packaged `.tpl` / sGTM / Zaraz templates (`0.6.0`, `RM-19`) | Manual custom HTML | Community docs | Community docs | Official templates | Native |

---

## 5. Interfaces & Developer Experience

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Clean typed Query HTTP API** | `POST /v1/query` + CSV (`0.1.0`) | `/api/v0/stats/*` | Stats API v2 | REST API (paid on Cloud) | Reporting API | GA Data API |
| **Idempotent IaC / OpenTofu Admin API** | `PUT /v1/admin/sites/{id}` (`0.1.0`) | Basic site endpoints | Provisioning API (Enterprise) | Teams/websites API | API (`SitesManager`) | Admin API |
| **First-class CLI (`query`, `backup`, `doctor`)** | Yes (`0.1.0` + `0.2.0` `restore`/`migrate`) | Server/import CLI | Minimal mix tasks | DB scripts | `console` PHP CLI | None (gcloud/API) |
| **Built-in MCP server (AI agents)** | `stdio` read-only (`0.1.0`); Streamable HTTP `/mcp` + `--allow-admin` (`0.3.0`, `RM-20`) | None | None | External community MCP | None | Official / community MCP |
| **Interactive Terminal UI (TUI)** | `ratatui` block chart + drill-down (`0.1.0`) | None | None | None | None | None |
| **Built-in Web Dashboard UI** | Static `--ui=on\|off` backend + `/v1/capabilities` (`0.3.0` `RM-37`); UI shell & plugins (`0.3.0` `RM-38`..`RM-40`) | Yes (HTML templates) | Yes (React/Tailwind) | Yes (Next.js) | Yes (Vue/PHP) | Yes |
| **Admin auth, OIDC & Multi-org RBAC** | Scoped API keys (`0.1.0`); admin/OIDC/CF Access cookies (`0.3.0` `RM-37`); multi-org RBAC (`0.6.0` `RM-29`) | Basic logins | Yes (teams/roles) | Yes (teams/roles) | Granular RBAC | Google IAM roles |

---

## 6. Product Analytics & Enterprise Features

| Feature | Rotameter | GoatCounter | Plausible CE | Umami | Matomo (On-Prem) | Google Analytics 4 |
| --- | --- | --- | --- | --- | --- | --- |
| **Custom events & JSON properties** | Key-value props (`0.1.0`) | Limited event paths | Custom events + props | Custom events + data | Events (`category/action/name`) | Event parameters |
| **Outbound links, 404s & downloads** | Built into `collect.js` (`0.1.0`) | Event/path conventions | Script extensions | Manual / custom events | Built-in | Enhanced measurement |
| **Compound filter AST (`AND`/`OR`/`NOT` + regex)** | `eq`/`neq`/`in`/`contains`/`prefix` (`0.1.0`); compound AST + regex (`0.4.0`, `RM-21`) | Basic search | Filter arrays (partial OR) | Basic filters | Segments | Comparisons & Explorations |
| **Multi-step conversion funnels** | Ordered & unordered (`0.4.0`, `RM-21`) | No | Yes (Enterprise / CE recent) | Yes (Pro / v2+) | Paid plugin / enterprise | Funnel Exploration |
| **User flows / Path transitions** | Flows & live streams (`0.4.0`, `RM-21`) | No | No | User Journeys | Transitions plugin | Path Exploration |
| **Retention cohorts** | Retention cohorts (`0.4.0`, `RM-21`) | No | No | Retention report | Cohorts plugin | Cohort Exploration |
| **Click/scroll heatmaps (cookieless)** | Opt-in module + UI plugin (`0.4.0`, `RM-21`) | No | No | No | Paid plugin | No |
| **Session replay** | Out of scope (privacy & bloat boundary) | No | No | No | Paid plugin | No |

---

## 7. Candid Positioning

**Where Rotameter wins (`0.1.0`–`0.2.0` shipped):**
- You want **Plausible-caliber session metrics and structured querying** on a **\$0 `1 vCPU / 1 GB` VM** (`5 MB` idle / `14 MB` peak RSS) without babysitting ClickHouse or PostgreSQL.
- You want a **headless, inspectable analytics instrument** built for static/edge sites (Astro + Cloudflare Workers), streaming proxy logs (Apache/Nginx/Caddy), CLI scripts, AI workflows (`rotameter mcp` / `/mcp`), and terminal operators (`rotameter tui`).
- You want **clean-sheet event-stream data + portable Arrow/Parquet exchange on day 1** so adding the Web UI (`0.3.0`), funnels/heatmaps (`0.4.0`), or DuckDB/Postgres/ClickHouse (`0.2.0`/`0.5.0`) never forces a migration.

**When another tool is the better fit:**
- Need a polished **Web dashboard UI right now** before `RM-38`..`RM-40` land? Use **GoatCounter** (`< 100 MB` RAM) or **Plausible CE** (if you have 2–4 GB+ RAM).
- Need **multi-step funnels, path explorations, or heatmaps right now**? Use **Plausible**, **Matomo**, or **GA4** until Rotameter `0.4.0`.
- Need **DOM session replays or 100M+ events/day across a multi-node cluster today**? Use **PostHog** or a ClickHouse-native stack.
