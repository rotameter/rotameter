#![forbid(unsafe_code)]
//! E2E test: zero-downtime `POST /v1/admin/backup` while `rotameter serve` runs split SQLite+DuckDB.
#![cfg(all(feature = "server", feature = "sqlite", feature = "duckdb"))]

mod common_http;

use std::error::Error;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common_http::{ChildGuard, http_req, wait_ready};
use rotameter_cli::run;
use rotameter_core::id::SiteRef;
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::{Value, json};

const ADMIN_KEY: &str = "rm_adm_live_backup_e2e_key";

fn seed_live_event(addr: SocketAddr, auth: &str) -> Result<(), Box<dyn Error>> {
    let site_body = serde_json::to_vec(&json!({ "slug": "wolff.industries" }))?;
    let (st, _, raw) = http_req(
        addr,
        "POST",
        "/v1/admin/sites",
        &[
            ("Authorization", auth),
            ("Content-Type", "application/json"),
        ],
        Some(&site_body),
    )?;
    assert_eq!(st, 201);
    let site_id = serde_json::from_slice::<Value>(&raw)?["site_id"]
        .as_str()
        .ok_or("missing site_id")?
        .to_string();

    let hit = serde_json::to_vec(&json!({
        "site_id": site_id,
        "event": "pageview",
        "url": "https://wolff.industries/live-backup"
    }))?;
    let hdrs = [
        ("Content-Type", "application/json"),
        ("CF-Connecting-IP", "198.51.100.77"),
        ("User-Agent", "Mozilla/5.0 LiveBackupTest"),
    ];
    assert_eq!(
        http_req(addr, "POST", "/v1/events", &hdrs, Some(&hit))?.0,
        202
    );

    let q_body = serde_json::to_vec(&json!({
        "site_id": "wolff.industries",
        "metrics": ["visitors", "pageviews"],
        "range": { "preset": "all" }
    }))?;
    let q_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < q_deadline {
        let (qs, _, qr) = http_req(
            addr,
            "POST",
            "/v1/query",
            &[
                ("Authorization", auth),
                ("Content-Type", "application/json"),
            ],
            Some(&q_body),
        )?;
        if qs == 200 && serde_json::from_slice::<Value>(&qr)?["summary"]["pageviews"] == 1 {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("timed out waiting for event flush".into())
}

fn verify_restore_and_offline_backup(
    dir: &Path,
    archive_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let unpacked = dir.join("unpacked");
    rotameter_arrow::unpack_tar_zst(archive_path, &unpacked)?;
    let manifest = rotameter_arrow::verify_manifest(&unpacked)?;
    assert_eq!(manifest.row_counts.events, 1);
    assert!(manifest.row_counts.sites >= 1);

    let r_ctrl = format!("sqlite://{}", dir.join("restored_ctrl.db").display());
    let r_ana_path = dir.join("restored_ana.duckdb");
    let r_ana = format!("duckdb://{}", r_ana_path.display());
    let live_s = archive_path.to_string_lossy().into_owned();
    assert_eq!(
        run([
            "rotameter",
            "restore",
            &live_s,
            "--to",
            &r_ctrl,
            "--plane",
            "control"
        ]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        run([
            "rotameter",
            "restore",
            &live_s,
            "--to",
            &r_ana,
            "--plane",
            "analytics"
        ]),
        ExitCode::SUCCESS
    );

    let now_ms = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let ctrl_sql = SqliteStore::open(
        dir.join("restored_ctrl.db"),
        SqliteConfig::default(),
        now_ms,
    )?;
    let org = ctrl_sql.ensure_default_org_sync(now_ms)?;
    let site = ctrl_sql
        .resolve_site_sync(org.org_id, &SiteRef::Slug("wolff.industries".to_string()))?
        .ok_or("missing restored site")?;
    let duck = DuckDbStore::open(&r_ana_path, &DuckDbConfig::default())?;
    let res = duck.query_sync(
        &site,
        &QuerySpec {
            site_id: SiteRef::Slug("wolff.industries".to_string()),
            range: TimeRange::Preset {
                preset: RangePreset::All,
            },
            granularity: None,
            metrics: vec![Metric::Visitors, Metric::Pageviews, Metric::Visits],
            group_by: vec![],
            filters: vec![],
            compare_previous: false,
            sort: None,
            limit: 50,
            offset: 0,
            format: QueryFormat::default(),
        },
        now_ms,
    )?;
    assert_eq!((res.summary.visitors, res.summary.pageviews), (1, 1));

    let offline_archive = dir.join("offline.tar.zst");
    let offline_s = offline_archive.to_string_lossy().into_owned();
    assert_eq!(
        run([
            "rotameter",
            "--db",
            &r_ctrl,
            "--analytics-db",
            &r_ana,
            "backup",
            &offline_s,
            "--format",
            "portable",
            "--archive",
        ]),
        ExitCode::SUCCESS
    );
    assert!(offline_archive.exists());
    Ok(())
}

#[test]
fn live_post_admin_backup_and_offline_split_backup_restore_duckdb() {
    let dir = tempfile::tempdir().unwrap();
    let ctrl_dsn = format!("sqlite://{}", dir.path().join("control.db").display());
    let ana_dsn = format!("duckdb://{}", dir.path().join("analytics.duckdb").display());
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let bind = addr.to_string();

    let child = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args([
            "--db",
            &ctrl_dsn,
            "--analytics-db",
            &ana_dsn,
            "serve",
            "--bind",
            &bind,
            "--bootstrap-admin-key",
            ADMIN_KEY,
            "--no-self-site",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = ChildGuard(child);
    wait_ready(addr).unwrap();

    let auth = format!("Bearer {ADMIN_KEY}");
    seed_live_event(addr, &auth).unwrap();
    assert_eq!(
        http_req(addr, "POST", "/v1/admin/backup", &[], None)
            .unwrap()
            .0,
        401
    );

    let (st, hdrs, bytes) = http_req(
        addr,
        "POST",
        "/v1/admin/backup",
        &[("Authorization", &auth)],
        None,
    )
    .unwrap();
    assert_eq!(st, 200);
    assert!(
        hdrs.to_ascii_lowercase().contains("application/zstd"),
        "{hdrs}"
    );
    let live_archive = dir.path().join("live.tar.zst");
    std::fs::write(&live_archive, &bytes).unwrap();
    verify_restore_and_offline_backup(dir.path(), &live_archive).unwrap();
}
