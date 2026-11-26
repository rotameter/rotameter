#![forbid(unsafe_code)]
//! Zero-copy Arrow streaming adapters targeting [`DuckDbStore`] for `restore` and `migrate`.

use std::path::Path;

use rotameter_arrow::{
    PortableManifest, PortableRowCounts, events_to_record_batch, rollups_daily_to_record_batch,
    rollups_hourly_to_record_batch, rollups_path_daily_to_record_batch, sessions_to_record_batch,
    stream_parquet_batches, unpack_tar_zst, verify_manifest,
};
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};
use rotameter_store_sqlite::SqliteStore;

use crate::cli::ExchangePlane;
use crate::exit::CliError;
use crate::portable_export::export_portable;
use crate::serve_parse::{env_non_empty, env_u32};

const CHUNK_SIZE: usize = 4_096;

fn duckdb_config_from_env() -> Result<DuckDbConfig, CliError> {
    let mut cfg = DuckDbConfig::default();
    let env_fn = |k: &str| std::env::var(k).ok();
    if let Some(limit) = env_non_empty(&env_fn, "ROTAMETER_DUCKDB_MEMORY_LIMIT") {
        cfg.memory_limit = Some(limit);
    }
    if let Some(threads) = env_u32(&env_fn, "ROTAMETER_DUCKDB_THREADS")? {
        cfg.threads = Some(threads);
    }
    Ok(cfg)
}

pub(crate) fn restore_to_duckdb(
    source: &Path,
    duck_path: &Path,
    plane: ExchangePlane,
) -> Result<PortableManifest, CliError> {
    if plane != ExchangePlane::Analytics {
        return Err(CliError::usage(
            "DuckDB is analytics-only; pass `--plane analytics` when restoring to `duckdb://`",
        ));
    }
    let cfg = duckdb_config_from_env()?;
    let dst = DuckDbStore::open(duck_path, &cfg)?;
    if source.is_file() || source.to_string_lossy().ends_with(".tar.zst") {
        let tmp = tempfile::tempdir()
            .map_err(|err| CliError::runtime(format!("cannot create unpack staging dir: {err}")))?;
        unpack_tar_zst(source, tmp.path()).map_err(|err| {
            CliError::runtime(format!("failed to unpack `{}`: {err}", source.display()))
        })?;
        restore_dir_to_duckdb(tmp.path(), &dst)
    } else if source.is_dir() {
        restore_dir_to_duckdb(source, &dst)
    } else {
        Err(CliError::usage(format!(
            "portable source `{}` does not exist",
            source.display()
        )))
    }
}

fn restore_dir_to_duckdb(dir: &Path, dst: &DuckDbStore) -> Result<PortableManifest, CliError> {
    let manifest = verify_manifest(dir)
        .map_err(|err| CliError::runtime(format!("portable backup verification failed: {err}")))?;

    for rel in manifest.files.keys() {
        if rel.starts_with("events/") && rel.ends_with(".parquet") {
            stream_parquet_batches(&dir.join(rel), CHUNK_SIZE, |rb| {
                dst.append_events_batch_sync(rb)?;
                Ok(())
            })
            .map_err(|err| CliError::runtime(err.to_string()))?;
        } else if rel.starts_with("sessions/") && rel.ends_with(".parquet") {
            stream_parquet_batches(&dir.join(rel), CHUNK_SIZE, |rb| {
                dst.append_sessions_batch_sync(rb)?;
                Ok(())
            })
            .map_err(|err| CliError::runtime(err.to_string()))?;
        }
    }

    for (file, table) in [
        ("rollups_hourly.parquet", "rollups_hourly"),
        ("rollups_daily.parquet", "rollups_daily"),
        ("rollups_path_daily.parquet", "rollups_path_daily"),
    ] {
        let p = dir.join(file);
        if p.exists() {
            stream_parquet_batches(&p, CHUNK_SIZE, |rb| {
                dst.append_rollup_batch_sync(table, rb)?;
                Ok(())
            })
            .map_err(|err| CliError::runtime(err.to_string()))?;
        }
    }
    Ok(manifest)
}

pub(crate) fn migrate_sqlite_to_duckdb(
    src: &SqliteStore,
    duck_path: &Path,
    plane: ExchangePlane,
    stage_dir: Option<&Path>,
    now_ms: i64,
) -> Result<PortableRowCounts, CliError> {
    if plane != ExchangePlane::Analytics {
        return Err(CliError::usage(
            "DuckDB is analytics-only; pass `--plane analytics` when migrating to `duckdb://`",
        ));
    }
    if let Some(stage) = stage_dir {
        let is_archive = stage.to_string_lossy().ends_with(".tar.zst");
        let manifest = export_portable(src, stage, is_archive, now_ms)?;
        restore_to_duckdb(stage, duck_path, plane)?;
        return Ok(manifest.row_counts);
    }

    let cfg = duckdb_config_from_env()?;
    let dst = DuckDbStore::open(duck_path, &cfg)?;
    let mut counts = PortableRowCounts::default();

    let mut offset = 0_usize;
    loop {
        let chunk = src.read_events_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        counts.events += u64::try_from(chunk.len()).unwrap_or(0);
        let rb =
            events_to_record_batch(&chunk).map_err(|err| CliError::runtime(err.to_string()))?;
        dst.append_events_batch_sync(rb)?;
    }

    offset = 0;
    loop {
        let chunk = src.read_sessions_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        counts.sessions += u64::try_from(chunk.len()).unwrap_or(0);
        let rb =
            sessions_to_record_batch(&chunk).map_err(|err| CliError::runtime(err.to_string()))?;
        dst.append_sessions_batch_sync(rb)?;
    }

    counts.rollups_hourly = stream_rollups(
        src,
        &dst,
        "rollups_hourly",
        SqliteStore::read_rollups_hourly_chunk_sync,
        rollups_hourly_to_record_batch,
    )?;
    counts.rollups_daily = stream_rollups(
        src,
        &dst,
        "rollups_daily",
        SqliteStore::read_rollups_daily_chunk_sync,
        rollups_daily_to_record_batch,
    )?;
    counts.rollups_path_daily = stream_rollups(
        src,
        &dst,
        "rollups_path_daily",
        SqliteStore::read_rollups_path_daily_chunk_sync,
        rollups_path_daily_to_record_batch,
    )?;
    dst.checkpoint()?;
    Ok(counts)
}

fn stream_rollups<T, R, E>(
    src: &SqliteStore,
    dst: &DuckDbStore,
    table: &str,
    read_fn: R,
    encode_fn: E,
) -> Result<u64, CliError>
where
    R: Fn(&SqliteStore, usize, usize) -> Result<Vec<T>, rotameter_core::error::CoreError>,
    E: Fn(&[T]) -> Result<rotameter_arrow::RecordBatch, rotameter_arrow::ArrowError>,
{
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = read_fn(src, offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let rb = encode_fn(&chunk).map_err(|err| CliError::runtime(err.to_string()))?;
        dst.append_rollup_batch_sync(table, rb)?;
    }
    Ok(total)
}
