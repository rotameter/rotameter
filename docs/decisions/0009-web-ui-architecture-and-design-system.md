---
status: accepted
date: 2026-11-26
decision-makers: Colin Wolff (owner)
---

# Adopt Bun-workspace static React UI, CAD flight-instrument token system, feature-gated embedding, and tiered plugin seams

## Context and Problem Statement

Rotameter `0.1.0`–`0.2.0` established a headless Rust analytics engine (`< 40 MB` idle RSS, `< 128 MB` peak RSS, `< 25 MB` static binary) and clean OLTP/OLAP + Arrow + `--role` seams (`ADR-0008`). Ahead of implementing `0.3.0` (`RM-18`) and `0.4.0` (`RM-21`), we need a frontend architecture that matches the precision CAD / flight-instrument ergonomics of `rotameter tui`, ships as a zero-Node-runtime static bundle, keeps `0.3.0` lean on `1 vCPU / 1 GB RAM` VMs, and preserves clean module boundaries for `0.4.0` builders/streams and `0.5.0` multinode clusters.

## Decision Drivers

* Keep `rotameter` self-hostable on `1 vCPU / 1 GB RAM` VMs with zero Node/Bun server runtime at serving time and zero UI asset overhead when `--ui=off` (or compiled without the `ui` Cargo feature).
* Enforce strict compile-time dependency boundaries between host shell, UI primitives, SDK, and lazy-loaded feature plugins (`0 KB` initial load for `0.4.0` modules in `0.3.0`).
* Align every UI endpoint and JSON payload with `SPEC-API.md` (`/v1/*`, `snake_case`) while keeping visitor tracking 100% cookieless (`ADR-0004`).
* Guarantee WCAG 2.2 AA accessibility (`>= 4.5:1` contrast, `2px` `:focus-visible` rings, non-color chart encoding + tabular fallback, `>= 24×24px` interactive targets even in compact density).
* Avoid premature WebGL, ECharts bloat, or third-party runtime plugin ABI lock-in during `0.3.0`.

## Considered Options

* Bun-workspace monorepo (`@rotameter/ui`, `@rotameter/plugin-sdk`, lazy `@rotameter/plugin-*`) + Tailwind v4/`tokens.css` + `@base-ui/react` + TanStack Router/Query/Table/Virtual + `uPlot` + feature-gated Cargo `ui` embedding + Tier 2 runtime loader deferred to `0.5.0+`
* Single monolithic SPA without workspace package boundaries and with full ECharts + WebGL bundled upfront
* Full Tier 2 third-party runtime `/plugins/*/entry.js` + iframe sandbox shipped immediately in `0.3.0`

## Decision Outcome

Chosen option: "Bun-workspace monorepo + Tailwind v4/`tokens.css` + `@base-ui/react` + TanStack stack + `uPlot` + feature-gated Cargo `ui` embedding + Tier 2 runtime loader deferred to `0.5.0+`", because it mirrors Rotameter's modular Rust workspace discipline while keeping `0.3.0` fast, small, accessible, and strictly aligned with `/v1/*`.

