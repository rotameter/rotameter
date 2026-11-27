# Web UI Architecture & Design Specification (`0.3.0`–`0.5.0`)

Rotameter ships an optional, zero-runtime-Node static Web UI (`ui/dist/`) built for `1 vCPU / 1 GB RAM` self-hosted deployments while preserving clean seams for `0.4.0` product analytics and `0.5.0` multinode clusters.

*Related specs: [`SPEC-API.md`](./SPEC-API.md) · [`SPEC-ARCHITECTURE.md`](./SPEC-ARCHITECTURE.md) · [`SPEC-INTERFACES.md`](./SPEC-INTERFACES.md) · [`ADR-0009`](./decisions/0009-web-ui-architecture-and-design-system.md)*

---

## 1. Core Stack, Workspace Layout & Hosting Model

- **Monorepo (`Bun` workspaces + `Vite+` under `ui/`):**
  - `ui/apps/web` — Vite+ (`vite-plus`) + React + TypeScript (`strict: true`) host shell, router, capability boot, and auth gate.
  - `ui/packages/ui` (`@rotameter/ui`) — owned component library wrapping `@base-ui/react` primitives + `tokens.css`.
  - `ui/packages/plugin-sdk` (`@rotameter/plugin-sdk`) — typed `PluginManifest`, `useDashboardQuery()`, `updateQuery()`, `ChartHost`, capability/auth context, and scoped API client.
  - `ui/packages/plugin-*` — lazy-loaded feature modules (`overview`, `breakdowns`, `realtime`, `admin`, `cluster` in `0.3.0`; `funnels`, `heatmaps`, `streams`, `builder` in `0.4.0`).
- **Styling & Primitives:**
  - Tailwind CSS v4 paired with semantic CSS custom properties (`tokens.css`); zero CSS-in-JS runtime overhead.
  - **Two-layer primitive hybrid:** `@base-ui/react` (headless, zero bundled CSS) wrapped in `@rotameter/ui`, plus `components.json` configured for Base UI-backed shadcn recipes re-skinned exclusively with `--*` tokens for complex scaffolds (command palettes, drawers, multi-select comboboxes).
- **Packaging & Serving (`rotameter serve`):**
  - Static `ui/dist/` bundle supports both standalone static hosting (CDN / reverse proxy) and embedded binary assets.
  - Cargo feature `ui` embeds pre-compressed `ui/dist/` (`--ui=on|off`, default `off`), paired with `--ui-dir <path>` runtime override so headless Rust builds never require Bun.
- **Budgets:** Initial JS+CSS load `<= 250 KB` gzip (`0.4.0` plugins cost `0 KB` on `0.3.0` initial load); total compressed embedded `ui/dist/` `<= 1.5 MB`.

---

## 2. Charting, Virtualized Tables & Live Streams

### 2.1 Hybrid Charting + Deferred WebGL Policy
- **`0.3.0`:** `uPlot` (~50 KB min) for high-speed time-series and 30-minute realtime pulse, paired with owned HTML/SVG bars inside TanStack Table. **Zero Apache ECharts in `0.3.0`.**
- **`0.4.0`:** Owned SVG / Canvas 2D for multi-step funnels and click/scroll/matrix heatmaps; lazy-loaded tree-shaken Apache ECharts modules are permitted *only* inside `0.4.0` plugin chunks when complex flow/matrix layouts justify them.
- **Deferred WebGL Policy:** Custom WebGL / `*-gl` renderers are explicitly deferred across `0.3.0` and `0.4.0` to minimize bundle weight and avoid GPU/driver overhead on `1 vCPU / 1 GB RAM` VMs. All chart components render through `ChartHost` so a lazy WebGL renderer can be registered later if sustained high-rate stream/density workloads require it.

### 2.2 Virtualized Tables (`@tanstack/react-table` + `@tanstack/react-virtual`)
- Breakdown and event tables use TanStack Table + TanStack Virtual.
- Virtualizer `estimateSize` must read `--density-table-row-h` dynamically via `getComputedStyle` + a `MutationObserver` watching `data-density` on `:root` / container elements, giving `O(1)` scroll offset math without DOM measurement thrashing.

### 2.3 Live Event Streams (`0.4.0` Freeze-on-Interact)
- Live event streams automatically pause auto-scroll when the operator scrolls away from top, moves keyboard focus onto a row, or opens an inspector drawer.
- Paused streams display a non-intrusive `+N new events · Resume` badge so keyboard focus and screen readers are never hijacked.

---

## 3. Three-Bucket State Separation & URL Sync

1. **Analytical Query State (Shareable URL Search Params):**
   - Managed via `@tanstack/react-router` with schema-validated search params.
   - Canonical URL shape:
     `/sites/:siteId/overview?range=7d&interval=1h&compare=prev&f=country:US,path:/blog/*`
   - **History discipline:** Use `push` for discrete analytical pivots (changing site, time preset, route, or adding/removing a filter pill) and `replace` for continuous adjustments (chart brush zoom, cursor scrub, search input typing).
