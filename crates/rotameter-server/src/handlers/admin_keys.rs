#![forbid(unsafe_code)]
//! Idempotent `/v1/admin/keys/*` API key minting, listing, and revocation (`rm_adm_` only).

use std::str::FromStr;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::types::{ApiKeyRecord, KeyScope};
use serde::{Deserialize, Serialize};

use crate::auth::{mint_api_token, require_admin};
use crate::error::ApiError;
use crate::state::{AppState, format_rfc3339, new_key_id, now_ms};

#[derive(Deserialize)]
pub(crate) struct CreateKeyRequest {
    scope: KeyScope,
    #[serde(alias = "site_id")]
    site: Option<SiteRef>,
    label: String,
}

#[derive(Serialize)]
pub(crate) struct CreatedKeyResponse {
    key_id: KeyId,
    org_id: OrgId,
    scope: KeyScope,
    site_id: Option<SiteId>,
    key_prefix: String,
    label: String,
    token: String,
    created_at: String,
}

#[derive(Serialize)]
pub(crate) struct KeyMetadataResponse {
    key_id: KeyId,
    org_id: OrgId,
    scope: KeyScope,
    site_id: Option<SiteId>,
    key_prefix: String,
    label: String,
    created_at: String,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
}

impl From<ApiKeyRecord> for KeyMetadataResponse {
    fn from(k: ApiKeyRecord) -> Self {
        Self {
            key_id: k.key_id,
            org_id: k.org_id,
            scope: k.scope,
            site_id: k.site_id,
            key_prefix: k.key_prefix,
            label: k.label,
            created_at: format_rfc3339(k.created_at_ms),
            last_used_at: k.last_used_ms.map(format_rfc3339),
            revoked_at: k.revoked_at_ms.map(format_rfc3339),
        }
    }
}

pub(crate) async fn create_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateKeyRequest>,
) -> Result<(StatusCode, Json<CreatedKeyResponse>), ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let label = req.label.trim().to_string();
    if label.is_empty() || label.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_label",
            "key `label` must be 1..=128 characters",
        ));
    }

    let site_id = match (req.scope, req.site) {
        (KeyScope::Ingest | KeyScope::Read, Some(site_ref)) => {
            let site = state
                .control_plane
                .resolve_site(auth.org_id, &site_ref)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;
            Some(site.site_id)
        }
        (KeyScope::Ingest | KeyScope::Read, None) => {
            return Err(ApiError::bad_request(
                "missing_site_id",
                "`ingest` and `read` keys must be bound to a `site`",
            ));
        }
        (KeyScope::Admin, Some(_)) => {
            return Err(ApiError::bad_request(
                "unexpected_site_id",
                "`admin` keys are org-scoped and cannot bind a single `site`",
            ));
        }
        (KeyScope::Admin, None) => None,
    };

    let (token, key_prefix, key_hash) = mint_api_token(req.scope).map_err(ApiError::from)?;
    let key_id = new_key_id().map_err(ApiError::from)?;
    let created_at_ms = now_ms();
    state
        .control_plane
        .create_api_key(&ApiKeyRecord {
            key_id,
            org_id: auth.org_id,
            key_prefix: key_prefix.clone(),
            key_hash,
            scope: req.scope,
            site_id,
            label: label.clone(),
            created_at_ms,
            last_used_ms: None,
            revoked_at_ms: None,
        })
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedKeyResponse {
            key_id,
            org_id: auth.org_id,
            scope: req.scope,
            site_id,
            key_prefix,
            label,
            token,
            created_at: format_rfc3339(created_at_ms),
        }),
    ))
}

pub(crate) async fn list_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<KeyMetadataResponse>>, ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let keys = state
        .control_plane
        .list_api_keys(auth.org_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        keys.into_iter().map(KeyMetadataResponse::from).collect(),
    ))
}

pub(crate) async fn revoke_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(raw_key_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auth = require_admin(&state, &headers).await?;
    let key_id = KeyId::from_str(&raw_key_id).map_err(ApiError::from)?;
    let owned = state
        .control_plane
        .list_api_keys(auth.org_id)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .any(|k| k.key_id == key_id);
    if !owned {
        return Err(ApiError::not_found("key_not_found", "unknown key_id"));
    }
    let revoked = state
        .control_plane
        .revoke_api_key(key_id, now_ms())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        serde_json::json!({ "revoked": revoked, "key_id": key_id }),
    ))
}
