#![forbid(unsafe_code)]
//! Integration tests for `rotameter seed` (`RM-26`).

#![cfg(feature = "sqlite")]

use std::error::Error;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use rotameter_core::id::SiteRef;
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFormat, QuerySpec, RangePreset, TimeRange,
};
use rotameter_core::storage::QueryResult;
use rotameter_core::types::Site;
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::Value;

fn run_seed(args: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args(args)
        .output()?)
}

fn now_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
}

fn query_30d(
    store: &SqliteStore,
    site: &Site,
    dims: Vec<Dimension>,
    now: i64,
) -> Result<QueryResult, Box<dyn Error>> {
    let spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Preset {
            preset: RangePreset::Days30,
        },
        granularity: Some(Granularity::Day),
        metrics: vec![Metric::Visitors, Metric::Pageviews, Metric::Visits],
        group_by: dims,
        filters: Vec::new(),
        compare_previous: false,
        sort: None,
        limit: 20,
        offset: 0,
        format: QueryFormat::Json,
    };
    Ok(store.query_sync(site, &spec, now)?)
}

fn seed_json(
    db: &Path,
    days: &str,
    per_day: &str,
    rt: &str,
    reset: bool,
) -> Result<Value, Box<dyn Error>> {
    let db_str = db.to_string_lossy().into_owned();
    let mut args = vec![
        "--db",
        &db_str,
        "--format",
        "json",
        "seed",
        "--site",
        "demo.local",
        "--days",
        days,
        "--sessions-per-day",
        per_day,
        "--realtime-visitors",
        rt,
        "--seed",
        "42",
    ];
    if reset {
        args.push("--reset");
    }
    let out = run_seed(&args)?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

#[test]
fn seed_populates_multiday_queries_breakdowns_and_realtime_pulse() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("seed.db");
    let payload = seed_json(&db, "14", "40", "6", false).unwrap();
    assert_eq!(payload["site"], "demo.local");
    let sessions_seeded = payload["sessions"].as_u64().unwrap();
    assert!(sessions_seeded > 500);
    assert!(payload["events"].as_u64().unwrap() > sessions_seeded);
    assert_eq!(payload["realtime_visitors"], 6);

    let now = now_ms();
    let store = SqliteStore::open(&db, SqliteConfig::default(), now).unwrap();
    let org = store.ensure_default_org_sync(now).unwrap();
    let site = store
        .resolve_site_sync(org.org_id, &SiteRef::Slug("demo.local".to_string()))
        .unwrap()
        .unwrap();

    let res = query_30d(&store, &site, vec![Dimension::Path], now).unwrap();
    assert_eq!(res.summary.visits, sessions_seeded);
    assert!(res.summary.visitors > 0 && res.summary.pageviews > 0);
    assert!(res.timeseries.len() >= 14);
    assert!(res.rows.len() > 3);
    for dim in [Dimension::Country, Dimension::Browser, Dimension::Source] {
        assert!(query_30d(&store, &site, vec![dim], now).unwrap().rows.len() >= 2);
    }

    let rt = store.realtime_sync(&site, now).unwrap();
    assert!(rt.active_visitors >= 6);
    assert_ne!(rt.active_pages.len(), 0);
    assert!(rt.pulse.iter().map(|p| p.events).sum::<u64>() > 0);
}

#[test]
fn seed_is_deterministic_supports_reset_and_validates_usage() {
    let dir = tempfile::tempdir().unwrap();
    let db1 = dir.path().join("s1.db");
    let db2 = dir.path().join("s2.db");

    let p1 = seed_json(&db1, "7", "30", "4", false).unwrap();
    let p2 = seed_json(&db2, "7", "30", "4", false).unwrap();
    assert_eq!(p1["sessions"], p2["sessions"]);
    assert_eq!(p1["events"], p2["events"]);

    let _ = seed_json(&db1, "7", "30", "4", true).unwrap();
    let now = now_ms();
    let store = SqliteStore::open(&db1, SqliteConfig::default(), now).unwrap();
    let org = store.ensure_default_org_sync(now).unwrap();
    let site = store
        .resolve_site_sync(org.org_id, &SiteRef::Slug("demo.local".to_string()))
        .unwrap()
        .unwrap();
    let res = query_30d(&store, &site, Vec::new(), now).unwrap();
    assert_eq!(res.summary.visits, p1["sessions"].as_u64().unwrap());

    let db_str = db1.to_str().unwrap();
    assert_eq!(
        run_seed(&["--db", db_str, "seed", "--days", "0"])
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        run_seed(&["--db", db_str, "seed", "--site", "bad slug!"])
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
