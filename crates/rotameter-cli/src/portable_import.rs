#![forbid(unsafe_code)]
//! Verified restore importer reading portable Parquet bundles and `.tar.zst` archives.

use std::fs;
use std::path::Path;

use rotameter_arrow::{
    PortableManifest, record_batch_to_events, record_batch_to_rollups_daily,
    record_batch_to_rollups_hourly, record_batch_to_rollups_path_daily, record_batch_to_sessions,
    stream_parquet_batches, unpack_tar_zst, verify_manifest,
};
use rotameter_core::storage::ControlPlaneSnapshot;
use rotameter_store_sqlite::SqliteStore;

use crate::cli::ExchangePlane;
use crate::exit::CliError;

const BATCH_SIZE: usize = 4_096;

pub(crate) fn restore_portable(
    source: &Path,
    dst: &SqliteStore,
    plane: ExchangePlane,
) -> Result<PortableManifest, CliError> {
    if source.is_file() || source.to_string_lossy().ends_with(".tar.zst") {
        let tmp = tempfile::tempdir()
            .map_err(|err| CliError::runtime(format!("cannot create unpack staging dir: {err}")))?;
        unpack_tar_zst(source, tmp.path()).map_err(|err| {
            CliError::runtime(format!("failed to unpack `{}`: {err}", source.display()))
        })?;
        restore_from_dir(tmp.path(), dst, plane)
    } else if source.is_dir() {
        restore_from_dir(source, dst, plane)
    } else {
        Err(CliError::usage(format!(
            "portable source `{}` does not exist",
            source.display()
        )))
    }
}

pub(crate) fn restore_from_dir(
    dir: &Path,
    dst: &SqliteStore,
    plane: ExchangePlane,
) -> Result<PortableManifest, CliError> {
    let manifest = verify_manifest(dir)
        .map_err(|err| CliError::runtime(format!("portable backup verification failed: {err}")))?;

    if plane.includes_control() {
        let raw = fs::read(dir.join("control_plane.json"))
            .map_err(|err| CliError::runtime(format!("cannot read control_plane.json: {err}")))?;
        let ctrl: ControlPlaneSnapshot = serde_json::from_slice(&raw)
            .map_err(|err| CliError::runtime(format!("invalid control_plane.json: {err}")))?;
        dst.import_control_plane_sync(&ctrl)?;
    }

    if plane.includes_analytics() {
        restore_analytics_files(dir, &manifest, dst)?;
    }

    Ok(manifest)
}

fn restore_analytics_files(
    dir: &Path,
    manifest: &PortableManifest,
    dst: &SqliteStore,
) -> Result<(), CliError> {
    for rel in manifest.files.keys() {
        if rel.starts_with("events/") && rel.ends_with(".parquet") {
            stream_parquet_batches(&dir.join(rel), BATCH_SIZE, |batch| {
                let rows = record_batch_to_events(&batch)?;
                dst.import_events_chunk_sync(&rows)?;
                Ok(())
            })
            .map_err(|err| CliError::runtime(err.to_string()))?;
        } else if rel.starts_with("sessions/") && rel.ends_with(".parquet") {
            stream_parquet_batches(&dir.join(rel), BATCH_SIZE, |batch| {
                let rows = record_batch_to_sessions(&batch)?;
                dst.import_sessions_chunk_sync(&rows)?;
                Ok(())
            })
            .map_err(|err| CliError::runtime(err.to_string()))?;
        }
    }

    let rh = dir.join("rollups_hourly.parquet");
    if rh.exists() {
        stream_parquet_batches(&rh, BATCH_SIZE, |batch| {
            let rows = record_batch_to_rollups_hourly(&batch)?;
            dst.import_rollups_hourly_chunk_sync(&rows)?;
            Ok(())
        })
        .map_err(|err| CliError::runtime(err.to_string()))?;
    }

    let rd = dir.join("rollups_daily.parquet");
    if rd.exists() {
        stream_parquet_batches(&rd, BATCH_SIZE, |batch| {
            let rows = record_batch_to_rollups_daily(&batch)?;
            dst.import_rollups_daily_chunk_sync(&rows)?;
            Ok(())
        })
        .map_err(|err| CliError::runtime(err.to_string()))?;
    }

    let rpd = dir.join("rollups_path_daily.parquet");
    if rpd.exists() {
        stream_parquet_batches(&rpd, BATCH_SIZE, |batch| {
            let rows = record_batch_to_rollups_path_daily(&batch)?;
            dst.import_rollups_path_daily_chunk_sync(&rows)?;
            Ok(())
        })
        .map_err(|err| CliError::runtime(err.to_string()))?;
    }

    Ok(())
}
