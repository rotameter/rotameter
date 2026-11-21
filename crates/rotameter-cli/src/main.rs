#![forbid(unsafe_code)]
//! Unified `rotameter` CLI binary entrypoint (MVP `serve`, `--version`, `--help`).

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None | Some("-h" | "--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("{}", rotameter_cli::version_banner());
            ExitCode::SUCCESS
        }
        Some("serve") => run_serve(args),
        Some(other) => {
            eprintln!("unknown subcommand `{other}` (run `rotameter --help`)");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "{}\n\nUSAGE:\n  rotameter serve [--bind ADDR] [--db PATH] [--bootstrap-admin-key KEY]\n\nENV:\n  ROTAMETER_BIND                Bind socket address (default: 127.0.0.1:8000)\n  ROTAMETER_DB                  SQLite database path (default: rotameter.db)\n  ROTAMETER_BOOTSTRAP_ADMIN_KEY Optional rm_adm_... key seeded on startup",
        rotameter_cli::version_banner()
    );
}

#[cfg(all(feature = "server", feature = "sqlite"))]
fn run_serve(args: impl Iterator<Item = String>) -> ExitCode {
    let Ok((config, db_path)) = parse_serve_args(args) else {
        return ExitCode::from(2);
    };
    rotameter_server::init_tracing();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    let store = match rotameter_cli::serve_store::SqliteServerStore::open(&db_path, now_ms) {
        Ok(store) => std::sync::Arc::new(store),
        Err(err) => {
            eprintln!("failed to open SQLite store at `{db_path}`: {err}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        eprintln!("failed to construct Tokio runtime");
        return ExitCode::FAILURE;
    };

    if let Err(err) = rt.block_on(rotameter_server::serve(config, store)) {
        eprintln!("server exited with error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(not(all(feature = "server", feature = "sqlite")))]
fn run_serve(_args: impl Iterator<Item = String>) -> ExitCode {
    eprintln!("`rotameter serve` requires the `server` and `sqlite` cargo features");
    ExitCode::from(2)
}

#[cfg(all(feature = "server", feature = "sqlite"))]
fn parse_serve_args(
    mut args: impl Iterator<Item = String>,
) -> Result<(rotameter_server::ServerConfig, String), ()> {
    let mut config = rotameter_server::ServerConfig {
        bootstrap_admin_key: env::var("ROTAMETER_BOOTSTRAP_ADMIN_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty()),
        ..rotameter_server::ServerConfig::default()
    };
    if let Ok(raw_bind) = env::var("ROTAMETER_BIND")
        && !raw_bind.trim().is_empty()
    {
        config.bind_addr = raw_bind
            .trim()
            .parse()
            .map_err(|err| eprintln!("invalid ROTAMETER_BIND `{raw_bind}`: {err}"))?;
    }
    let mut db_path = env::var("ROTAMETER_DB")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "rotameter.db".to_string());

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                let Some(val) = args.next() else {
                    eprintln!("`--bind` requires an address argument");
                    return Err(());
                };
                config.bind_addr = val
                    .parse()
                    .map_err(|err| eprintln!("invalid `--bind` address `{val}`: {err}"))?;
            }
            "--db" => {
                let Some(val) = args.next() else {
                    eprintln!("`--db` requires a path argument");
                    return Err(());
                };
                db_path = val;
            }
            "--bootstrap-admin-key" => {
                let Some(val) = args.next() else {
                    eprintln!("`--bootstrap-admin-key` requires a token argument");
                    return Err(());
                };
                config.bootstrap_admin_key = Some(val);
            }
            unknown => {
                eprintln!("unknown `serve` flag `{unknown}`");
                return Err(());
            }
        }
    }
    Ok((config, db_path))
}
