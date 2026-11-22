#![forbid(unsafe_code)]
//! Idempotent local `self_site` first-boot initialization (`SPEC-API.md` §6.1).

use rotameter_core::id::{OrgId, SiteId, SiteRef, validate_slug};
use rotameter_core::types::{SaltRotation, Site};
use url::Url;

use crate::config::{ServerConfig, ServerError};
use crate::state::{ServerStore, new_site_id};

pub(crate) fn ensure_self_site(
    store: &dyn ServerStore,
    org_id: OrgId,
    config: &ServerConfig,
    now_ms: i64,
) -> Result<Option<SiteId>, ServerError> {
    if !config.self_site_enabled {
        return Ok(None);
    }

    let existing_sites = store
        .list_sites(org_id)
        .map_err(|err| ServerError::new(format!("cannot list sites for self_site check: {err}")))?;
    if let Some(existing) = existing_sites.iter().find(|s| s.is_self_site) {
        return Ok(Some(existing.site_id));
    }

    let (explicit_id, slug) = resolve_identity(config)?;
    if let Some(id) = explicit_id
        && let Some(existing) = store
            .resolve_site(org_id, &SiteRef::Id(id))
            .map_err(|err| ServerError::new(format!("cannot check self_site id: {err}")))?
    {
        return Ok(Some(existing.site_id));
    }
    if let Some(existing) = existing_sites.iter().find(|s| s.slug == slug) {
        return Ok(Some(existing.site_id));
    }

    let site_id = match explicit_id {
        Some(id) => id,
        None => new_site_id()
            .map_err(|err| ServerError::new(format!("cannot mint self_site id: {err}")))?,
    };
    let site = Site {
        site_id,
        org_id,
        slug,
        display_name: "Self (Local)".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: true,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    };
    let saved = store
        .upsert_site(&site)
        .map_err(|err| ServerError::new(format!("cannot seed self_site: {err}")))?;
    tracing::info!(site_id = %saved.site_id, slug = %saved.slug, "seeded local self_site");
    Ok(Some(saved.site_id))
}

fn resolve_identity(config: &ServerConfig) -> Result<(Option<SiteId>, String), ServerError> {
    let default_slug =
        slug_from_public_url(config.public_url.as_deref()).unwrap_or_else(|| "_self".to_string());
    let Some(raw_override) = config
        .self_site_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok((None, default_slug));
    };

    match SiteRef::parse(raw_override) {
        Ok(SiteRef::Id(id)) => Ok((Some(id), default_slug)),
        Ok(SiteRef::Slug(slug)) => Ok((None, slug)),
        Err(err) => Err(ServerError::new(format!(
            "invalid self_site_id `{raw_override}`: {err}"
        ))),
    }
}

fn slug_from_public_url(raw: Option<&str>) -> Option<String> {
    let raw_url = raw.map(str::trim).filter(|s| !s.is_empty())?;
    let parsed = Url::parse(raw_url).ok()?;
    let host = parsed.host_str()?;
    validate_slug(host).ok()
}
