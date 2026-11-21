---
name: gaugeblock-workflow
description: >-
  Follow the 7-step Gaugeblock suite workflow when claiming, slicing, implementing, verifying,
  deferring scope, or closing Storypole tickets in rotameter.
---

# Gaugeblock Suite Workflow (`rotameter`)

Use this loop for every implementation ticket in `rotameter`:

## 1. Brief & Lease (`storypole`)
1. Inspect ready work: `storypole next` and `storypole show <KEY>`.
2. Claim a leaf ticket on a feature branch:
   `storypole claim <KEY> <AGENT_ID> --branch feat/<rm-n-slug>`
3. Keep long leases alive with `storypole heartbeat <KEY> <AGENT_ID>`, or release incomplete work with `storypole handoff`.

## 2. Slice Context (`miter` + `chalkline`)
1. Run `chalkline recon -d .` when entering an unfamiliar area.
2. Refresh index if needed: `miter index .`.
3. Inspect symbols/APIs via `miter skeleton <path>`, `miter lookup <Symbol> --context`, and `miter impact <Symbol> --depth 2` before reading files.

## 3. Record Architectural Decisions (`docs/decisions/`)
If a choice affects crate boundaries, storage schemas, wire contracts, privacy invariants, or budgets, record an accepted MADR 4.0 ADR under `docs/decisions/` and update `docs/decisions/README.md` before finishing the ticket.

## 4. Implement in Tier 1
- Write focused modules (`< 200` lines ideal, `< 250` hard ceiling), `#![forbid(unsafe_code)]`, zero non-test `.unwrap()`/`.expect()`, docstrings on public items, and unit/property tests alongside code.
- Validate each edit with `miter verify --path crates/<dir>` and `cargo nextest run -p <crate>`.
- Use `dado render` / `batten validate` if generating repetitive typed schemas or error matrices.

## 5. Defer Out-of-Scope Finds (`storypole defer`)
Never fix unrelated issues inline:
```bash
storypole defer "<title>" --kind <bug|tech-debt|feature|refactor|spike> \
  --crate-name <crate> --symbols "<crate::path::symbol>" --files "<path>"
```

## 6. Commit in Tier 2
Before every commit:
```bash
cargo fmt --check -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
cargo nextest run -p <crate>
miter audit --path . --threshold 250
chalkline docs check -d .
```
Update `## [Unreleased]` in `CHANGELOG.md` in the same commit as observable changes, using Conventional Commits (`<type>(<scope>): <summary>` + `Task: RM-<n>`).

## 7. Close in Tier 3 (or Release in Tier 4)
Before `storypole verify <KEY> --run` and merging to `main`:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --doc --workspace
miter audit --path . --threshold 200
miter deps --path . --unused
storypole verify <KEY> --run
```
At version release only (Tier 4): `cargo build --workspace --release`, `outfeed plan -d . --no-llm`, `outfeed apply` (review diff first), and tag `v<version>`.