2. **Server / Telemetry State (`@tanstack/react-query`):**
   - Keyed directly off the validated URL query object:
     `['metrics', siteId, range, interval, compare, filters]`
   - Conservative `staleTime` (`30s`–`60s` for historical queries; `5s`–`15s` for realtime).
   - Pass `AbortSignal` to every `fetch` so rapid filter/range changes cancel in-flight backend queries on `1 vCPU` VMs.
3. **UI & Device Preferences (`localStorage`):**
   - `theme` (`light | dark | system`), `density` (`compact | standard | comfortable`), sidebar collapse state, and live-stream pause state persist in `localStorage` without polluting shared URLs.

---

## 4. API, Auth & Single-to-Multinode Topology Contract

All UI endpoints live under `/v1/*` and use `snake_case` JSON (`SPEC-API.md`).

### 4.1 Boot Capability Handshake (`GET /v1/capabilities`)
```json
{
  "topology": "single",
  "node_role": "all",
  "features": { "ui": true, "arrow_export": false, "oidc": false, "streams": false },
  "auth_methods": ["session", "bearer", "cf_access"],
  "endpoints": { "query": "/v1", "stream": "/v1" }
}
```
- When connected to an ingest-only node (`node_role === "ingest"`), the UI gracefully gates query-dependent routes and presents the Node Role Telemetry status view instead of failing on `405`/`501`.
- Host SDK exposes configurable `query_base_url` and `stream_base_url` (defaulting to same-origin) so `0.4.0` SSE/WebSocket streams can target an ingest-tap or coordinator endpoint.

### 4.2 Query Response Metadata & Telemetry Strip
- Client wrappers parse existing `meta.query_ms` plus optional cluster fields on `POST /v1/query`: `watermark_ts`, `partial_nodes`, and `served_by_node`.
- Surfaced in a compact header telemetry strip: `SINGLE-NODE · 4.2ms` (`0.3.0`) expanding to `CLUSTER · 3 NODES · LAG 120ms` or amber `PARTIAL SHARD` (`0.5.0`).

### 4.3 Admin Web UI Authentication (`0.3.0`, `/v1/auth/*`)
- Endpoints: `POST /v1/auth/login`, `POST /v1/auth/logout`, `GET /v1/auth/me`, `GET /v1/auth/oidc/start`, `GET /v1/auth/oidc/callback`.
- Local admin login and OIDC/SSO issue `HttpOnly; SameSite=Lax; Secure` operator session cookies with CSRF protection, alongside optional Cloudflare Access header/JWT trust and `rm_adm_`/`rm_rd_` Bearer passthrough.
- **Privacy boundary:** Admin session cookies authenticate dashboard operators only. Visitor collection (`collect.js`, `pixel.gif`, `/v1/events`) remains **100% cookieless**.

---

## 5. Two-Tier Plugin & Extensibility Architecture

### 5.1 Tier 1 — First-Party Workspace Modules (`0.3.0`–`0.4.0`)
- Compiled in `ui/packages/plugin-*` and lazy-loaded via Vite dynamic `import()` (`0 KB` initial load for `0.4.0` modules in `0.3.0`).
- **Extension points:** `routes` / `nav_items`, `dashboard_panels`, `visualizations` (keyed by `viz_type`), `inspector_tabs` / `row_actions`, and `query_transforms` / `exporters`.
- **Hard guardrails:**
  - Read/mutate analytical state exclusively via `useDashboardQuery()` / `updateQuery()` (never raw `window.location`).
  - Style exclusively via semantic `--*` CSS custom properties and compose `@rotameter/ui` primitives.
  - Declare required API scopes (`read:aggregates`, `read:events`, `write:dashboards`, `admin:sites`, `admin:keys`) in `PluginManifest`.

### 5.2 Tier 2 — Third-Party Runtime Plugins (`0.5.0+`, `RM-24`)
- Deferred to `0.5.0+` alongside backend plugin seams (`RM-24`).
- Will load `/plugins/<id>/entry.js` (or allowlisted URLs) via typed `PluginManifest`, sharing `react`, `react-dom`, `@rotameter/ui`, and `@rotameter/plugin-sdk` as host singletons (import maps / `window.ROTAMETER_SDK`), with an optional sandboxed `iframe` isolation mode for untrusted scripts.

---

## 6. Visual System, CSS Tokens & WCAG 2.2 AA

### 6.1 CAD / Flight-Instrument Design Rules
- Crisp `1px` hairline structural grids (`gap: 1px; background: var(--border-hairline)`).
- Zero blurred SaaS drop-shadows; sharp radii (`0px` split panes, `2px` panels/controls, `4px` popovers).
- Tabular monospace telemetry (`font-variant-numeric: tabular-nums`) and optional pure-CSS `6px` corner reticle ticks on primary instrument frames.

