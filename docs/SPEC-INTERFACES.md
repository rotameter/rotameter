# Interfaces Specification: CLI, MCP Server & TUI (`0.1.0+`)

Because Rotameter ships headless-first (`0.1.0`–`0.2.0`), the **CLI**, **MCP server**, and **TUI** are first-class operator interfaces sharing `rotameter-client` and a single configuration model.

*Related specs: [`SPEC-API.md`](./SPEC-API.md) · [`SPEC-ARCHITECTURE.md`](./SPEC-ARCHITECTURE.md) · [`SPEC-OPERATIONS.md`](./SPEC-OPERATIONS.md)*

---

## 1. Shared Configuration & Authentication Resolution

All client-facing commands (`query`, `realtime`, `site`, `key`, `mcp`, `tui`) resolve endpoint credentials in priority order:
1. CLI flags (`--url`, `--token`, `--profile`, `--access-id`, `--access-secret`)
2. Environment variables:
   - `ROTAMETER_URL` (e.g., `https://analytics.example.com`)
   - `ROTAMETER_TOKEN` (`rm_rd_...` or `rm_adm_...`)
   - `CF_ACCESS_CLIENT_ID` / `CF_ACCESS_CLIENT_SECRET` (sent automatically on every request when present)
3. Profile file `~/.config/rotameter/config.toml` (mode `0600`):
   ```toml
   default_profile = "prod"

   [profiles.prod]
   url = "https://analytics.example.com"
   site = "example.com"
   token_env = "ROTAMETER_TOKEN"
   cf_access_client_id_env = "CF_ACCESS_CLIENT_ID"
   cf_access_client_secret_env = "CF_ACCESS_CLIENT_SECRET"
   ```

---

## 2. CLI (`rotameter`)

Uniform flags across operational subcommands: `--format table|json|csv`, `--profile <name>`, `--quiet`.
All `<site>` arguments across CLI, MCP, and TUI resolve either an immutable `site_id` (`site_...`) or the site's unique `slug` within the active org/profile (`ADR-0007`).
Standard exit codes: `0` OK, `1` runtime/HTTP error, `2` invalid CLI args, `3` auth/Access error, `4` health/doctor gate failure.

### Subcommand Tree (`0.1.0`–`0.3.0`)

| Command | Mode | Purpose |
| --- | --- | --- |
| `rotameter serve` | Local DB | Start HTTP server (`--bind 127.0.0.1:8000`, `--control-db`, `--analytics-db`, `--role`, `--salt-store`, `--enable-mcp-http`, `--mcp-allow-admin`, `--ui`, `--ui-dir`, `--public-url`, `--no-self-site`, `--geoip`, `--trusted-proxy`) |
| `rotameter site upsert <site>` | Local DB or Remote Admin API | Idempotently create/update site (`--slug`, `--name`, `--timezone`, `--salt-rotation`, `--respect-gpc`, `--origin`, `--allow-query`, `--retention-days`) |
| `rotameter site list / show / purge / delete` | Local or Remote | Inspect sites, prune old raw events, or delete site |
| `rotameter key create / list / revoke` | Local or Remote | Mint scoped keys (`--scope ingest\|read\|admin`, `--site <id>`, `--label`) and revoke |
| `rotameter query <site>` | Remote API | Query summary, timeseries, and breakdowns (`--range 24h\|7d\|30d\|12m\|all`, `--group-by path`, `--filter country=US`, `--compare`, `--format csv`) |
| `rotameter realtime <site>` | Remote API | Stream/print 5-minute active visitors and active pages (`--watch`) |
| `rotameter import <site> <file>` | Local DB or Remote Batch API | Backfill or stream Apache/Nginx/Caddy logs (`--stdin`, `--unix-socket`, `--follow`, `--input-format clf\|jsonl\|caddy-json`, `--dry-run`) |
| `rotameter seed` | Local DB | Synthesize repeatable multi-day diurnal curves, breakdowns, and live pulse for local previews |
| `rotameter backup <dest>` | Local DB | Execute safe online `SQLite VACUUM INTO <dest>` snapshot or `--format portable` Arrow/Parquet `.tar.zst` bundle (`--features arrow`) |
| `rotameter restore <src>` | Local DB | Restore a verified portable Arrow/Parquet directory or `.tar.zst` archive (`--plane all\|control\|analytics`) |
| `rotameter migrate` | Local DB | Stream data between storage DSNs (`sqlite://`, `duckdb://`) via Arrow or `--stage-dir` |
| `rotameter doctor` | Local or Remote | Check DB WAL integrity, pragma ceilings, salt/clock health, queue usage, or remote `/v1/health` + Access token auth |
| `rotameter mcp` | Remote API (`stdio`) | Start Model Context Protocol server over `stdin`/`stdout` (read-only by default; opt-in `--allow-admin` in `0.3.0`) |
| `rotameter tui` | Remote API | Launch interactive `ratatui` flight-instrument dashboard |

**Server self-site flags (`rotameter serve`):** First boot idempotently provisions a local preconfigured self-site (`--public-url` host or `_self`, override via `--self-site-id` / `ROTAMETER_SELF_SITE_ID` / `[server] self_site_id`), controllable with precedence `CLI > env > config` and disabled cleanly via `--no-self-site` (`--self-site=false`), `ROTAMETER_SELF_SITE=false`, or `[server] self_site = false`.

---

## 3. MCP Server (`rotameter mcp` & HTTP `/mcp`)

