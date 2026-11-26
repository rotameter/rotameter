#![forbid(unsafe_code)]
//! Release gate: `10,000` acknowledged (`202`) events survive immediate `SIGTERM` drain.

#![cfg(all(feature = "server", feature = "sqlite"))]

use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use rotameter_client::BlockingRotameterClient;
use rotameter_client::config::ClientConfig;
use rotameter_client::types::{BatchEventItem, BatchEventsRequest, SiteRef, UpsertSiteRequest};
use rotameter_core::id::SiteRef as CoreSiteRef;
use rotameter_core::query::{
    Metric as CoreMetric, QueryFormat, QuerySpec, RangePreset as CoreRange, TimeRange,
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
fn sigterm_drains_ten_thousand_queued_events_with_zero_loss() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("drain.db");
    let addr = pick_free_port().unwrap();
    let admin = "rm_adm_sigterm_drain_release_key_1";

    let mut child = spawn_server(addr, &db_path, admin).unwrap();
    let base_url = format!("http://{addr}");
    let client =
        BlockingRotameterClient::new(ClientConfig::new(&base_url).unwrap().with_token(admin))
            .unwrap();
    wait_for_health(&client).unwrap();

    client
        .create_site(&UpsertSiteRequest {
            slug: Some("drain.dev".to_string()),
            ..UpsertSiteRequest::default()
        })
        .unwrap();

    let start_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let ts = DateTime::from_timestamp_millis(start_ms)
        .unwrap()
        .to_rfc3339();
    for b in 0..20_u32 {
        let items: Vec<BatchEventItem> = (0..500_u32)
            .map(|i| BatchEventItem {
                site_id: SiteRef::Slug("drain.dev".to_string()),
                event: Some("pageview".to_string()),
                url: format!("https://drain.dev/item/{}", b * 500 + i),
                referrer: None,
                viewport_width: Some(1280),
                props: None,
                ts: Some(ts.clone()),
                visitor_hint: Some(format!("v-{b}-{i}")),
            })
            .collect();
        let res = client
            .post_events_batch(&BatchEventsRequest { events: items })
            .unwrap();
        assert_eq!((res.accepted, res.rejected), (500, 0));
    }

    let status = send_sigterm(&mut child.0).unwrap();
    assert!(
        status.success(),
        "server exited non-zero on SIGTERM: {status}"
    );

    let now_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let store = SqliteStore::open(&db_path, SqliteConfig::default(), now_ms).unwrap();
    let org = store.ensure_default_org_sync(now_ms).unwrap();
    let site = store
        .resolve_site_sync(org.org_id, &CoreSiteRef::Slug("drain.dev".to_string()))
        .unwrap()
        .unwrap();
    let res = store
        .query_sync(
            &site,
            &QuerySpec {
                site_id: CoreSiteRef::Id(site.site_id),
                range: TimeRange::Preset {
                    preset: CoreRange::Days30,
                },
                granularity: None,
                metrics: vec![CoreMetric::Pageviews, CoreMetric::Visitors],
                group_by: Vec::new(),
                filters: Vec::new(),
                compare_previous: false,
                sort: None,
                limit: 50,
                offset: 0,
                format: QueryFormat::default(),
            },
            now_ms,
        )
        .unwrap();
    assert_eq!(res.summary.pageviews, 10_000);
    assert_eq!(res.summary.visitors, 10_000);
}

fn pick_free_port() -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.local_addr()
}

fn spawn_server(addr: SocketAddr, db_path: &Path, admin: &str) -> std::io::Result<ChildGuard> {
    let child = Command::new(env!("CARGO_BIN_EXE_rotameter"))
        .args([
            "serve",
            "--bind",
            &addr.to_string(),
            "--db",
            &db_path.to_string_lossy(),
            "--bootstrap-admin-key",
            admin,
            "--no-self-site",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildGuard(child))
}

fn wait_for_health(client: &BlockingRotameterClient) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if client.health().is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err("rotameter server did not become healthy within 5s".to_string())
}

#[cfg(unix)]
fn send_sigterm(child: &mut Child) -> std::io::Result<std::process::ExitStatus> {
    let pid = child.id().to_string();
    let kill_status = Command::new("kill").args(["-TERM", &pid]).status()?;
    if !kill_status.success() {
        return Err(std::io::Error::other("kill -TERM failed"));
    }
    child.wait()
}
