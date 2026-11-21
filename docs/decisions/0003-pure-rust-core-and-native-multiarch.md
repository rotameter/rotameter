---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Use a pure-Rust core crate and native multi-arch binaries, deferring WASM runtime targets

## Context and Problem Statement

Rotameter needs to run natively on Oracle `VM.Standard.E2.1.Micro` (`linux/amd64`), Oracle `VM.Standard.A1.Flex` (`linux/arm64`), and operator macOS workstations (`arm64` and `amd64`) as a single unified binary—while avoiding architectural lock-in that would block future edge/WASM runtimes or independent server vs. client builds.

## Decision Drivers

* Zero runtime dependencies on Linux (`static musl`) and macOS across `amd64` and `arm64`.
* Fast unit and property testing of hashing, normalization, bot filtering, and session logic without spinning up `tokio` or SQLite.
* Keep the option to compile pure domain logic to WASM or plug in new backends later without paying serverless/WASM complexity costs in `v1.0`.

## Considered Options

* 7-crate Cargo workspace with an OS/async-free `rotameter-core` crate, unified feature-gated `rotameter` binary (`musl` + `darwin`), deferring WASM runtime targets
* Single monolithic binary crate mixing Axum, SQLite, TUI, and domain logic directly
* Ship a `wasm32-wasip2` / Cloudflare Worker edge target inside `v1.0` alongside the VM binary

## Decision Outcome

Chosen option: "7-crate Cargo workspace with an OS/async-free `rotameter-core` crate and native multi-arch binaries, deferring WASM runtime targets", because it hits `v1.0`'s Oracle VM target cleanly without Durable Object state complexity while guaranteeing clean crate boundaries (`rotameter-core`, `rotameter-store-sqlite`, `rotameter-server`, `rotameter-client`, `rotameter-mcp`, `rotameter-tui`, `rotameter-cli`).

### Consequences

* Good, because `rotameter-core` tests run in milliseconds and stay portable to future WASM or edge targets whenever justified.
* Good, because operators can compile full-featured `rotameter` or slim server-only / client-only binaries via Cargo features (`server`, `sqlite`, `mcp`, `tui`).
* Good, because `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` binaries drop cleanly onto minimal Linux hosts with zero glibc mismatch.
* Neutral, because managing a 7-crate Cargo workspace adds slight manifest boilerplate compared to one `Cargo.toml`.

### Confirmation

* `RM-2` enforces `cargo tree -p rotameter-core` checks proving zero `tokio`, `rusqlite`, `axum`, or `ratatui` dependencies.
* `RM-16` builds and verifies release artifacts for all four native triples plus multi-arch (`linux/amd64`, `linux/arm64`) OCI containers.

## Pros and Cons of the Options

### 7-crate workspace + OS/async-free `rotameter-core` + native multi-arch binaries

* Good, because domain logic, storage, HTTP server, and operator clients share types without pulling server deps into client/MCP/TUI builds.
* Good, because native VM memory and SQLite WAL give persistent in-memory 30m sessions at $0/mo.

### Single monolithic crate

* Bad, because `rotameter-client`, `rotameter-mcp`, and `rotameter-tui` end up entangled with `rusqlite` and `axum`, and `rotameter-core` inevitably leaks `tokio`/FS assumptions.

### Shipping a WASM / Cloudflare Worker runtime in `v1.0`

* Bad, because stateless edge workers lack persistent in-memory 30m session state without paid Durable Objects, adding substantial scope that does not serve the `VM.Standard.E2.1.Micro` deployment target.

## More Information

* Storypole: `RM-2`, `RM-3`, `RM-16`.
* Specs: [`../SPEC-ARCHITECTURE.md`](../SPEC-ARCHITECTURE.md), [`../SPEC-OPERATIONS.md`](../SPEC-OPERATIONS.md), [`../../AGENTS.md`](../../AGENTS.md).
