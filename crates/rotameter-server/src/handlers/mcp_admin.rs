#![forbid(unsafe_code)]
//! Guarded admin tool dispatchers for the Streamable HTTP `/mcp` endpoint.

use std::str::FromStr;

use rotameter_core::id::{KeyId, SiteRef};
use rotameter_core::types::{ApiKeyRecord, KeyScope};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{AuthContext, mint_api_token};
use crate::handlers::admin_keys::KeyMetadataResponse;
use crate::handlers::admin_wire::{SiteResponse, UpsertSiteRequest, merge_site_for_put};
use crate::state::{AppState, format_rfc3339, new_key_id, now_ms};

pub(crate) async fn call_admin_tool(
    state: &AppState,
    auth: AuthContext,
    name: &str,
    args: &Value,
) -> Option<Result<Value, String>> {
    match name {
        "rotameter_site_upsert" => Some(run_site_upsert(state, auth, args).await),
        "rotameter_site_delete" => Some(run_site_delete(state, auth, args).await),
        "rotameter_key_create" => Some(run_key_create(state, auth, args).await),
        "rotameter_key_list" => Some(run_key_list(state, auth).await),
        "rotameter_key_revoke" => Some(run_key_revoke(state, auth, args).await),
        _ => None,
    }
}

fn parse_site_arg(args: &Value) -> Result<SiteRef, String> {
    let raw = args
        .get("site")
        .or_else(|| args.get("site_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing required `site`".to_string())?;
    SiteRef::parse(raw).map_err(|e| e.to_string())
}

async fn run_site_upsert(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let site_ref = parse_site_arg(args)?;
    let req: UpsertSiteRequest =
        serde_json::from_value(args.clone()).map_err(|e| format!("invalid site payload: {e}"))?;
    let existing = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(|e| e.to_string())?;
    let (site, _) = merge_site_for_put(auth.org_id, &site_ref, existing.as_ref(), req, now_ms())
        .map_err(|e| e.to_string())?;
    let saved = state
        .control_plane
        .upsert_site(&site)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(SiteResponse::from(saved)).map_err(|e| e.to_string())
}

async fn run_site_delete(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let site_ref = parse_site_arg(args)?;
    let confirm = args
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !confirm {
        return Err("`rotameter_site_delete` requires `confirm: true`".to_string());
    }
    let site = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "unknown site identifier".to_string())?;
    let deleted = state
        .control_plane
        .delete_site(site.site_id)
        .await
        .map_err(|e| e.to_string())?;
    if deleted {
        state
            .analytics
            .purge_before(site.site_id, i64::MAX)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(json!({ "deleted": deleted, "site_id": site.site_id }))
}

#[derive(Deserialize)]
struct McpCreateKeyArgs {
    scope: KeyScope,
    #[serde(alias = "site_id")]
    site: Option<SiteRef>,
    label: String,
}

async fn run_key_create(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let req: McpCreateKeyArgs =
        serde_json::from_value(args.clone()).map_err(|e| format!("invalid key args: {e}"))?;
    let label = req.label.trim().to_string();
    if label.is_empty() || label.len() > 128 {
        return Err("key `label` must be 1..=128 characters".to_string());
    }
    let site_id = match (req.scope, req.site) {
        (KeyScope::Ingest | KeyScope::Read, Some(site_ref)) => {
            let site = state
                .control_plane
                .resolve_site(auth.org_id, &site_ref)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "unknown site identifier".to_string())?;
            Some(site.site_id)
        }
        (KeyScope::Ingest | KeyScope::Read, None) => {
            return Err("`ingest` and `read` keys must be bound to a `site_id`".to_string());
        }
        (KeyScope::Admin, Some(_)) => {
            return Err("`admin` keys are org-scoped and cannot bind a `site_id`".to_string());
        }
        (KeyScope::Admin, None) => None,
    };

    let (token, key_prefix, key_hash) = mint_api_token(req.scope).map_err(|e| e.to_string())?;
    let key_id = new_key_id().map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "key_id": key_id,
        "org_id": auth.org_id,
        "scope": req.scope,
        "site_id": site_id,
        "key_prefix": key_prefix,
        "label": label,
        "token": token,
        "created_at": format_rfc3339(created_at_ms),
    }))
}

async fn run_key_list(state: &AppState, auth: AuthContext) -> Result<Value, String> {
    let keys = state
        .control_plane
        .list_api_keys(auth.org_id)
        .await
        .map_err(|e| e.to_string())?;
    let out: Vec<KeyMetadataResponse> = keys.into_iter().map(KeyMetadataResponse::from).collect();
    Ok(json!({ "keys": out }))
}

async fn run_key_revoke(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let raw = args
        .get("key_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing required `key_id`".to_string())?;
    let key_id = KeyId::from_str(raw).map_err(|e| e.to_string())?;
    let owned = state
        .control_plane
        .list_api_keys(auth.org_id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|k| k.key_id == key_id);
    if !owned {
        return Err("unknown key_id".to_string());
    }
    let revoked = state
        .control_plane
        .revoke_api_key(key_id, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({ "revoked": revoked, "key_id": key_id }))
}
