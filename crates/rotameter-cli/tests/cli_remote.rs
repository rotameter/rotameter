#![forbid(unsafe_code)]
//! Remote CLI integration tests (`query`, `realtime`, `import`, `doctor`) against live `rotameter-server`.

#![cfg(all(feature = "server", feature = "sqlite"))]

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rotameter_cli::exit::{EXIT_AUTH, EXIT_HEALTH, EXIT_OK};
use rotameter_cli::run;
use rotameter_cli::serve_store::SqliteServerStore;
use rotameter_server::{ServerConfig, ServerStores};

#[test]
fn remote_cli_query_realtime_import_and_doctor_exit_codes() {
    let admin = "rm_adm_cli_remote_test_token_123";
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("remote.db");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    drop(listener);

    let store = Arc::new(SqliteServerStore::open(&db, 1_700_000_000_000).unwrap());
    let cfg = ServerConfig {
        bind_addr: addr,
        bootstrap_admin_key: Some(admin.to_string()),
        self_site_enabled: true,
        flush_interval_ms: 50,
        ..ServerConfig::default()
    };

    let _srv_thread = std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            let _ = rt.block_on(rotameter_server::serve(cfg, ServerStores::unified(store)));
        }
    });
    wait_for_port(addr);

    let base_url = format!("http://{addr}");
    assert_doctor_exit_codes(&base_url, admin);
    assert_ingest_query_realtime(&base_url, admin, dir.path()).unwrap();
}

fn assert_doctor_exit_codes(base_url: &str, admin: &str) {
    assert_eq!(
        run([
            "rotameter",
            "doctor",
            "--remote",
            "--url",
            base_url,
            "--token",
            admin
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "doctor",
            "--remote",
            "--url",
            base_url,
            "--token",
            "rm_adm_invalid"
        ]),
        ExitCode::from(EXIT_AUTH)
    );
    assert_eq!(
        run([
            "rotameter",
            "doctor",
            "--remote",
            "--url",
            "http://127.0.0.1:1"
        ]),
        ExitCode::from(EXIT_HEALTH)
    );
}

fn assert_ingest_query_realtime(base_url: &str, admin: &str, dir: &Path) -> std::io::Result<()> {
    assert_eq!(
        run([
            "rotameter",
            "site",
            "upsert",
            "colinwolff.dev",
            "--url",
            base_url,
            "--token",
            admin
        ]),
        ExitCode::from(EXIT_OK)
    );
    let jsonl = dir.join("remote_hits.jsonl");
    std::fs::write(
        &jsonl,
        "{\"url\":\"https://colinwolff.dev/docs\",\"visitor_hint\":\"v1\"}\n",
    )?;
    let jsonl_str = jsonl.to_string_lossy();
    assert_eq!(
        run([
            "rotameter",
            "import",
            "colinwolff.dev",
            &jsonl_str,
            "--url",
            base_url,
            "--token",
            admin
        ]),
        ExitCode::from(EXIT_OK)
    );
    std::thread::sleep(Duration::from_millis(150));

    assert_eq!(
        run([
            "rotameter",
            "query",
            "colinwolff.dev",
            "--range",
            "24h",
            "--group-by",
            "path",
            "--filter",
            "path^=/docs",
            "--url",
            base_url,
            "--token",
            admin,
            "--format",
            "csv"
        ]),
        ExitCode::from(EXIT_OK)
    );
    assert_eq!(
        run([
            "rotameter",
            "realtime",
            "colinwolff.dev",
            "--watch",
            "--interval",
            "1",
            "--max-ticks",
            "1",
            "--url",
            base_url,
            "--token",
            admin
        ]),
        ExitCode::from(EXIT_OK)
    );
    Ok(())
}

fn wait_for_port(addr: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
