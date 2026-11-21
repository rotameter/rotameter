#![forbid(unsafe_code)]
//! Wire DTOs, builder helpers, and validation for `/v1/admin/sites/*`.

use axum::http::StatusCode;
use chrono_tz::Tz;
use rotameter_core::id::{OrgId, SiteId, SiteRef, validate_slug};
use rotameter_core::types::{SaltRotation, Site};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::{format_rfc3339, new_site_id};

#[derive(Deserialize)]
pub(crate) struct UpsertSiteRequest {
    pub(crate) slug: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) salt_rotation: Option<SaltRotation>,
    pub(crate) respect_gpc: Option<bool>,
    pub(crate) allowed_origins: Option<Vec<String>>,
    pub(crate) allowed_query_keys: Option<Vec<String>>,
    pub(crate) raw_retention_days: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct DeleteSiteQuery {
    #[serde(default)]
    pub(crate) confirm: bool,
}

#[derive(Serialize)]
pub(crate) struct SiteResponse {
    pub(crate) site_id: SiteId,
    pub(crate) org_id: OrgId,
    pub(crate) slug: String,
    pub(crate) display_name: String,
    pub(crate) timezone: String,
    pub(crate) salt_rotation: SaltRotation,
    pub(crate) respect_gpc: bool,
    pub(crate) is_self_site: bool,
    pub(crate) allowed_origins: Vec<String>,
    pub(crate) allowed_query_keys: Vec<String>,
    pub(crate) raw_retention_days: Option<u32>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

impl From<Site> for SiteResponse {
    fn from(s: Site) -> Self {
        Self {
            site_id: s.site_id,
            org_id: s.org_id,
            slug: s.slug,
            display_name: s.display_name,
            timezone: s.timezone,
            salt_rotation: s.salt_rotation,
            respect_gpc: s.respect_gpc,
            is_self_site: s.is_self_site,
            allowed_origins: s.allowed_origins,
            allowed_query_keys: s.allowed_query_keys,
            raw_retention_days: s.raw_retention_days,
            created_at: format_rfc3339(s.created_at_ms),
            updated_at: format_rfc3339(s.updated_at_ms),
        }
    }
}

pub(crate) fn validate_timezone(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    trimmed
        .parse::<Tz>()
        .map(|tz| tz.name().to_string())
        .map_err(|_| {
            ApiError::bad_request("invalid_timezone", format!("unknown timezone `{trimmed}`"))
        })
}

pub(crate) fn merge_site_for_put(
    org_id: OrgId,
    site_ref: &SiteRef,
    existing: Option<&Site>,
    req: UpsertSiteRequest,
    now: i64,
) -> Result<(Site, StatusCode), ApiError> {
    let (site_id, slug, created_at_ms, status, is_self_site) = if let Some(prev) = existing {
        let slug = match req.slug.as_deref() {
            Some(s) => validate_slug(s).map_err(ApiError::from)?,
            None => prev.slug.clone(),
        };
        (
            prev.site_id,
            slug,
            prev.created_at_ms,
            StatusCode::OK,
            prev.is_self_site,
        )
    } else {
        let slug = match (site_ref, req.slug.as_deref()) {
            (_, Some(s)) => validate_slug(s).map_err(ApiError::from)?,
            (SiteRef::Slug(s), None) => s.clone(),
            (SiteRef::Id(_), None) => {
                return Err(ApiError::bad_request(
                    "missing_slug",
                    "creating a new site requires `slug`",
                ));
            }
        };
        let id = match site_ref {
            SiteRef::Id(id) => *id,
            SiteRef::Slug(_) => new_site_id().map_err(ApiError::from)?,
        };
        (id, slug, now, StatusCode::CREATED, false)
    };

    let tz_raw = req
        .timezone
        .as_deref()
        .or_else(|| existing.map(|p| p.timezone.as_str()))
        .unwrap_or("UTC");
    let site = Site {
        site_id,
        org_id,
        slug: slug.clone(),
        display_name: req
            .display_name
            .or_else(|| existing.map(|p| p.display_name.clone()))
            .unwrap_or(slug),
        timezone: validate_timezone(tz_raw)?,
        salt_rotation: req
            .salt_rotation
            .or_else(|| existing.map(|p| p.salt_rotation))
            .unwrap_or_default(),
        respect_gpc: req
            .respect_gpc
            .or_else(|| existing.map(|p| p.respect_gpc))
            .unwrap_or(false),
        is_self_site,
        allowed_origins: req
            .allowed_origins
            .or_else(|| existing.map(|p| p.allowed_origins.clone()))
            .unwrap_or_default(),
        allowed_query_keys: req
            .allowed_query_keys
            .or_else(|| existing.map(|p| p.allowed_query_keys.clone()))
            .unwrap_or_default(),
        raw_retention_days: req
            .raw_retention_days
            .or_else(|| existing.and_then(|p| p.raw_retention_days)),
        created_at_ms,
        updated_at_ms: now,
    };
    Ok((site, status))
}