1. **Bun Workspace Monorepo (`ui/`):** Organize `ui/apps/web` (Vite + React + strict TypeScript), `ui/packages/ui` (`@rotameter/ui`), `ui/packages/plugin-sdk` (`@rotameter/plugin-sdk`), and lazy-loaded `ui/packages/plugin-*` (`overview`, `breakdowns`, `realtime`, `admin`, `cluster` in `0.3.0`; `funnels`, `heatmaps`, `streams`, `builder` in `0.4.0`).
2. **Design System & Primitives:** Tailwind CSS v4 + semantic `tokens.css` (`Drafting Vellum` light, `Avionics Night` dark, 3 density modes) wrapping `@base-ui/react` inside `@rotameter/ui`, with `components.json` Base UI-backed shadcn recipes as a token-skinned escape hatch. Zero hardcoded colors, shadows, or pixel row heights.
3. **Charting, Virtualization & Deferred WebGL:** `0.3.0` uses `uPlot` + owned HTML/SVG bars (zero ECharts). `0.4.0` uses owned SVG/Canvas 2D and allows lazy-loaded tree-shaken ECharts only inside `0.4.0` plugin chunks if justified. Custom WebGL / `*-gl` layers remain deferred behind `ChartHost`. Tables use `@tanstack/react-table` + `@tanstack/react-virtual` reading `--density-table-row-h` dynamically via `getComputedStyle` + `MutationObserver`.
4. **State, `/v1/*` Capabilities & Admin Auth:** Analytical query state lives in `@tanstack/react-router` search params (`push` vs `replace` discipline) driving `@tanstack/react-query` (`staleTime` + `AbortSignal`), while UI preferences live in `localStorage`. Boot calls `GET /v1/capabilities` (`snake_case` `topology`, `node_role`, `features`, `auth_methods`, `endpoints`) and gracefully gates ingest-only nodes. Operator auth uses `/v1/auth/*` (`HttpOnly; SameSite=Lax; Secure` session cookies + CSRF, OIDC, CF Access / Bearer passthrough) while visitor collection stays 100% cookieless.
5. **Feature-Gated Rust Packaging:** Cargo feature `ui` embeds pre-compressed `ui/dist/` (`<= 1.5 MB` compressed, `<= 250 KB` gzip initial load) toggled via `--ui=on|off` (default `off`), alongside `--ui-dir <path>` override so headless Rust builds require neither Bun nor embedded assets.
6. **Tier 2 Runtime Plugins Deferred (`0.5.0+`):** Third-party runtime JS plugins (`/plugins/<id>/entry.js`, host singletons, optional iframe sandbox) wait for `0.5.0+` (`RM-24`) once `0.3.0`–`0.4.0` extension hooks stabilize.

### Consequences

* Good, because `0.3.0` ships a crisp, accessible, sub-250 KB gzip dashboard with zero Node runtime at serving time and zero overhead on `--ui=off` or headless builds.
* Good, because workspace packages enforce SDK and token boundaries at compile time and isolate `0.4.0` builders/streams at `0 KB` initial cost.
* Good, because `/v1/capabilities` and `POST /v1/query` `meta` fields bridge cleanly from single-node `0.3.0` (`--role`) to `0.5.0` multinode clusters (`RM-22`).
* Bad, because maintaining a Bun workspace + Rust feature-gated embedding pipeline adds modest CI/release packaging complexity.

### Confirmation

* `chalkline docs check -d .` validates `SPEC-UI.md`, `DESIGN.md`, and canonical docs under 500 lines.
* `RM-37`, `RM-38`, `RM-39`, and `RM-40` verify backend `/v1/capabilities` + `/v1/auth/*` + Cargo `ui` embedding, Bun workspace foundation, core `0.3.0` plugins, and WCAG 2.2 AA + bundle-budget gates (`<= 250 KB` gzip initial load, `<= 1.5 MB` embedded `ui/dist/`).

## Pros and Cons of the Options

### Bun workspace monorepo + lazy Tier 1 modules + deferred Tier 2

* Good, enforces `@rotameter/plugin-sdk` and `tokens.css` boundaries without premature runtime plugin ABI lock-in.
* Bad, requires workspace `package.json` / `tsconfig` wiring upfront.

### Monolithic SPA with ECharts + WebGL upfront

* Bad, inflates initial bundle and RAM/GPU footprint on `1 vCPU / 1 GB RAM` VMs and leaks internal app state into feature views.

### Tier 2 runtime JS/iframe loader in `0.3.0`

* Bad, locks in an untested external plugin API and adds CSP/XSS/iframe bridge surface before `0.4.0` extensions exist.

## More Information

* Storypole: `RM-18` (`RM-37`, `RM-38`, `RM-39`, `RM-40`), `RM-21` (`0.4.0`), `RM-22` & `RM-24` (`0.5.0`).
* Specs: [`../SPEC-UI.md`](../SPEC-UI.md), [`../SPEC-API.md`](../SPEC-API.md), [`../../AGENTS.md`](../../AGENTS.md).
