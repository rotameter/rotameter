#![forbid(unsafe_code)]
//! Integration tests for streaming `rotameter import` (`--stdin`/`-` and non-blocking drop).

#![cfg(all(unix, feature = "server", feature = "sqlite"))]

use std::error::Error;
use std::io::Write;
use std::process::{Command, Stdio};

use rotameter_core::id::SiteRef as CoreSiteRef;
use rotameter_core::query::{
    Dimension, Metric, QueryFormat as CoreFormat, QuerySpec, RangePreset as CoreRange,
    TimeRange as CoreTimeRange,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

#[test]
fn stdin_dash_streams_clf_with_proxy_filter_and_ua_into_local_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("stdin.db");
    let now = now_ms();
    let store = SqliteStore::open(&db, SqliteConfig::default(), now).unwrap();
    let org = store.ensure_default_org_sync(now).unwrap();
    drop(store);

    let out = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args(["--db", db.to_str().unwrap(), "site", "upsert", "proxy.dev"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let ts = chrono::DateTime::from_timestamp_millis(now)
        .unwrap()
        .format("%d/%b/%Y:%H:%M:%S +0000");
    let logs = format!(
        "203.0.113.10 - - [{ts}] \"GET /pricing HTTP/1.1\" 200 512 \"https://news.ycombinator.com/\" \"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0.0.0\"\n\
         203.0.113.11 - - [{ts}] \"GET /missing HTTP/1.1\" 404 128 \"-\" \"Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0\"\n\
         203.0.113.12 - - [{ts}] \"GET /assets/app.js HTTP/1.1\" 200 1024 \"-\" \"Mozilla/5.0 Chrome/131.0.0.0\"\n\
         203.0.113.13 - - [{ts}] \"POST /api/login HTTP/1.1\" 200 64 \"-\" \"Mozilla/5.0 Chrome/131.0.0.0\"\n\
         203.0.113.14 - - [{ts}] \"GET /bot HTTP/1.1\" 200 64 \"-\" \"Googlebot/2.1 (+http://www.google.com/bot.html)\"\n"
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args([
            "--db",
            db.to_str().unwrap(),
            "--format",
            "json",
            "import",
            "proxy.dev",
            "-",
            "--batch-size",
            "2",
            "--flush-interval",
            "40",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(logs.as_bytes())
        .unwrap();
    let res = child.wait_with_output().unwrap();
    assert!(res.status.success());

    let summary: serde_json::Value = serde_json::from_slice(&res.stdout).unwrap();
    assert_eq!(summary["parsed"].as_u64(), Some(3));
    assert_eq!(summary["accepted"].as_u64(), Some(2));
    assert_eq!(summary["rejected"].as_u64(), Some(1));

    let store = SqliteStore::open(&db, SqliteConfig::default(), now).unwrap();
    let site = store
        .resolve_site_sync(org.org_id, &CoreSiteRef::Slug("proxy.dev".to_string()))
        .unwrap()
        .unwrap();
    let qr = query_events(
        &store,
        &site,
        now,
        vec![Dimension::EventName, Dimension::Browser],
    )
    .unwrap();
    assert_eq!(qr.summary.visitors, 2);
    assert_eq!(qr.summary.pageviews, 1);
    assert_eq!(qr.rows.len(), 2);
}

#[test]
fn streaming_drops_nonblocking_when_upstream_is_unreachable() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args([
            "--url",
            "http://127.0.0.1:1",
            "--token",
            "rm_adm_unreachable_drop_test",
            "--format",
            "json",
            "import",
            "drop.dev",
            "--stdin",
            "--max-queue",
            "1",
            "--batch-size",
            "1",
            "--flush-interval",
            "20",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let payload = "{\"url\":\"https://drop.dev/a\"}\n".repeat(12);
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let res = child.wait_with_output().unwrap();
    assert!(res.status.success());
    let summary: serde_json::Value = serde_json::from_slice(&res.stdout).unwrap();
    assert_eq!(summary["parsed"].as_u64(), Some(12));
    let dropped = summary["dropped_backpressure"].as_u64().unwrap_or(0)
        + summary["dropped_upstream"].as_u64().unwrap_or(0);
    assert_eq!(dropped, 12);
}

fn query_events(
    store: &SqliteStore,
    site: &rotameter_core::types::Site,
    now: i64,
    group_by: Vec<Dimension>,
) -> Result<rotameter_core::storage::QueryResult, Box<dyn Error>> {
    Ok(store.query_sync(
        site,
        &QuerySpec {
            site_id: CoreSiteRef::Id(site.site_id),
            range: CoreTimeRange::Preset {
                preset: CoreRange::Hours24,
            },
            granularity: None,
            metrics: vec![Metric::Visitors, Metric::Pageviews],
            group_by,
            filters: Vec::new(),
            compare_previous: false,
            sort: None,
            limit: 20,
            offset: 0,
            format: CoreFormat::Json,
        },
        now,
    )?)
}

fn now_ms() -> i64 {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(dur.as_millis()).unwrap_or(i64::MAX)
}
