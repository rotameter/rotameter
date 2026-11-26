#![forbid(unsafe_code)]
//! Integration tests for `rotameter migrate` (direct in-memory Arrow streaming & `--stage-dir`).
#![cfg(all(feature = "sqlite", feature = "arrow"))]

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use rotameter_cli::run;
use rotameter_core::id::SiteRef;
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

#[test]
fn arrow_direct_and_staged_migrate_preserve_multi_day_data() {
    let dir = tempfile::tempdir().unwrap();
    let src_db = dir.path().join("src.db");
    let migrated_db = dir.path().join("migrated.db");
    let stage_dir = dir.path().join("stage_bundle");
    let staged_db = dir.path().join("staged.db");
    let src_s = src_db.to_str().unwrap();

    assert_eq!(
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
        ]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        run([
            "rotameter",
            "migrate",
            "--from",
            src_s,
            "--to",
            migrated_db.to_str().unwrap(),
            "--plane",
            "all",
        ]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        run([
            "rotameter",
            "migrate",
            "--from",
            src_s,
            "--to",
            staged_db.to_str().unwrap(),
            "--stage-dir",
            stage_dir.to_str().unwrap(),
        ]),
        ExitCode::SUCCESS
    );
    assert!(stage_dir.join("manifest.json").exists());

    let ts = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let src_store = SqliteStore::open(&src_db, SqliteConfig::default(), ts).unwrap();
    let org = src_store.ensure_default_org_sync(ts).unwrap();
    let site = src_store
        .resolve_site_sync(org.org_id, &SiteRef::Slug("demo.local".to_string()))
        .unwrap()
        .unwrap();
    let spec = QuerySpec {
        site_id: SiteRef::Slug("demo.local".to_string()),
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
    };
    let expected = src_store.query_sync(&site, &spec, ts).unwrap();

    for db in [migrated_db, staged_db] {
        let dst = SqliteStore::open(&db, SqliteConfig::default(), ts).unwrap();
        let actual = dst.query_sync(&site, &spec, ts).unwrap();
        assert_eq!(actual.summary, expected.summary);
    }
}
