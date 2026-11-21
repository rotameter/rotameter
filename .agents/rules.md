# Agent Operating Rules — Rotameter

These rules govern autonomous and assisted engineering work in `rotameter`. The `gaugeblock` suite (`miter`, `storypole`, `chalkline`, `dado`, `batten`, `joiner`, `benchwork`, `outfeed`) acts as the deterministic co-processor and gatekeeper.

---

## 1. Modularity & File Size Invariants

- **Hard File Ceiling:** No `.rs` source file may reach **250 lines** (comments and inline tests included). Enforce with `miter audit --path . --threshold 250`.
- **Warning Band:** Any source file reaching **200–249 lines** must be split before growing further by extracting submodules (`types.rs`, `error.rs`, `tests.rs`) with `miter slice`.
- **Docs Ceiling:** Markdown specs and docs stay under **500 lines** (`chalkline docs check -d .`).

---

## 2. AST-First Inspection (`miter`)

- Use `miter lookup <Symbol> --context`, `miter skeleton <path>`, `miter references <Symbol>`, and `miter impact <Symbol> --depth 2` before opening full source files.
- Restrict `read_file` to the minimal line window needed for an edit after resolving the contract via `miter`.
- Run `miter index .` if new source files created during the session are not yet indexed in `.gaugeblock/miter/miter.redb`.

---

## 3. Closed-Loop Compiler & Gate Verification

- **Tier 1 (inner loop):** Run `miter verify --path crates/<dir>` and targeted `cargo nextest run -p <crate>` after edits rather than flooding context with raw full-workspace builds.
- **Tier 2 (per commit):** `cargo fmt --check`, `cargo clippy -p <crate> --all-targets -- -D warnings`, `cargo nextest run -p <crate>`, `miter audit --path . --threshold 250`, and `chalkline docs check -d .`.
- **Tier 3 (ticket close):** Full workspace fmt/clippy/nextest/doctests, `miter audit --threshold 200` inspection, `miter deps --path . --unused`, `rotameter-core` purity check, and `storypole verify <KEY> --run`.

---

## 4. Scope, Privacy & Commit Discipline

- **Zero inline scope creep:** Record unrelated fixes or ideas immediately with `storypole defer "<title>" --kind <bug|tech-debt|feature|refactor|spike> ...` and stay strictly on the claimed ticket.
- **Atomic commits & changelog:** Every user-visible or behavioral change carries a line under `## [Unreleased]` in `CHANGELOG.md` in the same commit, using Conventional Commits (`<type>(<scope>): <summary>` + `Task: RM-<n>` footer).
- **Privacy & budgets:** Never persist raw IPs or tracking cookies, never phone home (local `self_site` dogfooding stays strictly in the operator DB), never violate `#![forbid(unsafe_code)]`, and keep `collect.js <= 1.5 KB` gzip.
