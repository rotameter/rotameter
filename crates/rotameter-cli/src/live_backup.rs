#![forbid(unsafe_code)]
//! Live `LiveBackupExporter` implementation for `rotameter serve` (`POST /v1/admin/backup`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use rotameter_arrow::{pack_tar_zst, sha256_file, write_manifest};
use rotameter_core::error::CoreError;
use rotameter_core::storage::BoxFuture;
use rotameter_server::LiveBackupExporter;
#[cfg(feature = "duckdb")]
use rotameter_store_duckdb::DuckDbStore;
use rotameter_store_sqlite::SqliteStore;

use crate::portable_export::{export_portable, export_to_dir};

/// Online `.tar.zst` portable backup exporter wired into [`rotameter_server::ServerStores`].
pub(crate) enum LiveBackupAdapter {
    UnifiedSqlite(Arc<SqliteStore>),
    SplitSqlite {
        control: Arc<SqliteStore>,
        analytics: Arc<SqliteStore>,
    },
    #[cfg(feature = "duckdb")]
    SplitDuckDb {
        control: Arc<SqliteStore>,
        analytics: Arc<DuckDbStore>,
    },
}

impl LiveBackupExporter for LiveBackupAdapter {
    fn export_portable_tar_zst(&self) -> BoxFuture<'_, Result<Vec<u8>, CoreError>> {
        let now_ms = crate::format::now_ms();
        match self {
            Self::UnifiedSqlite(store) => {
                let s = Arc::clone(store);
                Box::pin(run_archive_blocking(move |archive| {
                    export_portable(&s, archive, true, now_ms)
                        .map_err(|err| CoreError::Storage(err.message))?;
                    Ok(())
                }))
            }
            Self::SplitSqlite { control, analytics } => {
                let ctrl = Arc::clone(control);
                let ana = Arc::clone(analytics);
                Box::pin(run_archive_blocking(move |archive| {
                    export_split_sqlite_archive(&ctrl, &ana, archive, now_ms)
                }))
            }
            #[cfg(feature = "duckdb")]
            Self::SplitDuckDb { control, analytics } => {
                let ctrl = Arc::clone(control);
                let ana = Arc::clone(analytics);
                Box::pin(run_archive_blocking(move |archive| {
                    crate::portable_duckdb_export::export_split_duckdb_portable(
                        &ctrl, &ana, archive, true, now_ms,
                    )
                    .map_err(|err| CoreError::Storage(err.message))?;
                    Ok(())
                }))
            }
        }
    }
}

fn export_split_sqlite_archive(
    control: &SqliteStore,
    analytics: &SqliteStore,
    archive_path: &Path,
    now_ms: i64,
) -> Result<(), CoreError> {
    let stage = tempfile::tempdir()
        .map_err(|err| CoreError::Storage(format!("cannot create backup stage dir: {err}")))?;
    let mut manifest = export_to_dir(analytics, stage.path(), now_ms)
        .map_err(|err| CoreError::Storage(err.message))?;
    let ctrl = control.export_control_plane_sync()?;
    let ctrl_path = stage.path().join("control_plane.json");
    let ctrl_bytes = serde_json::to_vec_pretty(&ctrl)
        .map_err(|err| CoreError::Storage(format!("control-plane JSON error: {err}")))?;
    fs::write(&ctrl_path, ctrl_bytes)
        .map_err(|err| CoreError::Storage(format!("cannot write control_plane.json: {err}")))?;
    let sha = sha256_file(&ctrl_path).map_err(|err| CoreError::Storage(err.to_string()))?;
    manifest.files.insert("control_plane.json".to_string(), sha);
    manifest.schema_version = control.schema_version()?;
    manifest.row_counts.orgs = u64::try_from(ctrl.orgs.len()).unwrap_or(0);
    manifest.row_counts.sites = u64::try_from(ctrl.sites.len()).unwrap_or(0);
    manifest.row_counts.api_keys = u64::try_from(ctrl.api_keys.len()).unwrap_or(0);
    manifest.row_counts.rotating_salts = u64::try_from(ctrl.rotating_salts.len()).unwrap_or(0);
    write_manifest(stage.path(), &manifest).map_err(|err| CoreError::Storage(err.to_string()))?;
    pack_tar_zst(stage.path(), archive_path).map_err(|err| CoreError::Storage(err.to_string()))
}

async fn run_archive_blocking<F>(f: F) -> Result<Vec<u8>, CoreError>
where
    F: FnOnce(&Path) -> Result<(), CoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let tmp = tempfile::tempdir()
            .map_err(|err| CoreError::Storage(format!("cannot create archive tempdir: {err}")))?;
        let archive = tmp.path().join("rotameter-backup.tar.zst");
        f(&archive)?;
        fs::read(&archive)
            .map_err(|err| CoreError::Storage(format!("cannot read backup archive: {err}")))
    })
    .await
    .map_err(|err| CoreError::Storage(format!("backup task panicked: {err}")))?
}
