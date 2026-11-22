---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Ship a clean-sheet /v1 API with scoped hashed keys and Astro/Cloudflare Workers SSR integration

## Context and Problem Statement

Existing self-hosted analytics APIs carry historical wire quirks (e.g., cryptic short field names `{ n, u, d, r }`, separate v1/v2 query models, or limited zero-JS/SSR support). Rotameter's owner explicitly rejected inheriting external wire-protocol debt, while needing first-class support for static/hybrid Astro sites on Cloudflare Workers (e.g., `example.com`) where the analytics engine sits privately behind Cloudflare Tunnel and Zero Trust Access.

## Decision Drivers

* Ergonomic, self-describing `snake_case` JSON under `/v1/*` with a uniform `{ error: { code, message, details? } }` envelope.
* Principle of least privilege via scoped hashed keys (`rm_ing_`, `rm_rd_`, `rm_adm_`) plus idempotent Admin REST routes for CLI and OpenTofu provisioning.
* Zero third-party browser requests and strict `'self'` CSP compatibility on Astro + Cloudflare Workers sites.
* Zero-JS collection options (`/v1/pixel.gif` and pure SSR `trackServerHit()`) alongside a `<= 1.5 KB` gzip `collect.js`.

## Considered Options

* Clean-sheet `/v1/*` JSON API + scoped hashed Bearer keys + `<= 1.5 KB` `collect.js` + `/v1/pixel.gif` + Astro/CF Workers SSR integration
* Emulate Plausible (`POST /api/event`, `/api/v2/query`) or GoatCounter (`/api/v0/*`) endpoints
* Require external third-party origin scripts (`https://analytics.../js/script.js`) without a Worker proxy helper

## Decision Outcome

Chosen option: "Clean-sheet `/v1/*` JSON API + scoped hashed Bearer keys + first-party Astro/CF Workers integration", because owning one crisp `/v1` contract keeps server, client, CLI, MCP, TUI, and Cloudflare Workers integrations consistent and small.

### Consequences

* Good, because `rotameter-client`, `rotameter-mcp`, `rotameter-tui`, and external consumers (like `example.com/analytics`) share one orthogonal query endpoint (`POST /v1/query`).
* Good, because `PUT /v1/admin/sites/{id}` makes CLI and future OpenTofu provisioning idempotent without exposing admin mutations over MCP.
* Good, because the Astro/Cloudflare Workers integration keeps browsers `'self'`-only, proxies via `ctx.waitUntil()`, injects `CF-Access-Client-Id`/`Secret`, and drops `*.workers.dev` preview noise automatically.
* Bad, because sites migrating from Plausible/GoatCounter change their tracker snippet rather than swapping DNS alone (mitigated by `rotameter import` for historical data).

### Confirmation

* `RM-7`, `RM-8`, `RM-9`, `RM-10`, and `RM-11` test every `/v1/*` contract, key scope check, `<= 1536 B` gzip collector budget, and Worker `waitUntil()` round-trip.

## Pros and Cons of the Options

### Clean-sheet `/v1/*` API + Astro/CF Workers integration

* Good, zero legacy protocol baggage and full type safety across all 7 workspace crates.
* Bad, requires first-party integration snippets (mitigated by shipping `integrations/astro-cloudflare/` in `v1.0`).

### Plausible / GoatCounter wire emulation

* Bad, permanently binds Rotameter's query grammar and payload shapes to another project's design compromises.

## More Information

* Storypole: `RM-7`, `RM-8`, `RM-9`, `RM-10`, `RM-11`.
* Specs: [`../SPEC-API.md`](../SPEC-API.md), [`../COMPARISON-FEATURES.md`](../COMPARISON-FEATURES.md).
