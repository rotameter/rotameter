# Rotameter Production Deployment & Disaster Recovery (`infra/`)

Reference manifests for running `rotameter` unattended on a `1 vCPU / 1 GB RAM` Linux VM (`VM.Standard.E2.1.Micro`) behind Cloudflare Tunnel + Zero Trust Access, with continuous Litestream WAL replication to Cloudflare R2.

## Files

- [`rotameter.service`](./rotameter.service) — Hardened `systemd` unit binding `127.0.0.1:8000` (`MemoryHigh=160M`, `MemoryMax=256M`, `TimeoutStopSec=15s`, `SIGTERM` drain).
- [`rotameter.env.example`](./rotameter.env.example) — Environment template for `/etc/rotameter/rotameter.env` (`0600`).
- [`litestream.yml`](./litestream.yml) & [`litestream.service`](./litestream.service) — Continuous SQLite WAL streaming to Cloudflare R2 (`5s` sync, `168h` retention).
- [`restore-drill.sh`](./restore-drill.sh) — Automated restore verification against `VACUUM INTO` backups, Litestream R2 replicas, or an isolated `--self-test`.
- [`../Dockerfile`](../Dockerfile) — Multi-stage static `musl` distroless OCI build (`linux/amd64` + `linux/arm64`).

## Quick Provisioning (`systemd` + Litestream)

```bash
sudo useradd --system --home /var/lib/rotameter --shell /usr/sbin/nologin rotameter
sudo install -d -o rotameter -g rotameter -m 0750 /var/lib/rotameter /etc/rotameter
sudo cp infra/rotameter.env.example /etc/rotameter/rotameter.env
sudo chmod 0600 /etc/rotameter/rotameter.env

sudo cp infra/rotameter.service infra/litestream.service /etc/systemd/system/
sudo cp infra/litestream.yml /etc/litestream.yml && sudo chmod 0600 /etc/litestream.yml
sudo systemctl daemon-reload
sudo systemctl enable --now rotameter litestream
```

## Restore Drill

Verify an online snapshot or live R2 replica before rotating or migrating hosts:

```bash
# Automated local backup + WAL integrity self-test
ROTAMETER_BIN=./target/release/rotameter ./infra/restore-drill.sh --self-test

# Verify an existing VACUUM INTO backup
./infra/restore-drill.sh --backup /var/backups/rotameter-2026-11-22.db

# Verify a Litestream R2 replica into a temporary database
./infra/restore-drill.sh --litestream /etc/litestream.yml --db /var/lib/rotameter/rotameter.db
```
