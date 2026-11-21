# AGENTS.md — Working Contract

This repository houses **Rotameter** (`MIT OR Apache-2.0`), a single-binary, privacy-first, headless web analytics engine in Rust. Read [`PROJECT.md`](./PROJECT.md) and the specifications in `docs/` before writing implementation code.

---

## 1. Storypole Is the System of Record

Once the specification documents are reviewed and `.gaugeblock/storypole/` is initialized, all implementation work is tracked in Storypole tickets:

- No implementation code lands outside a claimed ticket.
- Every ticket carries explicit acceptance criteria and verification gates.
- Scope discovered mid-task (`v1.5` UI, `v2.0` funnels, GTM/Zaraz templates, secondary DB adapters) is `defer`red rather than silently pulled into `v1.0`.
- Architectural decisions are recorded as MADRs in [`docs/decisions/`](./docs/decisions/README.md).

---

## 2. Branch and Commit Protocol

- `main` holds verified work only.
- One branch per ticket: `<type>/<ticket>-<slug>` (e.g., `feat/rm-12-sqlite-wal-ingestor`).
- Conventional commits in imperative mood, referencing the ticket and exact verification command output.
- Merge with `--no-ff`, then delete the feature branch. Never force-push `main`.

---

## 3. Hard Engineering Boundaries

| Rule | Detail |
| --- | --- |
| **Memory ceiling** | Idle RSS `< 40 MB`; peak RSS `< 128 MB` on `1 vCPU / 1 GB RAM` (`VM.Standard.E2.1.Micro`) under load. |
| **Client script budget** | `collect.js` stays **`<= 1.5 KB` gzip** (`1,536 B`), zero cookies, zero `localStorage` reads except `rotameter_ignore`, zero external network calls. |
| **Zero unsafe code** | `#![forbid(unsafe_code)]` at crate root for all application crates (`rotameter-*`). |
| **No panic paths in prod** | Zero `.unwrap()`, `.expect()`, or `panic!()` outside `#[cfg(test)]` / `tests/` / `benches/`. Use typed errors (`thiserror` in libraries, `anyhow`/structured errors at CLI/server boundaries). |
| **Workspace DAG & core purity** | `rotameter-core` must compile **without** `tokio`, `axum`, `rusqlite`, `ratatui`, or OS network/filesystem crates. Verify cleanly with `cargo tree -p rotameter-core`. |
| **Privacy invariant** | Never persist raw IP addresses, full unhashed `User-Agent` identifiers tied to individuals, or tracking cookies in SQLite, memory dumps, or `tracing` logs. |
| **Clean-sheet `/v1/*` API** | Never introduce legacy compatibility shims for Plausible, GoatCounter, Matomo, or GA4 wire formats. |
| **SQLite safety** | All writes flow through the single batched WAL writer thread/task; every schema change uses numbered forward migrations; backups use `VACUUM INTO` or Litestream (never raw `cp` of an open WAL DB). |
| **File modularity** | Keep source files focused and under ~400 lines; split domain logic cleanly across modules rather than building monolithic `main.rs` or `lib.rs` files. |
| **Warnings as errors** | `cargo clippy --workspace --all-targets -- -D warnings` must pass cleanly. |

---

## 4. Planned Workspace Map

```text
rotameter/
  PROJECT.md
  AGENTS.md
  docs/
    COMPARISON-FEATURES.md
    COMPARISON-METRICS.md
    SPEC-ARCHITECTURE.md
    SPEC-PRIVACY.md
    SPEC-API.md
    SPEC-INTERFACES.md
    SPEC-OPERATIONS.md
    decisions/
      README.md
      template.md
  crates/
    rotameter-core/          # Pure Rust domain types, hashing, session state machine, normalization, query AST
    rotameter-store-sqlite/  # Bundled SQLite WAL Storage implementation + migrations + backup
    rotameter-server/        # Axum HTTP ingest/query/admin server, ring-buffer pipeline, collect.js/pixel
    rotameter-client/        # Typed async/sync HTTP client with CF-Access service token support
    rotameter-mcp/           # stdio MCP server exposing read-only analytics tools
    rotameter-tui/           # ratatui flight-instrument terminal UI
    rotameter-cli/           # Unified `rotameter` binary (feature-gated: server, sqlite, mcp, tui)
  assets/
    collect.js               # Source + size-gated minified collector
  integrations/
    astro-cloudflare/        # First-party Astro + Cloudflare Workers proxy & SSR hit helper
  infra/                     # Reference systemd unit, cloudflared/Access & Litestream deployment configs
```

---

## 5. Verification Gates (Target `v1.0` CI & Local)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo tree -p rotameter-core --edges normal    # assert zero tokio/rusqlite/axum/ratatui deps
./scripts/check-collector-size.sh             # gzip(collect.js) <= 1536 bytes
./scripts/check-memory-budget.sh              # idle < 40 MB RSS, burst/query peak < 128 MB RSS
```

For cross-compilation release verification:

```bash
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl
```

---

## 6. Architectural Decisions

Significant architectural trade-offs—such as storage layout changes, auth model changes, cardinality cap tuning, or adding a new `Storage` backend—are recorded as MADRs in [`docs/decisions/`](./docs/decisions/README.md) using [`docs/decisions/template.md`](./docs/decisions/template.md). Never rewrite an accepted ADR; supersede it with a new record.
