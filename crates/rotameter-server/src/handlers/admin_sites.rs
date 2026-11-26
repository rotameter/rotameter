#![forbid(unsafe_code)]
//! Idempotent `/v1/admin/sites/*` control-plane handlers (`rm_adm_` only).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use rotameter_core::id::{SiteRef, validate_slug};
use rotameter_core::types::Site;

use crate::auth::require_admin;
use crate::error::ApiError;
use crate::handlers::admin_wire::{
    DeleteSiteQuery, SiteResponse, UpsertSiteRequest, merge_site_for_put, validate_timezone,
};
use crate::state::{AppState, new_site_id, now_ms};

pub(crate) async fn create_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UpsertSiteRequest>,
) -> Result<(StatusCode, Json<SiteResponse>), ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let raw_slug = req
        .slug
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("missing_slug", "`slug` is required"))?;
    let slug = validate_slug(raw_slug).map_err(ApiError::from)?;
    if state
        .control_plane
        .resolve_site(auth.org_id, &SiteRef::Slug(slug.clone()))
        .await
        .map_err(ApiError::from)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "site_exists",
            "site slug already exists",
        ));
    }
    let tz = validate_timezone(req.timezone.as_deref().unwrap_or("UTC"))?;
    let now = now_ms();
    let site = Site {
        site_id: new_site_id().map_err(ApiError::from)?,
        org_id: auth.org_id,
        slug: slug.clone(),
        display_name: req.display_name.unwrap_or(slug),
        timezone: tz,
        salt_rotation: req.salt_rotation.unwrap_or_default(),
        respect_gpc: req.respect_gpc.unwrap_or(false),
        is_self_site: false,
        allowed_origins: req.allowed_origins.unwrap_or_default(),
        allowed_query_keys: req.allowed_query_keys.unwrap_or_default(),
        raw_retention_days: req.raw_retention_days,
        created_at_ms: now,
        updated_at_ms: now,
    };
    let saved = state
        .control_plane
        .upsert_site(&site)
        .await
        .map_err(ApiError::from)?;
    Ok((StatusCode::CREATED, Json(SiteResponse::from(saved))))
}

pub(crate) async fn put_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(raw_site): Path<String>,
    Json(req): Json<UpsertSiteRequest>,
) -> Result<(StatusCode, Json<SiteResponse>), ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let site_ref = SiteRef::parse(&raw_site).map_err(ApiError::from)?;
    let existing = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(ApiError::from)?;
    let (site, status) =
        merge_site_for_put(auth.org_id, &site_ref, existing.as_ref(), req, now_ms())?;
    let saved = state
        .control_plane
        .upsert_site(&site)
        .await
        .map_err(ApiError::from)?;
    Ok((status, Json(SiteResponse::from(saved))))
}

pub(crate) async fn list_sites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SiteResponse>>, ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let sites = state
        .control_plane
        .list_sites(auth.org_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(sites.into_iter().map(SiteResponse::from).collect()))
}

pub(crate) async fn get_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(raw_site): Path<String>,
) -> Result<Json<SiteResponse>, ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let site_ref = SiteRef::parse(&raw_site).map_err(ApiError::from)?;
    let site = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;
    Ok(Json(SiteResponse::from(site)))
}

pub(crate) async fn delete_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(raw_site): Path<String>,
    Query(query): Query<DeleteSiteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth = require_admin(&state, &headers).await?;
    if !query.confirm {
        return Err(ApiError::bad_request(
            "confirm_required",
            "deleting a site requires `?confirm=true`",
        ));
    }
    let site_ref = SiteRef::parse(&raw_site).map_err(ApiError::from)?;
    let site = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;
    let deleted = state
        .control_plane
        .delete_site(site.site_id)
        .await
        .map_err(ApiError::from)?;
    if deleted {
        state
            .analytics
            .purge_before(site.site_id, i64::MAX)
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(
        serde_json::json!({ "deleted": deleted, "site_id": site.site_id }),
    ))
}
