#![forbid(unsafe_code)]
//! Scoped `SHA-256` Bearer API key verification, operator cookie/CF Access auth, and token minting.

use std::fmt::Write;

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use rotameter_core::error::CoreError;
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::compute_api_key_hash;
use rotameter_core::types::{KeyScope, Site};
use serde_json::Value;

use crate::auth_codec::{decode_jwt_payload, extract_cookie};
use crate::auth_session::SESSION_COOKIE_NAME;
use crate::error::ApiError;
use crate::state::{AppState, fill_random, now_ms};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthContext {
    pub(crate) org_id: OrgId,
    pub(crate) scope: KeyScope,
    pub(crate) site_id: Option<SiteId>,
}

#[derive(Debug, Clone)]
pub(crate) struct OperatorIdentity {
    pub(crate) auth: AuthContext,
    pub(crate) method: &'static str,
    pub(crate) subject: String,
}

/// Strictly Bearer-only extractor used by cookieless collection & MCP routes.
pub(crate) async fn extract_optional_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<AuthContext>, ApiError> {
    Ok(extract_bearer_record(state, headers)
        .await?
        .map(|(ctx, _)| ctx))
}

async fn extract_bearer_record(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<(AuthContext, String)>, ApiError> {
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
        .control_plane
        .find_active_key_by_hash(&hash)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::unauthorized("invalid or revoked API key"))?;

    Ok(Some((
        AuthContext {
            org_id: record.org_id,
            scope: record.scope,
            site_id: record.site_id,
        },
        record.key_prefix,
    )))
}

/// Resolves operator credentials via Bearer token, signed `rotameter_session` cookie (with CSRF
/// verification on mutating methods), or trusted Cloudflare Access headers.
pub(crate) async fn extract_operator_auth(
    state: &AppState,
    headers: &HeaderMap,
    require_csrf: bool,
) -> Result<Option<OperatorIdentity>, ApiError> {
    if let Some((auth, prefix)) = extract_bearer_record(state, headers).await? {
        return Ok(Some(OperatorIdentity {
            auth,
            method: "bearer",
            subject: prefix,
        }));
    }
    if let Some(raw_cookie) = extract_cookie(headers, SESSION_COOKIE_NAME) {
        let claims = state
            .operator_auth
            .verify_session_cookie(&raw_cookie, now_ms())
            .ok_or_else(|| ApiError::unauthorized("invalid or expired operator session"))?;
        if require_csrf {
            state.operator_auth.verify_csrf(headers, &claims)?;
        }
        return Ok(Some(OperatorIdentity {
            auth: AuthContext {
                org_id: claims.org_id,
                scope: claims.scope,
                site_id: claims.site_id,
            },
            method: if claims.method == "oidc" {
                "oidc"
            } else {
                "session"
            },
            subject: claims.sub,
        }));
    }
    if state.operator_auth.cf_access_trusted
        && let Some(subject) = extract_cf_access_subject(state, headers)?
    {
        return Ok(Some(OperatorIdentity {
            auth: AuthContext {
                org_id: state.default_org_id,
                scope: KeyScope::Admin,
                site_id: None,
            },
            method: "cf_access",
            subject,
        }));
    }
    Ok(None)
}

fn extract_cf_access_subject(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<String>, ApiError> {
    let email = headers
        .get("cf-access-authenticated-user-email")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let jwt = headers
        .get("cf-access-jwt-assertion")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if let Some(token) = jwt {
        let claims = decode_jwt_payload(token)?;
        if let Some(expected_aud) = state.operator_auth.cf_access_aud.as_deref() {
            let aud_ok = match claims.get("aud") {
                Some(Value::String(s)) => s == expected_aud,
                Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(expected_aud)),
                _ => false,
            };
            if !aud_ok {
                return Err(ApiError::unauthorized("CF Access JWT audience mismatch"));
            }
        }
        if let Some(expected_iss) = state.operator_auth.cf_access_team_domain.as_deref() {
            let iss = claims
                .get("iss")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if iss.trim_end_matches('/') != expected_iss.trim_end_matches('/') {
                return Err(ApiError::unauthorized("CF Access JWT issuer mismatch"));
            }
        }
        let claim_str = |k: &str| {
            claims
                .get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let jwt_sub = claim_str("email")
            .or_else(|| claim_str("sub"))
            .map(ToString::to_string);
        return Ok(email.or(jwt_sub));
    }
    if state.operator_auth.cf_access_aud.is_some()
        || state.operator_auth.cf_access_team_domain.is_some()
    {
        return Ok(None);
    }
    Ok(email)
}

pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    is_mutating: bool,
) -> Result<AuthContext, ApiError> {
    let ident = extract_operator_auth(state, headers, is_mutating)
        .await?
        .ok_or_else(|| ApiError::unauthorized("missing operator authentication"))?;
    if ident.auth.scope != KeyScope::Admin {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "this endpoint requires an `rm_adm_` admin key or admin session",
        ));
    }
    Ok(ident.auth)
}

pub(crate) async fn require_read_site(
    state: &AppState,
    headers: &HeaderMap,
    site_ref: &SiteRef,
    is_mutating: bool,
) -> Result<(AuthContext, Site), ApiError> {
    let ident = extract_operator_auth(state, headers, is_mutating)
        .await?
        .ok_or_else(|| ApiError::unauthorized("missing operator authentication"))?;
    if !matches!(ident.auth.scope, KeyScope::Read | KeyScope::Admin) {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "this endpoint requires a `read` or `admin` credential",
        ));
    }
    let site = state
        .control_plane
        .resolve_site(ident.auth.org_id, site_ref)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;
    if ident.auth.scope == KeyScope::Read && ident.auth.site_id != Some(site.site_id) {
        return Err(ApiError::forbidden(
            "site_scope_mismatch",
            "read key is not scoped to the requested site",
        ));
    }
    Ok((ident.auth, site))
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
