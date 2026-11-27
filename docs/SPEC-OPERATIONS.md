# Operations, Builds & Verification Specification (`0.1.0+`)

Rotameter is built for unattended durability on resource-constrained single VMs (`1 vCPU / 1 GB RAM`), verified end-to-end starting in `0.1.0`.

*Related specs: [`SPEC-ARCHITECTURE.md`](./SPEC-ARCHITECTURE.md) · [`SPEC-API.md`](./SPEC-API.md) · [`../AGENTS.md`](../AGENTS.md)*

---

## 1. Multi-Architecture Build & Release Matrix

Every release produces stripped static binaries (`LTO = "thin"`, `codegen-units = 1`, `panic = "abort"`) plus multi-arch minimal OCI images (`distroless` / `scratch`):

| Target Triple | Platform | C Runtime | Primary Role |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-musl` | Linux x86_64 | Static `musl` | `VM.Standard.E2.1.Micro` (`1 vCPU / 1 GB`), generic VPS |
| `aarch64-unknown-linux-musl` | Linux ARM64 | Static `musl` | `VM.Standard.A1.Flex`, ARM64 Kubernetes / Raspberry Pi |
| `aarch64-apple-darwin` | macOS Apple Silicon | System | Operator CLI / TUI / MCP workstation |
| `x86_64-apple-darwin` | macOS Intel | System | Operator CLI / TUI / MCP workstation |

- **Multi-arch container tags:** `linux/amd64` + `linux/arm64` manifest pushed per semver tag.
- **SQLite compilation:** `rusqlite` builds with the `bundled` feature so zero host `libsqlite3` version drift ever occurs across distros.

---

## 2. Reference Production Topology (`VM.Standard.E2.1.Micro` + Cloudflare)

```text
Visitor Browser / Astro Worker (example.com)
  │  POST /api/e (first-party) ──▶ CF Worker ctx.waitUntil()
  │                                  │ (CF-Access-Client-Id/Secret + CF-Connecting-IP + CF-IPCountry)
  ▼                                  ▼
Cloudflare Edge & Zero Trust Access (analytics.example.com)
  │  Cloudflare Tunnel (outbound-only QUIC/HTTP2 from VM — zero public HTTP ports open)
  ▼
Oracle Always-Free VM (VM.Standard.E2.1.Micro · 1 vCPU / 1 GB RAM / 2 GB swap)
  ├── cloudflared (host/container) ──▶ 127.0.0.1:8000
  ├── rotameter serve (systemd, MemoryMax=256M) ──▶ /var/lib/rotameter/rotameter.db (WAL)
  └── litestream (systemd) ──▶ continuous WAL segment replication to Cloudflare R2
```

### `systemd` Hardening (`infra/rotameter.service`)
- Dedicated unprivileged system user `rotameter:rotameter`.
- Binds strictly to `127.0.0.1:8000` (never `0.0.0.0` when fronted by `cloudflared`).
- Sandbox flags: `NoNewPrivileges=yes`, `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `ReadWritePaths=/var/lib/rotameter`, `MemoryHigh=160M`, `MemoryMax=256M`, `TimeoutStopSec=15s` (matches the `10s` graceful drain deadline).

---

## 3. Backups & Disaster Recovery

- **Continuous WAL streaming (recommended):** **Litestream** runs alongside `rotameter`, streaming WAL segments every few seconds to a private Cloudflare R2 bucket (e.g., `s3://example-backups/rotameter/`). RPO: ~seconds.
- **Point-in-time CLI snapshot:** `rotameter backup /tmp/rotameter-$(date +%F).db` executes SQLite `VACUUM INTO` safely while live writes continue without locking readers.
- **Verified Restore Drill (mandatory per release):**
  1. Stop service or spin up a fresh temp directory.
  2. Restore from R2 (`litestream restore`) or `rotameter backup` artifact.
  3. Run `PRAGMA integrity_check;` and `rotameter doctor --db <restored>`.
  4. Verify `POST /v1/query` returns matching totals against pre-restore counts.

---

## 4. Release Verification & Load Gates

Every release (`0.1.0+`) is gated on passing all automated tests and load checks on a `1 vCPU / 1 GB` constrained environment (or `docker run --cpus=1 --memory=1g` Linux `musl` container):

| Gate | Assertion |
| --- | --- |
| **Static purity** | `cargo tree -p rotameter-core` confirms zero `tokio`, `rusqlite`, `axum`, `ratatui`, or OS runtime crates. |
| **Zero unsafe / panics** | `#![forbid(unsafe_code)]` compiles; CI grep verifies zero `.unwrap()`/`.expect()` outside tests. |
| **Collector budget** | `gzip -9 -c assets/collect.js \| wc -c <= 1536` bytes. |
| **Idle & burst memory** | Idle RSS `< 40 MB`; peak RSS `< 128 MB` during `5,000 events/sec` 60-second burst + concurrent `POST /v1/query`. |
| **Graceful `SIGTERM` drain** | Enqueue `10,000` events, immediately send `SIGTERM`, assert zero acknowledged (`202`) events are lost once the process exits. |
| **Midnight salt rollover** | Replay a continuous visitor crossing `23:55 -> 00:05 UTC`; assert `visits == 1`, `visitors == 1`, `event_count == 2` under the 30-min grace window. |
| **Cardinality bomb resilience** | Blast `100,000` distinct query strings & paths (`/?junk=1..100000`) within 1 day; assert `dim_strings` locks cleanly at the per-site daily cap and routes excess into `(other)` with bounded disk/RAM. |
| **Query SLA benchmark** | Pre-seed `1,000,000` events (`~250,000` sessions) in SQLite WAL; assert `POST /v1/query` (`30d` summary + timeseries + top 20 paths) completes at `p95 < 25 ms`. |
| **Astro / CF Workers E2E** | Verified end-to-end test: browser `collect.js` → Worker `/api/e` (`202` + `waitUntil()`) → Access Service Token → `rotameter` → `POST /v1/query` shows pageview, `CF-IPCountry`, and zero dropped SSR response time. |
