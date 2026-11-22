---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Ship v1.0 headless with CLI, read-only stdio MCP, and interactive ratatui TUI

## Context and Problem Statement

Building a polished multi-user Web UI alongside a brand-new analytics engine expands scope dramatically, pulls in frontend build toolchains, OIDC/session management, and RBAC before the core engine and metrics are battle-tested. Meanwhile, operators and AI workflows still need immediate inspectability—and the owner wants both rich agent access and a terminal flight instrument in `v1.0`.

## Decision Drivers

* Reach a production-grade `v1.0` engine cleanly without frontend sprawl or user/auth tables delaying release.
* Give terminal operators an immediate live visual dashboard (`ratatui` TUI with ASCII block chart and drill-down).
* Give AI coding/ops agents safe, structured analytics access (`stdio` MCP) without exposing destructive admin mutations.
* Keep provisioning/setup cleanly on idempotent CLI and Admin REST APIs.

## Considered Options

* Headless `v1.0` with CLI + read-only `stdio` MCP + interactive `ratatui` TUI; defer Web UI, RBAC, Streamable HTTP `/mcp`, and opt-in `--allow-admin` MCP to `v1.5`
* Build an embedded Web UI + user accounts inside `v1.0`
* Expose `POST/DELETE` admin mutation tools inside the `v1.0` MCP server

## Decision Outcome

Chosen option: "Headless `v1.0` with CLI + read-only `stdio` MCP + interactive `ratatui` TUI", because `rotameter-client` lets CLI, MCP, and TUI talk cleanly over HTTPS + Cloudflare Access Service Tokens to the remote `v1` server from day one while keeping `v1.0` focused and secure.

### Consequences

* Good, because `rotameter mcp` is read-only by construction in `v1.0`, eliminating accidental site deletion or key revocation during AI exploration.
* Good, because `rotameter tui` gives operators cursor-scrubbing ASCII block charts and breakdown filter drill-downs over SSH or locally against `analytics.wolff.industries`.
* Good, because when Web UI (`v1.5`) arrives as a decoupled client over `/v1/*`, the headless engine continues running unchanged (`--ui=off` default).
* Bad, because browser-only admin viewing requires either an external dashboard (e.g., `example.com/analytics`), CLI/TUI, or waiting until `v1.5`.

### Confirmation

* `RM-12`, `RM-13`, `RM-14`, and `RM-15` deliver and test `rotameter-client`, unified feature-gated CLI, 7-tool read-only `stdio` MCP, and `ratatui` TUI.
* `RM-18` and `RM-20` track the deferred `v1.5` Web UI and HTTP/admin MCP extensions cleanly in Storypole (parked).

## Pros and Cons of the Options

### Headless `v1.0` (CLI + read-only `stdio` MCP + interactive TUI)

* Good, enforces clean API separation—every interface is just another typed `/v1` API client.
* Good, eliminates frontend asset weight and user/session tables from `v1.0`.

### Embedded Web UI + RBAC in `v1.0`

* Bad, mixes frontend bundling, login security, and RBAC migrations into the initial storage/engine release.

### Admin mutation tools in `v1.0` MCP

* Bad, increases blast radius for exploratory AI queries; setup belongs in idempotent CLI and `/v1/admin/*` APIs (or IaC providers).

## More Information

* Storypole: `RM-12`, `RM-13`, `RM-14`, `RM-15`, `RM-18`, `RM-20`.
* Specs: [`../SPEC-INTERFACES.md`](../SPEC-INTERFACES.md), [`../../PROJECT.md`](../../PROJECT.md).
