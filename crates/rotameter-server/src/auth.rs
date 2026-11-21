#![forbid(unsafe_code)]
//! Scoped `SHA-256` Bearer API key verification and CSPRNG token minting.

use std::fmt::Write;

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use rotameter_core::error::CoreError;
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::compute_api_key_hash;
use rotameter_core::types::{KeyScope, Site};

use crate::error::ApiError;
use crate::state::{AppState, fill_random};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthContext {
    pub(crate) org_id: OrgId,
    pub(crate) scope: KeyScope,
    pub(crate) site_id: Option<SiteId>,
}

pub(crate) fn extract_optional_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<AuthContext>, ApiError> {
    let Some(raw_header) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let header_str = raw_header
        .to_str()
        .map_err(|_| ApiError::unauthorized("malformed Authorization header"))?;
    let token = header_str
        .strip_prefix("Bearer ")
        .or_else(|| header_str.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| ApiError::unauthorized("expected `Authorization: Bearer <token>`"))?;

    let hash = compute_api_key_hash(token);
    let record = state
        .store
        .find_active_key_by_hash(&hash)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::unauthorized("invalid or revoked API key"))?;

    Ok(Some(AuthContext {
        org_id: record.org_id,
        scope: record.scope,
        site_id: record.site_id,
    }))
}

pub(crate) fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthContext, ApiError> {
    let auth = extract_optional_auth(state, headers)?
        .ok_or_else(|| ApiError::unauthorized("missing Bearer API key"))?;
    if auth.scope != KeyScope::Admin {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "this endpoint requires an `rm_adm_` admin key",
        ));
    }
    Ok(auth)
}

pub(crate) fn require_read_site(
    state: &AppState,
    headers: &HeaderMap,
    site_ref: &SiteRef,
) -> Result<(AuthContext, Site), ApiError> {
    let auth = extract_optional_auth(state, headers)?
        .ok_or_else(|| ApiError::unauthorized("missing Bearer API key"))?;
    if !matches!(auth.scope, KeyScope::Read | KeyScope::Admin) {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "this endpoint requires a `read` or `admin` API key",
        ));
    }
    let site = state
        .store
        .resolve_site(auth.org_id, site_ref)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;
    if auth.scope == KeyScope::Read && auth.site_id != Some(site.site_id) {
        return Err(ApiError::forbidden(
            "site_scope_mismatch",
            "read key is not scoped to the requested site",
        ));
    }
    Ok((auth, site))
}

pub(crate) fn mint_api_token(scope: KeyScope) -> Result<(String, String, [u8; 32]), CoreError> {
    let raw = fill_random::<32>()?;
    let mut token = String::with_capacity(scope.token_prefix().len() + 64);
    token.push_str(scope.token_prefix());
    for byte in raw {
        let _ = write!(&mut token, "{byte:02x}");
    }
    let prefix: String = token.chars().take(12).collect();
    let hash = compute_api_key_hash(&token);
    Ok((token, prefix, hash))
}
