#![forbid(unsafe_code)]
//! Zero-downtime live portable backup handler (`POST /v1/admin/backup`, `rm_adm_` only).

use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rotameter_core::error::CoreError;
use rotameter_core::storage::BoxFuture;

use crate::auth::require_admin;
use crate::error::ApiError;
use crate::state::AppState;

/// Produces an online portable `.tar.zst` archive from the running server's stores.
pub trait LiveBackupExporter: Send + Sync {
    /// Exports control-plane and analytics state into `.tar.zst` bytes without stopping ingest.
    fn export_portable_tar_zst(&self) -> BoxFuture<'_, Result<Vec<u8>, CoreError>>;
}

pub(crate) async fn post_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _ = require_admin(&state, &headers, true).await?;
    let Some(exporter) = state.backup_exporter.as_ref() else {
        return Err(ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "not_implemented",
            "live portable backup requires a server built with `--features arrow`",
        ));
    };
    let bytes = exporter
        .export_portable_tar_zst()
        .await
        .map_err(ApiError::from)?;
    let mut resp = (StatusCode::OK, bytes).into_response();
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zstd"));
    resp.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"rotameter-backup.tar.zst\""),
    );
    Ok(resp)
}
