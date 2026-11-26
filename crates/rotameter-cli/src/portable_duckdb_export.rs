#![forbid(unsafe_code)]
//! Portable directory & `.tar.zst` exporter for split `SQLite` (control) + `DuckDB` (analytics).

use std::fs;
use std::path::Path;

use rotameter_arrow::{
    ARROW_SCHEMA_VERSION, PORTABLE_FORMAT_VERSION, PortableManifest, pack_tar_zst, sha256_file,
    write_manifest,
};
use rotameter_store_duckdb::DuckDbStore;
use rotameter_store_sqlite::SqliteStore;

use crate::exit::CliError;
use crate::portable_duckdb::duckdb_config_from_env;

pub(crate) fn export_duckdb_path_portable(
    control: &SqliteStore,
    duck_path: &Path,
    dest: &Path,
    archive: bool,
    now_ms: i64,
) -> Result<PortableManifest, CliError> {
    let cfg = duckdb_config_from_env()?;
    let analytics = DuckDbStore::open(duck_path, &cfg)?;
    export_split_duckdb_portable(control, &analytics, dest, archive, now_ms)
}

pub(crate) fn export_split_duckdb_portable(
    control: &SqliteStore,
    analytics: &DuckDbStore,
    dest: &Path,
    archive: bool,
    now_ms: i64,
) -> Result<PortableManifest, CliError> {
    if archive {
        let tmp = tempfile::tempdir().map_err(|err| {
            CliError::runtime(format!("cannot create archive staging dir: {err}"))
        })?;
        let manifest = export_split_duckdb_dir(control, analytics, tmp.path(), now_ms)?;
        pack_tar_zst(tmp.path(), dest).map_err(|err| {
            CliError::runtime(format!("failed to pack `{}`: {err}", dest.display()))
        })?;
        Ok(manifest)
    } else {
        export_split_duckdb_dir(control, analytics, dest, now_ms)
    }
}

fn export_split_duckdb_dir(
    control: &SqliteStore,
    analytics: &DuckDbStore,
    dir: &Path,
    now_ms: i64,
) -> Result<PortableManifest, CliError> {
    fs::create_dir_all(dir)
        .map_err(|err| CliError::runtime(format!("cannot create `{}`: {err}", dir.display())))?;
    let ctrl = control.export_control_plane_sync()?;
    let (mut files, mut counts) = analytics.export_analytics_dir_sync(dir)?;

    let ctrl_path = dir.join("control_plane.json");
    let ctrl_bytes = serde_json::to_vec_pretty(&ctrl)
        .map_err(|err| CliError::runtime(format!("control-plane JSON error: {err}")))?;
    fs::write(&ctrl_path, ctrl_bytes)
        .map_err(|err| CliError::runtime(format!("cannot write control_plane.json: {err}")))?;
    let ctrl_sha = sha256_file(&ctrl_path).map_err(|err| CliError::runtime(err.to_string()))?;
    files.insert("control_plane.json".to_string(), ctrl_sha);

    counts.orgs = u64::try_from(ctrl.orgs.len()).unwrap_or(0);
    counts.sites = u64::try_from(ctrl.sites.len()).unwrap_or(0);
    counts.api_keys = u64::try_from(ctrl.api_keys.len()).unwrap_or(0);
    counts.rotating_salts = u64::try_from(ctrl.rotating_salts.len()).unwrap_or(0);

    let manifest = PortableManifest {
        format_version: PORTABLE_FORMAT_VERSION,
        rotameter_version: rotameter_core::WORKSPACE_VERSION.to_string(),
        schema_version: control.schema_version()?,
        arrow_schema_version: ARROW_SCHEMA_VERSION,
        created_at_ms: now_ms,
        row_counts: counts,
        files,
    };
    write_manifest(dir, &manifest).map_err(|err| CliError::runtime(err.to_string()))?;
    Ok(manifest)
}
