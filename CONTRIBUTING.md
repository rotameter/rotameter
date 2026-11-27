# Contributing to Rotameter

Thank you for contributing to **Rotameter** (`MIT OR Apache-2.0`). Read [`AGENTS.md`](./AGENTS.md) and [`PROJECT.md`](./PROJECT.md) before opening a branch.

---

## Toolchain & Prerequisites

- **Language:** Rust **edition 2024**, MSRV **1.100** (`rustc 1.100+`)
- **Compiler cache:** `sccache` (configured with a shared `target/` directory in `.cargo/config.toml`; override with `CARGO_BUILD_RUSTC_WRAPPER=""` if `sccache` is not installed)
- **Test runner:** `cargo-nextest` (`cargo nextest run`)
- **Cross-compilation (`musl`):** `cross` targeting `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`
- **Gaugeblock workflow tools:** `storypole`, `miter`, `chalkline`, `outfeed`, `benchwork`

---

## Workflow & Engineering Laws

1. **Storypole first:** Claim or create a ticket before changing code (`storypole next` / `storypole claim <KEY> <AGENT> --branch feat/<slug>`). Out-of-scope discoveries go to `storypole defer`.
2. **File size ceiling:** Keep source files under **200 lines** for task completion (`miter audit --threshold 200`; hard limit **250 lines**) and documentation files under **500 lines**.
3. **Safety & core purity:**
   - `#![forbid(unsafe_code)]` across every workspace crate.
   - Zero `.unwrap()`, `.expect()`, or `panic!()` outside tests/benches.
   - `rotameter-core` remains strictly OS/async/DB-free.
4. **Local previews:** Use `rotameter seed` (`RM-26`) to populate repeatable 30-day curves and live realtime data into an isolated SQLite DB when testing API queries, TUI screens, or UI prototypes.
5. **Conventional Commits & Changelog:** Use `<type>(<scope>): <summary>` with `Task: RM-<n>` footers, and record user-visible changes under `## [Unreleased]` in [`CHANGELOG.md`](./CHANGELOG.md) in the same commit.
6. **Verification before merge:**

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo nextest run --workspace
   cargo test --doc --workspace
   miter audit --path . --threshold 200
   miter deps --path . --unused
   chalkline docs check -d .
   ./scripts/run-release-gates.sh   # full Tier 4 release gate suite
   ```
