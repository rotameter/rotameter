# AGENTS.md — rotameter

Operational instructions and runtime guardrails for autonomous and assisted coding agents working in
this repository. Read this before changing any file: every commit and ticket close is verified against
this contract.

> **Maintainer:** Colin Wolff · **License:** `MIT OR Apache-2.0`
> **Built with (dogfooding only):** the `gaugeblock` suite — `storypole`, `miter`, `chalkline`, `dado`,
> `batten`, `joiner`, `benchwork`, `outfeed`, `rabbet`, `sawhorse`. `rotameter` is an independent
> project: it is not part of, owned by, governed by, or released under that suite or its organization.
> Those tools are used here strictly as a consumer.
> **Language:** Rust, **edition 2024**, MSRV **1.99** (`edition`/`rust-version` pinned in root `Cargo.toml`)
> **Versioning:** Lockstep SemVer across the workspace; git tags `v<version>`

---

## 1. Project Mission & Canonical Specs

`rotameter` is a single-binary, privacy-first, headless web analytics engine in Rust built for
unattended durability on a `1 vCPU / 1 GB RAM` VM (`< 40 MB` idle RSS, `< 128 MB` peak RSS) with zero
cookies, zero persisted raw IPs, and an event-stream-first SQLite WAL store behind pluggable `Storage`
traits.

- [`PROJECT.md`](./PROJECT.md) — product boundaries, engineering budgets, phased roadmap
- [`docs/ROADMAP.md`](./docs/ROADMAP.md) — `0.1.0` → `0.6.0` → `1.0.0` → `2.0.0` milestones and `RM-*` map
- [`docs/SPEC-ARCHITECTURE.md`](./docs/SPEC-ARCHITECTURE.md) — crate DAG, `Storage` traits, ring buffer, SQLite schema
- [`docs/SPEC-PRIVACY.md`](./docs/SPEC-PRIVACY.md) — `HMAC-SHA256` salt rotation, 30m sessions, bots, GeoIP, cardinality caps
- [`docs/SPEC-API.md`](./docs/SPEC-API.md) — `/v1/*` HTTP contract, scoped auth, `collect.js`, `pixel.gif`, Astro/CF SSR
- [`docs/SPEC-INTERFACES.md`](./docs/SPEC-INTERFACES.md) — CLI, `stdio` read-only MCP, `ratatui` TUI
- [`docs/SPEC-OPERATIONS.md`](./docs/SPEC-OPERATIONS.md) — multi-arch `musl`/macOS builds, `systemd`, Litestream/R2, load gates
- [`docs/decisions/README.md`](./docs/decisions/README.md) — MADR 4.0 architectural decisions

---

## 2. Workspace Layout & Crate Boundaries

Cargo virtual workspace (scaffolded under `RM-2`). Dependency edges flow strictly downward — zero
lateral edges between leaf apps or upward leaks into `rotameter-core`:

| Crate | Path | Owns |
| --- | --- | --- |
| `rotameter-core` | `crates/rotameter-core` | Domain types, `HMAC-SHA256` hashing, salt rotation, 30m session LRU, URL/UTM/referrer normalizers, bot/UA classifiers, cardinality guard, `Storage` traits. **Zero `tokio`, `rusqlite`, `axum`, `ratatui`, or OS I/O.** |
| `rotameter-store-sqlite` | `crates/rotameter-store-sqlite` | Bundled SQLite WAL `Storage` implementation, migrations, `dim_strings` interner, batched `BEGIN IMMEDIATE` writer, `VACUUM INTO` backups. |
| `rotameter-server` | `crates/rotameter-server` | `axum` `/v1/*` server, scoped Bearer auth, rate limiting, `16,384`-event MPSC ring buffer, GeoIP, `collect.js`/`pixel.gif`, `SIGTERM` drain, first-boot `self_site` bootstrap. |
| `rotameter-client` | `crates/rotameter-client` | Typed HTTP client, `config.toml` profiles, `CF-Access-Client-Id`/`Secret` headers. |
| `rotameter-mcp` | `crates/rotameter-mcp` | `stdio` JSON-RPC 2.0 read-only MCP server (7 analytics tools in `v1.0`). |
| `rotameter-tui` | `crates/rotameter-tui` | Interactive `ratatui` flight-instrument terminal UI with ASCII block chart and drill-down. |
| `rotameter-cli` | `crates/rotameter-cli` | Unified feature-gated `rotameter` binary (`server`, `sqlite`, `mcp`, `tui`). |