### `0.1.0` Baseline: `stdio` Transport, Read-Only by Default
By default, `rotameter mcp` speaks MCP JSON-RPC 2.0 over `stdio` and exposes the 7 read-only analytics tools (`rm_rd_` or `rm_adm_`)—mutations stay disabled unless explicitly opted in.

### Tool Catalog (`0.1.0` Read-Only)

| Tool Name | Input Schema Highlights | Output |
| --- | --- | --- |
| `rotameter_sites` | `{}` | Registered site IDs, timezones, and configured `salt_rotation` |
| `rotameter_summary` | `{ site_id, range, filters?, compare_previous? }` | Headline KPIs (`visitors`, `pageviews`, `visits`, `views_per_visit`, `bounce_rate`, `avg_visit_duration_ms`) + % deltas |
| `rotameter_timeseries` | `{ site_id, range, granularity?, metrics?, filters? }` | Timestamped bucket array matching the chosen granularity |
| `rotameter_breakdown` | `{ site_id, dimension, range, filters?, limit? }` | Ranked rows for `path`, `source`, `channel`, `country`, `region`, `browser`, `os`, `device`, `event_name`, or `utm_*` |
| `rotameter_realtime` | `{ site_id }` | Last 5-min active visitors + active pages + 30-min minute pulse |
| `rotameter_compare_periods` | `{ site_id, current: {from, to}, previous: {from, to}, dimension? }` | Side-by-side absolute and relative deltas across two explicit windows |
| `rotameter_query` | Full `POST /v1/query` typed object | Composable escape-hatch query with explicit `query_ms` and truncation flags |

### `0.3.0` Streamable HTTP `/mcp` & Opt-In Admin Tools (`RM-20`)
- Optional Streamable HTTP `/mcp` route on `rotameter serve` (`--enable-mcp-http` / `ROTAMETER_ENABLE_MCP_HTTP`) authenticated via `rm_rd_` or `rm_adm_` Bearer token (`405` on `GET`/`DELETE`, `202` on notifications).
- Opt-in `--allow-admin` (`rotameter mcp`) / `--mcp-allow-admin` (`rotameter serve`) exposing 5 guarded admin tools (`rotameter_site_upsert`, `rotameter_site_delete` with `confirm: true`, `rotameter_key_create`, `rotameter_key_list`, `rotameter_key_revoke`) when paired with an `rm_adm_` token.

---

## 4. Terminal UI (`rotameter tui`)

A crisp, high-density `ratatui` flight-instrument dashboard consuming the remote `/v1` API (with graceful non-blocking background polling every `30s` and manual refresh).

### Screen Anatomy
```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ ROTAMETER // example.com       [1:24H] [2:7D] [*3:30D*] [4:12M] [5:ALL]  ● 3 LIVE │
├───────────────────────────┬──────────────────────────────────────────────────┤
│ VISITORS       1,842 +22% │  ▅                                               │
│ PAGEVIEWS      4,190 +27% │  █   ▆       ▇   ▅                               │
│ VISITS         2,110 +24% │  █ ▄ █ ▂ ▅ ▃ █ ▆ █ ▄ █ ▇ ▃ ▆ █ ▅ ▇ ▄ ▆ █ ▃ ▅ ▇ ▓ │
│ VIEWS/VISIT     1.99  +2% │  2026.10.22 ─────────────────────▶ TODAY (11.20) │
│ BOUNCE RATE    48.2%  -3% │  CURSOR: 2026.11.14 · 112 VISITORS · 248 VIEWS   │
│ AVG DURATION   01m34s +7% │  ACTIVE FILTER: [country=US] (Esc to clear)      │
├───────────────────────────┴──────────────────┬───────────────────────────────┤
│ BREAKDOWN [Tab]: *PAGES* | SOURCES | GEO | TECH | EVENTS │  SHARE    VISITORS│
│ > /projects/rotameter                        │  22.4%       412              │
│   /posts/zero-hydration-edge                 │  16.1%       297              │
│   /                                          │  14.8%       273              │
│   /ledger                                    │   8.9%       164              │
└──────────────────────────────────────────────┴───────────────────────────────┘
│ [s] site  [1-5] range  [←/→] scrub bar  [Tab] panel  [Enter] filter  [?] help│
└──────────────────────────────────────────────────────────────────────────────┘
```

### Keybindings & ASCII Block-Chart Spec (`0.1.0+`)
- **`1`–`5`**: Switch window (`24H`, `7D`, `30D`, `12M`, `ALL`).
- **`←` / `→` (or `h` / `l`)**: Scrub across the **ASCII block chart** columns (`░▒▓█` / dense cap glyph `▀`/`█`, today's bar highlighted in accent tone); dynamically rolls the headline readout to the scrubbed bucket's date, visitors, and pageviews.
- **`↑` / `↓` (or `j` / `k`) + `Enter`**: Select a row in the active breakdown (`PAGES`, `SOURCES`, `GEO`, `TECH`, `EVENTS`) and push it onto the active filter bar (`[country=US]`), immediately re-querying all panels.
- **`Esc`**: Pop/clear the active drill-down filter.
- **`Tab` / `Shift+Tab`**: Cycle breakdown tabs (`PAGES`, `SOURCES`, `GEO`, `TECH`, `EVENTS`).
- **`s`**: Open site switcher overlay across accessible sites.
- **`r`**: Force immediate refresh; **`?`**: Keybinding reference; **`q` / `Ctrl-C`**: Clean exit.
