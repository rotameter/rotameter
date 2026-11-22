#![forbid(unsafe_code)]
//! Direct `SQLite` control-plane operations for `rotameter site`.

use rotameter_client::types::{
    DeleteSiteResponse, SiteId, SiteRef, SiteResponse, UpsertSiteRequest,
};
use rotameter_core::id::validate_slug;
use rotameter_core::storage::{AnalyticsQueryStore, ControlPlaneStore};
use rotameter_core::types::Site;

use crate::cli::GlobalOpts;
use crate::context::open_local_store;
use crate::exit::CliError;
use crate::format::format_rfc3339;
use crate::local_keys::random_bytes;

pub(crate) fn site_upsert(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    req: &UpsertSiteRequest,
) -> Result<SiteResponse, CliError> {
    let (store, now, org_id) = open_local_store(global)?;
    let existing = store.resolve_site(org_id, site_ref)?;
    let (site_id, slug, created_at_ms, is_self_site) = if let Some(prev) = &existing {
        let slug = match req.slug.as_deref() {
            Some(s) => validate_slug(s)?,
            None => prev.slug.clone(),
        };
        (prev.site_id, slug, prev.created_at_ms, prev.is_self_site)
    } else {
        let slug = match (site_ref, req.slug.as_deref()) {
            (_, Some(s)) => validate_slug(s)?,
            (SiteRef::Slug(s), None) => s.clone(),
            (SiteRef::Id(_), None) => {
                return Err(CliError::usage(
                    "creating a new site by ID requires `--slug`",
                ));
            }
        };
        let id = match site_ref {
            SiteRef::Id(id) => *id,
            SiteRef::Slug(_) => SiteId::from_uuidv7_parts(
                u64::try_from(now.max(0)).unwrap_or(0),
                random_bytes::<10>()?,
            ),
        };
        (id, slug, now, false)
    };
    let prev = existing.as_ref();
    let site = Site {
        site_id,
        org_id,
        slug: slug.clone(),
        display_name: req
            .display_name
            .clone()
            .or_else(|| prev.map(|p| p.display_name.clone()))
            .unwrap_or(slug),
        timezone: req
            .timezone
            .clone()
            .or_else(|| prev.map(|p| p.timezone.clone()))
            .unwrap_or_else(|| "UTC".to_string()),
        salt_rotation: req
            .salt_rotation
            .or_else(|| prev.map(|p| p.salt_rotation))
            .unwrap_or_default(),
        respect_gpc: req
            .respect_gpc
            .or_else(|| prev.map(|p| p.respect_gpc))
            .unwrap_or(false),
        is_self_site,
        allowed_origins: req
            .allowed_origins
            .clone()
            .or_else(|| prev.map(|p| p.allowed_origins.clone()))
            .unwrap_or_default(),
        allowed_query_keys: req
            .allowed_query_keys
            .clone()
            .or_else(|| prev.map(|p| p.allowed_query_keys.clone()))
            .unwrap_or_default(),
        raw_retention_days: req
            .raw_retention_days
            .or_else(|| prev.and_then(|p| p.raw_retention_days)),
        created_at_ms,
        updated_at_ms: now,
    };
    Ok(site_to_response(store.upsert_site(&site)?))
}

pub(crate) fn site_list(global: &GlobalOpts) -> Result<Vec<SiteResponse>, CliError> {
    let (store, _, org_id) = open_local_store(global)?;
    Ok(store
        .list_sites(org_id)?
        .into_iter()
        .map(site_to_response)
        .collect())
}

pub(crate) fn site_show(global: &GlobalOpts, site_ref: &SiteRef) -> Result<SiteResponse, CliError> {
    let (store, _, org_id) = open_local_store(global)?;
    let site = store
        .resolve_site(org_id, site_ref)?
        .ok_or_else(|| CliError::runtime(format!("unknown site `{site_ref}`")))?;
    Ok(site_to_response(site))
}

pub(crate) fn site_purge(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    before_ms: i64,
) -> Result<(SiteId, u64), CliError> {
    let (store, _, org_id) = open_local_store(global)?;
    let site = store
        .resolve_site(org_id, site_ref)?
        .ok_or_else(|| CliError::runtime(format!("unknown site `{site_ref}`")))?;
    let deleted = store.purge_before(site.site_id, before_ms)?;
    Ok((site.site_id, deleted))
}

pub(crate) fn site_delete(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    confirm: bool,
) -> Result<DeleteSiteResponse, CliError> {
    if !confirm {
        return Err(CliError::usage("deleting a site requires `--confirm`"));
    }
    let (store, _, org_id) = open_local_store(global)?;
    let site = store
        .resolve_site(org_id, site_ref)?
        .ok_or_else(|| CliError::runtime(format!("unknown site `{site_ref}`")))?;
    let deleted = store.delete_site(site.site_id)?;
    Ok(DeleteSiteResponse {
        deleted,
        site_id: site.site_id,
    })
}

fn site_to_response(s: Site) -> SiteResponse {
    SiteResponse {
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
