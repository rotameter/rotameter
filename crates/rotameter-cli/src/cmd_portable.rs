#![forbid(unsafe_code)]
//! CLI dispatchers for `backup --format portable`, `restore`, and `migrate`.

use crate::cli::{BackupArgs, GlobalOpts, MigrateArgs, RestoreArgs};
use crate::exit::CliError;

#[cfg(all(feature = "sqlite", feature = "arrow"))]
pub(crate) fn run_portable_backup(global: &GlobalOpts, args: &BackupArgs) -> Result<(), CliError> {
    let (store, now_ms, _) = crate::context::open_local_store(global)?;
    let archive = args.archive || args.dest.to_string_lossy().ends_with(".tar.zst");
    let manifest = crate::portable_export::export_portable(&store, &args.dest, archive, now_ms)?;
    let payload = serde_json::json!({
        "dest": args.dest.display().to_string(),
        "format": if archive { "portable-tar-zst" } else { "portable" },
        "events": manifest.row_counts.events,
        "sessions": manifest.row_counts.sessions,
        "files": manifest.files.len(),
        "status": "ok",
    });
    crate::context::print_formatted(
        global.format,
        &["DEST", "FORMAT", "EVENTS", "SESSIONS", "STATUS"],
        &[vec![
            args.dest.display().to_string(),
            if archive {
                "portable-tar-zst"
            } else {
                "portable"
            }
            .to_string(),
            manifest.row_counts.events.to_string(),
            manifest.row_counts.sessions.to_string(),
            "ok".to_string(),
        ]],
        &payload,
        global.quiet,
    )
}

#[cfg(all(feature = "sqlite", feature = "arrow"))]
pub(crate) fn run_restore(global: &GlobalOpts, args: &RestoreArgs) -> Result<(), CliError> {
    if let Some(dsn) = args.to.as_deref()
        && let crate::storage_dsn::StorageDsn::DuckDb(duck_path) =
            crate::storage_dsn::StorageDsn::parse(dsn)?
    {
        #[cfg(feature = "duckdb")]
        {
            let manifest =
                crate::portable_duckdb::restore_to_duckdb(&args.source, &duck_path, args.plane)?;
            let payload = serde_json::json!({
                "source": args.source.display().to_string(),
                "plane": args.plane.as_str(),
                "events": manifest.row_counts.events,
                "sessions": manifest.row_counts.sessions,
                "status": "ok",
            });
            return crate::context::print_formatted(
                global.format,
                &["SOURCE", "PLANE", "EVENTS", "SESSIONS", "STATUS"],
                &[vec![
                    args.source.display().to_string(),
                    args.plane.as_str().to_string(),
                    manifest.row_counts.events.to_string(),
                    manifest.row_counts.sessions.to_string(),
                    "ok".to_string(),
                ]],
                &payload,
                global.quiet,
            );
        }
        #[cfg(not(feature = "duckdb"))]
        {
            let _ = duck_path;
            return Err(CliError::usage(
                "DuckDB target requires `--features duckdb`",
            ));
        }
    }

    let dst = open_dsn_store(args.to.as_deref(), global)?;
    let manifest = crate::portable_import::restore_portable(&args.source, &dst, args.plane)?;
    let payload = serde_json::json!({
        "source": args.source.display().to_string(),
        "plane": args.plane.as_str(),
        "events": manifest.row_counts.events,
        "sessions": manifest.row_counts.sessions,
        "status": "ok",
    });
    crate::context::print_formatted(
        global.format,
        &["SOURCE", "PLANE", "EVENTS", "SESSIONS", "STATUS"],
        &[vec![
            args.source.display().to_string(),
            args.plane.as_str().to_string(),
            manifest.row_counts.events.to_string(),
            manifest.row_counts.sessions.to_string(),
            "ok".to_string(),
        ]],
        &payload,
        global.quiet,
    )
}

#[cfg(all(feature = "sqlite", feature = "arrow"))]
pub(crate) fn run_migrate(args: &MigrateArgs) -> Result<(), CliError> {
    let now_ms = crate::format::now_ms();
    let src = open_dsn_sqlite(&args.from, now_ms)?;
    let counts = match crate::storage_dsn::StorageDsn::parse(&args.to)? {
        crate::storage_dsn::StorageDsn::Sqlite(dst_path) => {
            let dst = rotameter_store_sqlite::SqliteStore::open(
                &dst_path,
                rotameter_store_sqlite::SqliteConfig::default(),
                now_ms,
            )?;
            crate::portable_migrate::migrate_stores(
                &src,
                &dst,
                args.plane,
                args.stage_dir.as_deref(),
                now_ms,
            )?
        }
        crate::storage_dsn::StorageDsn::DuckDb(duck_path) => {
            #[cfg(feature = "duckdb")]
            {
                crate::portable_duckdb::migrate_sqlite_to_duckdb(
                    &src,
                    &duck_path,
                    args.plane,
                    args.stage_dir.as_deref(),
                    now_ms,
                )?
            }
            #[cfg(not(feature = "duckdb"))]
            {
                let _ = duck_path;
                return Err(CliError::usage(
                    "DuckDB target requires `--features duckdb`",
                ));
            }
        }
    };
    println!(
        "migrated plane={} events={} sessions={} rollups_hourly={} rollups_daily={} rollups_path_daily={}",
        args.plane.as_str(),
        counts.events,
        counts.sessions,
        counts.rollups_hourly,
        counts.rollups_daily,
        counts.rollups_path_daily
    );
    Ok(())
}

#[cfg(all(feature = "sqlite", feature = "arrow"))]
fn open_dsn_store(
    explicit_to: Option<&str>,
    global: &GlobalOpts,
) -> Result<rotameter_store_sqlite::SqliteStore, CliError> {
    if let Some(dsn) = explicit_to {
        return open_dsn_sqlite(dsn, crate::format::now_ms());
    }
    let (store, _, _) = crate::context::open_local_store(global)?;
    Ok(store)
}

#[cfg(all(feature = "sqlite", feature = "arrow"))]
fn open_dsn_sqlite(
    raw: &str,
    now_ms: i64,
) -> Result<rotameter_store_sqlite::SqliteStore, CliError> {
    use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

    use crate::storage_dsn::StorageDsn;

    match StorageDsn::parse(raw)? {
        StorageDsn::Sqlite(path) => Ok(SqliteStore::open(&path, SqliteConfig::default(), now_ms)?),
        StorageDsn::DuckDb(_) => Err(CliError::usage(
            "DuckDB target/source requires `--features duckdb` (coming in RM-34)",
        )),
    }
}

#[cfg(not(all(feature = "sqlite", feature = "arrow")))]
pub(crate) fn run_portable_backup(
    _global: &GlobalOpts,
    _args: &BackupArgs,
) -> Result<(), CliError> {
    Err(CliError::usage(
        "portable backup requires `--features arrow` (and `sqlite`)",
    ))
}

#[cfg(not(all(feature = "sqlite", feature = "arrow")))]
pub(crate) fn run_restore(_global: &GlobalOpts, _args: &RestoreArgs) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter restore` requires `--features arrow` (and `sqlite`)",
    ))
}

#[cfg(not(all(feature = "sqlite", feature = "arrow")))]
pub(crate) fn run_migrate(_args: &MigrateArgs) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter migrate` requires `--features arrow` (and `sqlite`)",
    ))
}
