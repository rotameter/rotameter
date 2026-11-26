#![forbid(unsafe_code)]
//! Integration tests for `rotameter import --unix-socket` (`dgram` Nginx syslog & `stream` Caddy JSON).

#![cfg(all(unix, feature = "server", feature = "sqlite"))]

use std::error::Error;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rotameter_cli::serve_store::SqliteServerStore;
use rotameter_client::BlockingRotameterClient;
use rotameter_client::config::ClientConfig;
use rotameter_client::types::{
    QueryFormat, QueryRequest, RangePreset, SiteRef, UpsertSiteRequest, WireTimeRange,
};
use rotameter_server::{ServerConfig, ServerStores};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn unix_socket_dgram_and_stream_ingest_into_live_server() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("sock.db");
    let admin = "rm_adm_sock_import_test_token_1";
    let (base_url, client) = spawn_test_server(&db, admin).unwrap();

    run_dgram_check(dir.path(), &base_url, admin, &client).unwrap();
    run_stream_caddy_check(dir.path(), &base_url, admin, &client).unwrap();
}

fn run_dgram_check(
    dir: &Path,
    base_url: &str,
    admin: &str,
    client: &BlockingRotameterClient,
) -> Result<(), Box<dyn Error>> {
    let sock = dir.join("nginx.sock");
    let child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_rotameter"))
            .args([
                "--url",
                base_url,
                "--token",
                admin,
                "import",
                "stream.dev",
                "--unix-socket",
                sock.to_str().ok_or("bad path")?,
                "--socket-mode",
                "dgram",
                "--batch-size",
                "1",
                "--flush-interval",
                "30",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    wait_until(Duration::from_secs(3), || sock.exists());
    let sender = UnixDatagram::unbound()?;
    let ts = chrono::DateTime::from_timestamp_millis(now_ms())
        .ok_or("invalid ts")?
        .format("%d/%b/%Y:%H:%M:%S +0000");
    let msg = format!(
        "<190>Nov 26 10:00:00 edge nginx: 203.0.113.55 - - [{ts}] \"GET /nginx HTTP/1.1\" 200 400 \"-\" \"Mozilla/5.0 Chrome/131.0.0.0\"\n"
    );
    sender.send_to(msg.as_bytes(), &sock)?;

    wait_until(Duration::from_secs(4), || {
        query_remote_pageviews(client, "stream.dev") >= 1
    });
    assert!(send_sigterm(child.0.id()));
    Ok(())
}

fn run_stream_caddy_check(
    dir: &Path,
    base_url: &str,
    admin: &str,
    client: &BlockingRotameterClient,
) -> Result<(), Box<dyn Error>> {
    let sock = dir.join("caddy.sock");
    let child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_rotameter"))
            .args([
                "--url",
                base_url,
                "--token",
                admin,
                "import",
                "stream.dev",
                "--unix-socket",
                sock.to_str().ok_or("bad path")?,
                "--input-format",
                "caddy-json",
                "--batch-size",
                "1",
                "--flush-interval",
                "30",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    wait_until(Duration::from_secs(3), || sock.exists());

    let ts_secs = now_ms() / 1_000;
    let lines = format!(
        "{{\"ts\":{ts_secs},\"request\":{{\"method\":\"GET\",\"host\":\"stream.dev\",\"uri\":\"/caddy\",\"client_ip\":\"203.0.113.70:1234\",\"headers\":{{\"User-Agent\":[\"Mozilla/5.0 Firefox/132.0\"]}}}},\"status\":200,\"resp_headers\":{{\"Content-Type\":[\"text/html; charset=utf-8\"]}}}}\n\
         {{\"ts\":{ts_secs},\"request\":{{\"method\":\"GET\",\"host\":\"stream.dev\",\"uri\":\"/api/health\",\"client_ip\":\"203.0.113.71\"}},\"status\":200,\"resp_headers\":{{\"Content-Type\":[\"application/json\"]}}}}\n\
         {{\"ts\":{ts_secs},\"request\":{{\"method\":\"GET\",\"host\":\"stream.dev\",\"uri\":\"/gone\",\"client_ip\":\"203.0.113.72\",\"headers\":{{\"User-Agent\":[\"Mozilla/5.0 Firefox/132.0\"]}}}},\"status\":404,\"resp_headers\":{{\"Content-Type\":[\"text/html\"]}}}}\n"
    );
    let mut conn = UnixStream::connect(&sock)?;
    conn.write_all(lines.as_bytes())?;
    drop(conn);

    wait_until(Duration::from_secs(4), || {
        query_remote_pageviews(client, "stream.dev") >= 2
    });
    assert!(send_sigterm(child.0.id()));
    Ok(())
}

fn spawn_test_server(
    db: &Path,
    admin: &str,
) -> Result<(String, BlockingRotameterClient), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr: SocketAddr = listener.local_addr()?;
    drop(listener);
    let store = Arc::new(SqliteServerStore::open(db, now_ms())?);
    let cfg = ServerConfig {
        bind_addr: addr,
        bootstrap_admin_key: Some(admin.to_string()),
        self_site_enabled: false,
        flush_interval_ms: 25,
        ..ServerConfig::default()
    };
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            let _ = rt.block_on(rotameter_server::serve(cfg, ServerStores::unified(store)));
        }
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && TcpStream::connect(addr).is_err() {
        std::thread::sleep(Duration::from_millis(20));
    }
    let url = format!("http://{addr}");
    let client = BlockingRotameterClient::new(ClientConfig::new(&url)?.with_token(admin))?;
    client.create_site(&UpsertSiteRequest {
        slug: Some("stream.dev".to_string()),
        ..UpsertSiteRequest::default()
    })?;
    Ok((url, client))
}

fn query_remote_pageviews(client: &BlockingRotameterClient, site: &str) -> u64 {
    let req = QueryRequest {
        site_id: SiteRef::Slug(site.to_string()),
        range: WireTimeRange::Preset {
            preset: RangePreset::Hours24,
        },
        granularity: None,
        metrics: vec![rotameter_client::types::Metric::Pageviews],
        group_by: Vec::new(),
        filters: Vec::new(),
        compare_previous: false,
        sort: None,
        limit: 10,
        offset: 0,
        format: QueryFormat::Json,
    };
    client.query(&req).map_or(0, |r| r.summary.pageviews)
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
