#![forbid(unsafe_code)]
//! Authenticated `POST /v1/query` (`JSON` / `CSV`) and `GET /v1/sites/{site}/realtime` handlers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use rotameter_core::id::SiteRef;
use rotameter_core::query::QueryFormat;
use rotameter_core::storage::format_query_result_csv;

use crate::auth::require_read_site;
use crate::error::ApiError;
use crate::handlers::query_wire::{WireQueryRequest, WireQueryResult, WireRealtimeSnapshot};
use crate::state::{AppState, now_ms};

pub(crate) async fn post_query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<WireQueryRequest>,
) -> Result<Response, ApiError> {
    let spec = req.into_core()?;
    spec.validate().map_err(ApiError::from)?;
    let (_, site) = require_read_site(&state, &headers, &spec.site_id, true).await?;
    let result = state
        .analytics
        .query(&site, &spec, now_ms())
        .await
        .map_err(ApiError::from)?;

    if spec.format == QueryFormat::Csv {
        let csv = format_query_result_csv(&result, &spec);
        return Ok((
            StatusCode::OK,
            [(CONTENT_TYPE, "text/csv; charset=utf-8")],
            csv,
        )
            .into_response());
    }
    Ok(Json(WireQueryResult::from(result)).into_response())
}

pub(crate) async fn get_realtime(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(raw_site): Path<String>,
) -> Result<Json<WireRealtimeSnapshot>, ApiError> {
    let site_ref = SiteRef::parse(&raw_site).map_err(ApiError::from)?;
    let (_, site) = require_read_site(&state, &headers, &site_ref, false).await?;
    let snap = state
        .analytics
        .realtime(&site, now_ms())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(WireRealtimeSnapshot::from(snap)))
}