`miter deps --path . --unused` proves declared crate dependencies remain strictly used.

---

## 3. Invariant Rules (DO NOT BREAK)

### 3.1 Runtime, Privacy & Budget Invariants
- **Memory ceiling:** Idle RSS `< 40 MB`; peak RSS `< 128 MB` on `1 vCPU / 1 GB RAM` under `5,000 events/s` burst + concurrent queries.
- **Collector weight:** `assets/collect.js` stays **`<= 1,536 bytes` gzip**, zero cookies, zero tracking storage (reads `localStorage.rotameter_ignore` only).
- **Zero raw PII on disk:** Never store raw IPs or unhashed tracking identifiers in SQLite, memory dumps, or `tracing` logs.
- **Local-only self-dogfooding:** Preconfigured `self_site` initialization lives strictly inside the operator's local database and respects `--no-self-site` / `ROTAMETER_SELF_SITE=false` / `[server] self_site = false`. **Rotameter never phones home.**
- **Safety:** `#![forbid(unsafe_code)]` on all workspace crates; zero `.unwrap()`, `.expect()`, or `panic!()` outside `#[cfg(test)]` / `tests/` / `benches/`.
- **Clean-sheet `/v1/*`:** Never add Plausible, GoatCounter, Matomo, or GA4 wire compatibility shims.

### 3.2 Modularity: 250-Line Ceiling, 200-Line Warning
- **Hard ceiling — 250 lines per `.rs` file** (comments and inline tests included). `miter audit --path . --threshold 250` fails non-zero at `>= 250`.
- **Warning band — 200–249 lines:** Plan a submodule split via `miter slice` (`types.rs`, `error.rs`, `tests.rs` with `pub use` re-exports) before adding new code.
- **Docs ceiling:** Canonical docs stay under **500 lines** (`chalkline docs check -d .`).

### 3.3 Atomic Commits, Lockstep SemVer & Keep a Changelog
- One logical change per commit, on a feature branch (`feat/<rm-n-slug>`), using Conventional Commits (`<type>(<scope>): <summary>` + `Task: RM-<n>` footer).
- `[workspace.package] version` in root `Cargo.toml` is the single version source; release bumps run through `outfeed plan` / `outfeed apply` + annotated tag `v<version>`. Never bump versions in feature commits.
- Observable or user-facing changes land with an entry under `## [Unreleased]` in [`CHANGELOG.md`](./CHANGELOG.md) in the same commit (`chalkline docs check -d .` enforces `enforce_changelog_unreleased`).

### 3.4 Tiered Verification Policy

**Tier 1 — Inner loop (every edit):**
```bash
miter verify --path crates/<dir>
cargo nextest run -p <crate> <module>       # fallback: cargo test -p <crate>
```

**Tier 2 — Atomic commit gate (every commit, per touched crate):**
```bash
cargo fmt --check -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
cargo nextest run -p <crate>
miter audit --path . --threshold 250
chalkline docs check -d .
```

