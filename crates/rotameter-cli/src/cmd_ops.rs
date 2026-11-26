#![forbid(unsafe_code)]
//! `rotameter backup` (`VACUUM INTO`) and `rotameter doctor` health checks.

use crate::cli::{BackupArgs, BackupFormat, DoctorArgs, GlobalOpts};
use crate::context::{prefer_local_db, print_formatted, remote_client};
use crate::exit::CliError;

pub(crate) fn run_backup(global: &GlobalOpts, args: &BackupArgs) -> Result<(), CliError> {
    if args.format == BackupFormat::Portable
        || args.archive
        || args.dest.to_string_lossy().ends_with(".tar.zst")
    {
        return crate::cmd_portable::run_portable_backup(global, args);
    }
    #[cfg(feature = "sqlite")]
    {
        let (store, _, _) = crate::context::open_local_store(global)?;
        store.vacuum_into(&args.dest)?;
        let payload = serde_json::json!({
            "dest": args.dest.display().to_string(),
            "status": "ok",
        });
        print_formatted(
            global.format,
            &["DEST", "STATUS"],
            &[vec![args.dest.display().to_string(), "ok".to_string()]],
            &payload,
            global.quiet,
        )
    }
    #[cfg(not(feature = "sqlite"))]
    {
        let _ = (global, args);
        Err(CliError::usage(
            "`rotameter backup` requires the `sqlite` feature",
        ))
    }
}

pub(crate) fn run_doctor(global: &GlobalOpts, args: &DoctorArgs) -> Result<(), CliError> {
    if args.local && args.remote {
        return Err(CliError::usage(
            "cannot pass both `--local` and `--remote` to `doctor`",
        ));
    }
    let use_local = args.local || (!args.remote && prefer_local_db(global));
    if use_local {
        run_local_doctor(global)
    } else {
        run_remote_doctor(global)
    }
}

#[cfg(feature = "sqlite")]
fn run_local_doctor(global: &GlobalOpts) -> Result<(), CliError> {
    let (store, _, _) = crate::context::open_local_store(global)
        .map_err(|err| CliError::health(format!("cannot open local SQLite database: {err}")))?;
    let snap = store
        .health_snapshot()
        .map_err(|err| CliError::health(format!("SQLite health check failed: {err}")))?;
    let healthy = snap.journal_mode.eq_ignore_ascii_case("wal")
        && snap.integrity_ok
        && snap.schema_version >= 1;
    let payload = serde_json::json!({
        "mode": "local",
        "healthy": healthy,
        "journal_mode": snap.journal_mode,
        "schema_version": snap.schema_version,
        "cache_size": snap.cache_size,
        "mmap_size": snap.mmap_size,
        "integrity_ok": snap.integrity_ok,
    });
    print_formatted(
        global.format,
        &["MODE", "JOURNAL", "SCHEMA", "INTEGRITY", "HEALTHY"],
        &[vec![
            "local".to_string(),
            snap.journal_mode,
            snap.schema_version.to_string(),
            snap.integrity_ok.to_string(),
            healthy.to_string(),
        ]],
        &payload,
        global.quiet,
    )?;
    if !healthy {
        return Err(CliError::health("local SQLite WAL health check failed"));
    }
    Ok(())
}

#[cfg(not(feature = "sqlite"))]
fn run_local_doctor(_global: &GlobalOpts) -> Result<(), CliError> {
    Err(CliError::usage(
        "local `doctor` checks require the `sqlite` feature",
    ))
}

fn run_remote_doctor(global: &GlobalOpts) -> Result<(), CliError> {
    let client = remote_client(global).map_err(|err| CliError::health(err.message))?;
    let health = client.health().map_err(|err| {
        if err.is_auth_error() {
            CliError::auth(err.to_string())
        } else {
            CliError::health(format!("remote `/v1/health` check failed: {err}"))
        }
    })?;
    if client
        .config()
        .token
        .as_deref()
        .is_some_and(|t| t.starts_with("rm_adm_"))
    {
        client.list_sites().map_err(|err| {
            if err.is_auth_error() {
                CliError::auth(err.to_string())
            } else {
                CliError::health(format!("remote admin check failed: {err}"))
            }
        })?;
    }

    let ok = health.status.eq_ignore_ascii_case("ok");
    print_formatted(
        global.format,
        &["MODE", "STATUS", "VERSION", "SCHEMA", "QUEUE_PCT"],
        &[vec![
            "remote".to_string(),
            health.status.clone(),
            health.version.clone(),
            health.schema_version.to_string(),
            format!("{:.1}%", health.queue_fill_pct),
        ]],
        &health,
        global.quiet,
    )?;
    if !ok {
        return Err(CliError::health(format!(
            "remote server reported unhealthy status `{}`",
            health.status
        )));
    }
    Ok(())
}
