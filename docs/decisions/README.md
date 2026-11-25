# Architectural Decisions

This directory holds **Markdown Architectural Decision Records** ([MADR 4.0](https://adr.github.io/madr/)).
Each file records one architecturally significant decision: the forces at play, the options considered,
the chosen outcome, its consequences, and how compliance is verified.

The Storypole backlog tracks *what is being built*; these records track *why the architecture looks
the way it does*. A ticket that introduces or changes architecture references an ADR, and each ADR
links back to the implementing tickets and commits.

## Conventions

- **Filename:** `NNNN-title-with-dashes.md`, four digits, dashes, lowercase.
- **One decision per file.** If a later decision reverses one, write a new ADR and mark the old one
  `superseded by ADR-NNNN` — never rewrite history.
- **Status values:** `proposed`, `accepted`, `rejected`, `deprecated`, `superseded by ADR-NNNN`.
- **Headings:** Context and Problem Statement · Decision Drivers · Considered Options · Decision
  Outcome · Consequences · Confirmation · Pros and Cons of the Options · More Information.

## When to Write One

Write an ADR when a decision is hard to reverse, spans multiple crates, alters the wire or storage
contract, or trades one quality attribute against another (e.g., RAM footprint vs. query flexibility).

## Index

| ADR | Title | Status | Tickets |
| --- | --- | --- | --- |
| [0001](./0001-adopt-madr.md) | Record architectural decisions with MADR | accepted | RM-1 |
| [0002](./0002-sqlite-wal-behind-storage-traits.md) | Back v1.0 with bundled SQLite WAL behind pluggable Storage traits and an event-stream-first schema | accepted | RM-4, RM-5, RM-6, RM-17, RM-21, RM-22 |
| [0003](./0003-pure-rust-core-and-native-multiarch.md) | Use a pure-Rust core crate and native multi-arch binaries, deferring WASM runtime targets | accepted | RM-2, RM-3, RM-16 |
| [0004](./0004-cookieless-salt-sessions-and-cardinality-caps.md) | Enforce cookieless HMAC-SHA256 hashing, bounded 30m sessions, and daily cardinality caps | accepted | RM-3, RM-4, RM-8, RM-17 |
| [0005](./0005-clean-sheet-v1-api-and-scoped-keys.md) | Ship a clean-sheet /v1 API with scoped hashed keys and Astro/Cloudflare Workers SSR integration | accepted | RM-7, RM-8, RM-9, RM-10, RM-11 |
| [0006](./0006-headless-v1-interfaces-cli-mcp-tui.md) | Ship v1.0 headless with CLI, read-only stdio MCP, and interactive ratatui TUI | accepted | RM-12, RM-13, RM-14, RM-15, RM-18, RM-20 |
| [0007](./0007-opaque-uids-and-saas-tenancy-seams.md) | Adopt opaque prefixed UUIDv7 UIDs, site-scoped dictionaries, and SaaS seams | accepted | RM-3, RM-4, RM-5, RM-7, RM-8, RM-23 |
| [0008](./0008-oltp-olap-split-arrow-and-multinode-seams.md) | Split OLTP/OLAP storage, adopt feature-gated Arrow exchange, and establish multi-node seams | accepted | RM-22, RM-24, RM-28, RM-29 |

[`madr-template.md`](../templates/madr-template.md) is the starting point for `0009-...`.