### 6.2 Exact CSS Custom Properties (`tokens.css`)
```css
:root {
  --font-sans: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --font-mono: "JetBrains Mono", "IBM Plex Mono", ui-monospace, SFMono-Regular, monospace;

  --text-xs:   0.6875rem; /* 11px */
  --text-sm:   0.75rem;   /* 12px */
  --text-base: 0.8125rem; /* 13px */
  --text-md:   0.9375rem; /* 15px */
  --text-lg:   1.25rem;   /* 20px */
  --text-xl:   1.75rem;   /* 28px */

  --radius-none: 0px;
  --radius-xs:   2px;
  --radius-sm:   4px;
}

/* Light Mode: Drafting Vellum (WCAG AA Verified) */
:root[data-theme="light"] {
  --bg-canvas:       #F4F5F7;
  --bg-surface:      #FFFFFF;
  --bg-elevated:     #EBEEF2;
  --bg-sunken:       #E3E7ED;
  --fg-primary:      #0F1419;
  --fg-secondary:    #3D4754;
  --fg-muted:        #596575;
  --border-hairline: #CBD2DC;
  --border-strong:   #7A8699;
  --accent-primary:  #0052CC;
  --accent-fg:       #FFFFFF;
  --signal-ok:       #116932;
  --signal-warn:     #8F4700;
  --signal-crit:     #B42318;
  --focus-ring:      #0052CC;

  --series-1: #0052CC;
  --series-2: #C2410C;
  --series-3: #0F766E;
  --series-4: #7E22CE;
  --series-5: #A16207;
  --series-6: #334155;
}

/* Dark Mode: Avionics Night (WCAG AA Verified) */
:root[data-theme="dark"] {
  --bg-canvas:       #0B0E11;
  --bg-surface:      #12161C;
  --bg-elevated:     #1A2029;
  --bg-sunken:       #080A0D;
  --fg-primary:      #F0F4F8;
  --fg-secondary:    #A3B1C2;
  --fg-muted:        #7D8B9D;
  --border-hairline: #262F3D;
  --border-strong:   #4B586E;
  --accent-primary:  #4EA1FF;
  --accent-fg:       #08111E;
  --signal-ok:       #38C172;
  --signal-warn:     #F5A623;
  --signal-crit:     #FF6B6B;
  --focus-ring:      #66B2FF;

  --series-1: #4EA1FF;
  --series-2: #FB923C;
  --series-3: #2DD4BF;
  --series-4: #C084FC;
  --series-5: #FACC15;
  --series-6: #94A3B8;
}

/* Tiered Density Scales (Root and Container Overridable) */
[data-density="compact"] {
  --density-scale:       0.85;
  --density-grid-gap:    6px;
  --density-panel-pad:   8px 10px;
  --density-control-h:   26px;
  --density-table-row-h: 26px;
  --density-font-body:   0.75rem;   /* 12px */
  --density-icon-size:   14px;
}

[data-density="standard"] {
  --density-scale:       1;
  --density-grid-gap:    10px;
  --density-panel-pad:   12px 14px;
  --density-control-h:   32px;
  --density-table-row-h: 34px;
  --density-font-body:   0.8125rem; /* 13px */
  --density-icon-size:   16px;
}

[data-density="comfortable"] {
  --density-scale:       1.15;
  --density-grid-gap:    16px;
  --density-panel-pad:   16px 20px;
  --density-control-h:   38px;
  --density-table-row-h: 42px;
  --density-font-body:   0.875rem;  /* 14px */
  --density-icon-size:   18px;
}
```

### 6.3 WCAG 2.2 AA Verification Checklist
- **Contrast:** `>= 4.5:1` for all text/icons (`>= 7:1` AAA on primary KPIs); `>= 3:1` on non-text borders and focus rings.
- **Focus:** Visible `2px solid var(--focus-ring)` with `2px` offset on `:focus-visible` across all interactive controls.
- **Use of Color (WCAG 1.4.1):** Multi-series charts combine colorblind-safe `--series-*` tokens with distinct stroke dash patterns (`solid`, `6 3`, `2 2`) or point markers, plus a keyboard-accessible tabular data fallback view on every chart.
- **Target Size (WCAG 2.5.8):** Interactive controls maintain `>= 24×24px` target area with `>= 4px` spacing even in `compact` density mode.

---

## 7. Milestone Scope Matrix

| Version | Modules & Scope |
| --- | --- |
| **`0.3.0`** | App shell, `/v1/capabilities` handshake, `/v1/auth/*` (local admin, OIDC, CF Access), theme/density switcher, URL query bar, `@rotameter/plugin-overview` (KPI strip + `uPlot` time-series), `@rotameter/plugin-breakdowns` (Pages, Referrers, UTM/Cohorts, Devices/Geo + CSV/Parquet export), `@rotameter/plugin-realtime` (5m active + 30m pulse), `@rotameter/plugin-admin` (sites & API keys), `@rotameter/plugin-cluster` (single-node health & role status). |
| **`0.4.0`** | `@rotameter/plugin-funnels` (multi-step ordered/unordered), `@rotameter/plugin-heatmaps` (click/scroll/matrix), `@rotameter/plugin-streams` (live event stream with freeze-on-interact + `stream_base_url`), `@rotameter/plugin-builder` (configurable dashboard/query builder consuming Plugin SDK visualizations). |
| **`0.5.0`** | `@rotameter/plugin-cluster` multinode roster & replication lag when `topology === "cluster"` (`RM-22`); Tier 2 third-party runtime UI plugin loader & iframe sandbox (`RM-24`). |