**Tier 3 — Task completion gate (before merge to `main` & `storypole verify`):**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --doc --workspace
cargo tree -p rotameter-core --edges normal # verify zero tokio/rusqlite/axum/ratatui
miter audit --path . --threshold 200
miter deps --path . --unused
storypole verify <KEY> --run
```

**Tier 4 — Release gate (per version bump / `0.1.0` & `1.0.0` release milestones):**
Everything in Tier 3 plus `cargo build --workspace --release`, `./scripts/check-collector-size.sh`, `./scripts/check-memory-budget.sh`, `outfeed plan -d . --no-llm`, and the `RM-17` release harness (`SIGTERM` drain, midnight rollover, 100k cardinality bomb, 1M-event `p95 < 25 ms`, Litestream/R2 restore drill).

### 3.5 Zero Inline Scope Creep (`storypole defer`)
Unrelated bugs, `0.2.0` storage/Arrow work, `0.3.0` Web UI ideas, `0.4.0` funnel/heatmap features, or `0.5.0`/`0.6.0` adapter/plugin work discovered mid-ticket must not leak into the current diff:
```bash
storypole defer "<title>" --kind <bug|tech-debt|feature|refactor|spike> \
  --crate-name <crate> --symbols "<crate::path::symbol>" --files "<path>"
```

---

## 4. Dogfooding the `gaugeblock` Suite

Workspace configuration and state live under `.gaugeblock/` and `.agents/`:
- **Tracked:** `.gaugeblock/config.toml`, `.gaugeblock/docs.toml`, `.gaugeblock/storypole/config.toml`, `.gaugeblock/storypole/ledger.jsonl` (and `tickets/`/`plans/`), `.agents/mcp_config.json`, `.agents/rules.md`, `.agents/skills/gaugeblock-workflow/SKILL.md`.
- **Ignored (machine-local):** `.gaugeblock/storypole/index.sqlite*`, `.gaugeblock/storypole/workspace.sqlite*`, `.gaugeblock/miter/*.redb*`, `.gaugeblock/spool/`, `.gaugeblock/worktrees/`, `.gaugeblock/datasets/`.

| Tool | Role in `rotameter` |
| --- | --- |
| `storypole` | Backlog of record (`RM-*`), leases (`claim`/`heartbeat`/`handoff`), `defer`, `verify --run`, `ledger-verify`/`ledger-reconcile`. |
| `miter` | AST index (`index`, `lookup`, `skeleton`, `references`, `impact`, `slice`), 250/200-line `audit`, `deps --unused`, isolated `verify`. |
| `chalkline` | `recon`, `grill`, and `docs check -d .` across the 9-doc map in `.gaugeblock/docs.toml`. |
| `outfeed` | SemVer bump planning (`plan`, `apply`, `status`, `lockfiles`) and `CHANGELOG.md` promotion. |
| `benchwork` | Workspace capabilities check (`capabilities -r .`) and snapshots (`snapshot -r .`). |
| `dado` + `batten` | Deterministic boilerplate generation and grammar validation for repetitive wire/schema matrices. |
| `joiner` / `rabbet` / `sawhorse` | Optional synthesis & telemetry pipeline (never required on the critical path). |

---

## 5. Agent Execution Loop

1. **Brief & lease:** `storypole next` → `storypole claim <KEY> <AGENT_ID> --branch feat/<slug>` → `storypole show <KEY>`.
2. **Slice context:** `miter skeleton <path>` / `miter lookup <Symbol> --context` / `miter impact <Symbol> --depth 2` before reading full files.
3. **Decide:** Record architectural decisions as MADR 4.0 files in `docs/decisions/` and update [`docs/decisions/README.md`](./docs/decisions/README.md).
4. **Implement (Tier 1):** Small modules (`< 250` lines), docstrings on public items, tests beside code, `miter verify --path crates/<dir>`.
5. **Defer:** Push out-of-scope discoveries to `storypole defer`.
6. **Commit (Tier 2):** Per-crate fmt/clippy/nextest + `miter audit --threshold 250` + `chalkline docs check -d .` + `CHANGELOG.md` line.
7. **Close (Tier 3):** Whole-workspace gates + `storypole verify <KEY> --run`.
