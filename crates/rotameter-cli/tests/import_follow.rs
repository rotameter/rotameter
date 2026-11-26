#![forbid(unsafe_code)]
//! Integration test for `rotameter import --follow` log rotation / inode handling.

#![cfg(all(unix, feature = "server", feature = "sqlite"))]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rotameter_core::id::SiteRef as CoreSiteRef;
use rotameter_core::query::{
    Metric, QueryFormat as CoreFormat, QuerySpec, RangePreset as CoreRange,
    TimeRange as CoreTimeRange,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn follow_mode_tails_and_rotates_by_inode() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("follow.db");
    let log_path = dir.path().join("access.log");
    let rotated_path = dir.path().join("access.log.1");
    let now = now_ms();

    let out = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args(["--db", db.to_str().unwrap(), "site", "upsert", "follow.dev"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let ts = chrono::DateTime::from_timestamp_millis(now)
        .unwrap()
        .format("%d/%b/%Y:%H:%M:%S +0000");
    let line1 = format!(
        "198.51.100.1 - - [{ts}] \"GET /one HTTP/1.1\" 200 100 \"-\" \"Mozilla/5.0 Chrome/131.0.0.0\"\n"
    );
    let line2 = format!(
        "198.51.100.2 - - [{ts}] \"GET /two HTTP/1.1\" 200 100 \"-\" \"Mozilla/5.0 Firefox/132.0\"\n"
    );
    std::fs::write(&log_path, &line1).unwrap();

    let child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_rotameter"))
            .args([
                "--db",
                db.to_str().unwrap(),
                "import",
                "follow.dev",
                log_path.to_str().unwrap(),
                "--follow",
                "--batch-size",
                "1",
                "--flush-interval",
                "30",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );

    wait_until(Duration::from_secs(4), || {
        count_pageviews(&db, "follow.dev", now) >= 1
    });
    std::fs::rename(&log_path, &rotated_path).unwrap();
    std::fs::write(&log_path, &line2).unwrap();
    wait_until(Duration::from_secs(4), || {
        count_pageviews(&db, "follow.dev", now) >= 2
    });
    assert!(send_sigterm(child.0.id()));
}

fn count_pageviews(db: &Path, slug: &str, now: i64) -> u64 {
    let Ok(store) = SqliteStore::open(db, SqliteConfig::default(), now) else {
        return 0;
    };
    let Ok(org) = store.ensure_default_org_sync(now) else {
        return 0;
    };
    let Ok(Some(site)) = store.resolve_site_sync(org.org_id, &CoreSiteRef::Slug(slug.to_string()))
    else {
        return 0;
    };
    store
        .query_sync(
            &site,
            &QuerySpec {
                site_id: CoreSiteRef::Id(site.site_id),
                range: CoreTimeRange::Preset {
                    preset: CoreRange::Hours24,
                },
                granularity: None,
                metrics: vec![Metric::Pageviews],
                group_by: Vec::new(),
                filters: Vec::new(),
                compare_previous: false,
                sort: None,
                limit: 10,
                offset: 0,
                format: CoreFormat::Json,
            },
            now,
        )
        .map_or(0, |r| r.summary.pageviews)
}

fn wait_until<F: Fn() -> bool>(timeout: Duration, cond: F) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn send_sigterm(pid: u32) -> bool {
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .is_ok_and(|s| s.success())
}

fn now_ms() -> i64 {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(dur.as_millis()).unwrap_or(i64::MAX)
}
