#![forbid(unsafe_code)]
//! Portable directory & `.tar.zst` exporter from [`SqliteStore`] into Arrow/Parquet bundles.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::DateTime;
use rotameter_arrow::{
    ARROW_SCHEMA_VERSION, PORTABLE_FORMAT_VERSION, PartitionedParquetWriter, PortableManifest,
    PortableRowCounts, events_schema, events_to_record_batch, pack_tar_zst, rollups_daily_schema,
    rollups_daily_to_record_batch, rollups_hourly_schema, rollups_hourly_to_record_batch,
    rollups_path_daily_schema, rollups_path_daily_to_record_batch, sessions_schema,
    sessions_to_record_batch, sha256_file, write_manifest, write_parquet_batches,
};
use rotameter_core::storage::{IngestEvent, SessionUpsert};
use rotameter_store_sqlite::SqliteStore;

use crate::exit::CliError;

const CHUNK_SIZE: usize = 4_096;

macro_rules! export_rollup {
    ($store:expr, $dir:expr, $files:expr, $rel:expr, $schema:ident, $read:ident, $encode:ident) => {{
        let mut offset = 0_usize;
        let iter = std::iter::from_fn(|| {
            let chunk = match $store.$read(offset, CHUNK_SIZE) {
                Ok(rows) => rows,
                Err(err) => return Some(Err(rotameter_arrow::ArrowError::Core(err))),
            };
            if chunk.is_empty() {
                return None;
            }
            offset += chunk.len();
            Some($encode(&chunk))
        });
        let total = write_parquet_batches(&$dir.join($rel), $schema(), iter)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        record_file_hash($dir, $rel, $files)?;
        Ok::<u64, CliError>(total)
    }};
}

pub(crate) fn export_portable(
    store: &SqliteStore,
    dest: &Path,
    archive: bool,
    now_ms: i64,
) -> Result<PortableManifest, CliError> {
    if archive {
        let tmp = tempfile::tempdir().map_err(|err| {
            CliError::runtime(format!("cannot create archive staging dir: {err}"))
        })?;
        let manifest = export_to_dir(store, tmp.path(), now_ms)?;
        pack_tar_zst(tmp.path(), dest).map_err(|err| {
            CliError::runtime(format!("failed to pack `{}`: {err}", dest.display()))
        })?;
        Ok(manifest)
    } else {
        export_to_dir(store, dest, now_ms)
    }
}

pub(crate) fn export_to_dir(
    store: &SqliteStore,
    dir: &Path,
    now_ms: i64,
) -> Result<PortableManifest, CliError> {
    fs::create_dir_all(dir)
        .map_err(|err| CliError::runtime(format!("cannot create `{}`: {err}", dir.display())))?;

    let ctrl = store.export_control_plane_sync()?;
    let mut files = BTreeMap::new();
    let ctrl_path = dir.join("control_plane.json");
    let ctrl_bytes = serde_json::to_vec_pretty(&ctrl)
        .map_err(|err| CliError::runtime(format!("control-plane JSON error: {err}")))?;
    fs::write(&ctrl_path, ctrl_bytes)
        .map_err(|err| CliError::runtime(format!("cannot write control_plane.json: {err}")))?;
    record_file_hash(dir, "control_plane.json", &mut files)?;

    let ev_rows = export_events_partitioned(store, dir, &mut files)?;
    let sess_rows = export_sessions_partitioned(store, dir, &mut files)?;
    let hourly_rows = export_rollup!(
        store,
        dir,
        &mut files,
        "rollups_hourly.parquet",
        rollups_hourly_schema,
        read_rollups_hourly_chunk_sync,
        rollups_hourly_to_record_batch
    )?;
    let daily_rows = export_rollup!(
        store,
        dir,
        &mut files,
        "rollups_daily.parquet",
        rollups_daily_schema,
        read_rollups_daily_chunk_sync,
        rollups_daily_to_record_batch
    )?;
    let path_daily_rows = export_rollup!(
        store,
        dir,
        &mut files,
        "rollups_path_daily.parquet",
        rollups_path_daily_schema,
        read_rollups_path_daily_chunk_sync,
        rollups_path_daily_to_record_batch
    )?;

    let schema_version = store.schema_version()?;
    let manifest = PortableManifest {
        format_version: PORTABLE_FORMAT_VERSION,
        rotameter_version: rotameter_core::WORKSPACE_VERSION.to_string(),
        schema_version,
        arrow_schema_version: ARROW_SCHEMA_VERSION,
        created_at_ms: now_ms,
        row_counts: PortableRowCounts {
            orgs: u64::try_from(ctrl.orgs.len()).unwrap_or(0),
            sites: u64::try_from(ctrl.sites.len()).unwrap_or(0),
            api_keys: u64::try_from(ctrl.api_keys.len()).unwrap_or(0),
            rotating_salts: u64::try_from(ctrl.rotating_salts.len()).unwrap_or(0),
            events: ev_rows,
            sessions: sess_rows,
            rollups_hourly: hourly_rows,
            rollups_daily: daily_rows,
            rollups_path_daily: path_daily_rows,
        },
        files,
    };
    write_manifest(dir, &manifest).map_err(|err| CliError::runtime(err.to_string()))?;
    Ok(manifest)
}

fn export_events_partitioned(
    store: &SqliteStore,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<u64, CliError> {
    let mut writer = PartitionedParquetWriter::new(dir, "events", events_schema());
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = store.read_events_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let mut by_day: BTreeMap<String, Vec<IngestEvent>> = BTreeMap::new();
        for ev in chunk {
            by_day.entry(utc_day_label(ev.ts_ms)).or_default().push(ev);
        }
        for (day, rows) in by_day {
            let batch = events_to_record_batch(&rows)
                .map_err(|err| CliError::runtime(format!("Arrow events encode failed: {err}")))?;
            writer
                .write_day_batch(&day, &batch)
                .map_err(|err| CliError::runtime(err.to_string()))?;
        }
    }
    let written = writer
        .finish()
        .map_err(|err| CliError::runtime(err.to_string()))?;
    for (rel, _) in written {
        record_file_hash(dir, &rel, files)?;
    }
    Ok(total)
}

fn export_sessions_partitioned(
    store: &SqliteStore,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<u64, CliError> {
    let mut writer = PartitionedParquetWriter::new(dir, "sessions", sessions_schema());
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = store.read_sessions_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let mut by_day: BTreeMap<String, Vec<SessionUpsert>> = BTreeMap::new();
        for s in chunk {
            by_day
                .entry(utc_day_label(s.started_at_ms))
                .or_default()
                .push(s);
        }
        for (day, rows) in by_day {
            let batch = sessions_to_record_batch(&rows)
                .map_err(|err| CliError::runtime(format!("Arrow sessions encode failed: {err}")))?;
            writer
                .write_day_batch(&day, &batch)
                .map_err(|err| CliError::runtime(err.to_string()))?;
        }
    }
    let written = writer
        .finish()
        .map_err(|err| CliError::runtime(err.to_string()))?;
    for (rel, _) in written {
        record_file_hash(dir, &rel, files)?;
    }
    Ok(total)
}

fn record_file_hash(
    root: &Path,
    rel: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<(), CliError> {
    let digest = sha256_file(&root.join(rel)).map_err(|err| CliError::runtime(err.to_string()))?;
    files.insert(rel.to_string(), digest);
    Ok(())
}

fn utc_day_label(ts_ms: i64) -> String {
    DateTime::from_timestamp_millis(ts_ms).map_or_else(
        || "1970-01-01".to_string(),
        |dt| dt.format("%Y-%m-%d").to_string(),
    )
}
