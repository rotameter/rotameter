#![forbid(unsafe_code)]
//! Direct in-memory Arrow `RecordBatch` streaming and staged migration between storage stores.

use std::path::Path;

use rotameter_arrow::{
    PortableRowCounts, events_to_record_batch, record_batch_to_events,
    record_batch_to_rollups_daily, record_batch_to_rollups_hourly,
    record_batch_to_rollups_path_daily, record_batch_to_sessions, rollups_daily_to_record_batch,
    rollups_hourly_to_record_batch, rollups_path_daily_to_record_batch, sessions_to_record_batch,
};
use rotameter_store_sqlite::SqliteStore;

use crate::cli::ExchangePlane;
use crate::exit::CliError;
use crate::portable_export::export_portable;
use crate::portable_import::restore_portable;

const CHUNK_SIZE: usize = 4_096;

pub(crate) fn migrate_stores(
    src: &SqliteStore,
    dst: &SqliteStore,
    plane: ExchangePlane,
    stage_dir: Option<&Path>,
    now_ms: i64,
) -> Result<PortableRowCounts, CliError> {
    if let Some(stage) = stage_dir {
        let is_archive = stage.to_string_lossy().ends_with(".tar.zst");
        let manifest = export_portable(src, stage, is_archive, now_ms)?;
        restore_portable(stage, dst, plane)?;
        return Ok(manifest.row_counts);
    }

    let mut counts = PortableRowCounts::default();
    if plane.includes_control() {
        let ctrl = src.export_control_plane_sync()?;
        counts.orgs = u64::try_from(ctrl.orgs.len()).unwrap_or(0);
        counts.sites = u64::try_from(ctrl.sites.len()).unwrap_or(0);
        counts.api_keys = u64::try_from(ctrl.api_keys.len()).unwrap_or(0);
        counts.rotating_salts = u64::try_from(ctrl.rotating_salts.len()).unwrap_or(0);
        dst.import_control_plane_sync(&ctrl)?;
    }

    if plane.includes_analytics() {
        counts.events = stream_events(src, dst)?;
        counts.sessions = stream_sessions(src, dst)?;
        counts.rollups_hourly = stream_rollups_hourly(src, dst)?;
        counts.rollups_daily = stream_rollups_daily(src, dst)?;
        counts.rollups_path_daily = stream_rollups_path_daily(src, dst)?;
    }

    Ok(counts)
}

fn stream_events(src: &SqliteStore, dst: &SqliteStore) -> Result<u64, CliError> {
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = src.read_events_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let batch = events_to_record_batch(&chunk)
            .map_err(|err| CliError::runtime(format!("Arrow events encode failed: {err}")))?;
        let decoded = record_batch_to_events(&batch)
            .map_err(|err| CliError::runtime(format!("Arrow events decode failed: {err}")))?;
        dst.import_events_chunk_sync(&decoded)?;
    }
    Ok(total)
}

fn stream_sessions(src: &SqliteStore, dst: &SqliteStore) -> Result<u64, CliError> {
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = src.read_sessions_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let batch = sessions_to_record_batch(&chunk)
            .map_err(|err| CliError::runtime(format!("Arrow sessions encode failed: {err}")))?;
        let decoded = record_batch_to_sessions(&batch)
            .map_err(|err| CliError::runtime(format!("Arrow sessions decode failed: {err}")))?;
        dst.import_sessions_chunk_sync(&decoded)?;
    }
    Ok(total)
}

fn stream_rollups_hourly(src: &SqliteStore, dst: &SqliteStore) -> Result<u64, CliError> {
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = src.read_rollups_hourly_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let batch = rollups_hourly_to_record_batch(&chunk)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        let decoded = record_batch_to_rollups_hourly(&batch)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        dst.import_rollups_hourly_chunk_sync(&decoded)?;
    }
    Ok(total)
}

fn stream_rollups_daily(src: &SqliteStore, dst: &SqliteStore) -> Result<u64, CliError> {
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = src.read_rollups_daily_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let batch = rollups_daily_to_record_batch(&chunk)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        let decoded = record_batch_to_rollups_daily(&batch)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        dst.import_rollups_daily_chunk_sync(&decoded)?;
    }
    Ok(total)
}

fn stream_rollups_path_daily(src: &SqliteStore, dst: &SqliteStore) -> Result<u64, CliError> {
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = src.read_rollups_path_daily_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let batch = rollups_path_daily_to_record_batch(&chunk)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        let decoded = record_batch_to_rollups_path_daily(&batch)
            .map_err(|err| CliError::runtime(err.to_string()))?;
        dst.import_rollups_path_daily_chunk_sync(&decoded)?;
    }
    Ok(total)
}
