#![forbid(unsafe_code)]
//! Integration tests for `--features duckdb`: SQLite -> `DuckDB` migrate and portable restore parity.
#![cfg(all(feature = "sqlite", feature = "duckdb"))]

use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rotameter_cli::run;
use rotameter_core::id::SiteRef;
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_core::storage::KpiSummary;
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn seed_demo_sqlite(src_s: &str) -> ExitCode {
    run([
        "rotameter",
        "--db",
        src_s,
        "seed",
        "--site",
        "demo.local",
        "--days",
        "2",
        "--sessions-per-day",
        "5",
        "--realtime-visitors",
        "2",
    ])
}

fn sqlite_expected_summary(
    db: &Path,
) -> Result<(rotameter_core::types::Site, QuerySpec, i64, KpiSummary), String> {
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())?;
    let sql = SqliteStore::open(db, SqliteConfig::default(), now).map_err(|e| e.to_string())?;
    let org = sql
        .ensure_default_org_sync(now)
        .map_err(|e| e.to_string())?;
    let site = sql
        .resolve_site_sync(org.org_id, &SiteRef::Slug("demo.local".to_string()))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "missing seeded site".to_string())?;
    let spec = QuerySpec {
        site_id: SiteRef::Slug("demo.local".to_string()),
        range: TimeRange::Preset {
            preset: RangePreset::Hours24,
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
    };
    let summary = sql
        .query_sync(&site, &spec, now)
        .map_err(|e| e.to_string())?
        .summary;
    Ok((site, spec, now, summary))
}

#[test]
fn duckdb_migrate_and_portable_restore_match_sqlite_queries() {
    let dir = tempfile::tempdir().unwrap();
    let src_db = dir.path().join("src.db");
    let duck_mig = dir.path().join("migrated.duckdb");
    let bundle = dir.path().join("bundle.tar.zst");
    let duck_res = dir.path().join("restored.duckdb");
    let src_s = src_db.to_str().unwrap();

    assert_eq!(seed_demo_sqlite(src_s), ExitCode::SUCCESS);

    let to_mig = format!("duckdb://{}", duck_mig.display());
    assert_eq!(
        run([
            "rotameter",
            "migrate",
            "--from",
            src_s,
            "--to",
            &to_mig,
            "--plane",
            "analytics"
        ]),
        ExitCode::SUCCESS
    );
    assert_ne!(
        run([
            "rotameter",
            "migrate",
            "--from",
            src_s,
            "--to",
            &to_mig,
            "--plane",
            "all"
        ]),
        ExitCode::SUCCESS
    );

    let bundle_s = bundle.to_str().unwrap();
    let to_res = format!("duckdb://{}", duck_res.display());
    assert_eq!(
        run([
            "rotameter",
            "--db",
            src_s,
            "backup",
            "--format",
            "portable",
            bundle_s
        ]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        run([
            "rotameter",
            "restore",
            bundle_s,
            "--to",
            &to_res,
            "--plane",
            "analytics"
        ]),
        ExitCode::SUCCESS
    );

    let (site, spec, now, expected) = sqlite_expected_summary(&src_db).unwrap();
    for d_path in [duck_mig, duck_res] {
        let duck = DuckDbStore::open(&d_path, &DuckDbConfig::default()).unwrap();
        let actual = duck.query_sync(&site, &spec, now).unwrap();
        assert_eq!(actual.summary, expected);
    }
}
