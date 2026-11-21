---
status: accepted
date: 2026-11-20
decision-makers: Colin Wolff (owner)
---

# Record architectural decisions with MADR

## Context and Problem Statement

Rotameter starts as a clean-sheet Rust analytics system spanning multiple workspace crates, strict memory and collector-size budgets, and deliberate phased deferrals (`v1.0` headless engine → `v1.5` UI & tag managers → `v2.0` funnels & secondary storage adapters). Without lightweight architectural records, future contributors or agent sessions risk re-litigating settled boundaries—such as why legacy wire protocols are absent, why SQLite WAL backs `v1.0`, or why WASM/UI/funnels were deferred.

## Decision Drivers

* Keep architectural rationale in-repo alongside code and Storypole tickets so decisions survive context resets.
* Distinguish *why* architecture looks the way it does (MADRs) from *what* work is queued (Storypole).
* Enforce a consistent, scannable format that never rewrites accepted history.

## Considered Options

* MADR 4.0 Markdown files in `docs/decisions/` with an indexed `README.md`
* Informal architectural notes inside `PROJECT.md` and commit messages only
* External wiki or issue-tracker comments outside git

## Decision Outcome

Chosen option: "MADR 4.0 Markdown files in `docs/decisions/` with an indexed `README.md`", because plain Markdown files committed in git travel with every branch, follow a well-known structure, and never drift behind external tools.

### Consequences

* Good, because every major trade-off (storage traits, privacy windows, wire contract, interface split) has one canonical source of truth.
* Good, because superseding a decision requires adding a new numbered record rather than mutating past context.
* Neutral, because local reversible choices inside one crate skip ADRs and live directly in tickets and docstrings.

### Confirmation

* `docs/decisions/README.md` indexes every `NNNN-*.md` file in `docs/decisions/` with status and Storypole ticket references.
* `AGENTS.md` §6 enforces the MADR workflow and template compliance.

## Pros and Cons of the Options

### MADR 4.0 Markdown files in `docs/decisions/`

* Good, because decisions are versioned, reviewable in pull requests, and machine-readable.
* Neutral, because adding a record takes a few minutes of prose discipline per architectural choice.

### Informal notes in `PROJECT.md` and commit messages

* Good, because zero extra files are created.
* Bad, because rationale fragments across git history and gets accidentally overwritten during README/spec edits.

### External wiki or issue comments

* Good, because rich formatting is easy.
* Bad, because offline/agent workflows lose rationale whenever external links rot or sit outside the repo.

## More Information

* Storypole: `RM-1`.
* Template & index: [`template.md`](./template.md), [`README.md`](./README.md).
* Working contract: [`../../AGENTS.md`](../../AGENTS.md).
