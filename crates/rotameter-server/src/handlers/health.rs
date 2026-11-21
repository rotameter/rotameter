#![forbid(unsafe_code)]
//! Unauthenticated `/v1/health` liveness and queue telemetry endpoint.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    version: &'static str,
    schema_version: u32,
    queue_depth: usize,
    queue_capacity: usize,
    queue_fill_pct: f64,
    uptime_secs: u64,
}

#[allow(clippy::cast_precision_loss)]
pub(crate) async fn get_health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    let schema_version = state.store.schema_version().map_err(ApiError::from)?;
    let queue_depth = state.ingest.queue_depth();
    let queue_capacity = state.ingest.capacity().max(1);
    let queue_fill_pct = (queue_depth as f64 / queue_capacity as f64) * 100.0;

    Ok(Json(HealthResponse {
        status: "ok",
        version: rotameter_core::WORKSPACE_VERSION,
        schema_version,
        queue_depth,
        queue_capacity,
        queue_fill_pct,
        uptime_secs: state.started_at.elapsed().as_secs(),
    }))
}
