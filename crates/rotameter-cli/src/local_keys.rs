#![forbid(unsafe_code)]
//! Direct `SQLite` control-plane operations for `rotameter key`.

use std::fmt::Write;
use std::str::FromStr;

use rotameter_client::types::{
    CreateKeyRequest, CreatedKeyResponse, KeyId, KeyMetadataResponse, KeyScope, RevokeKeyResponse,
};
use rotameter_core::privacy::compute_api_key_hash;
use rotameter_core::types::ApiKeyRecord;

use crate::cli::GlobalOpts;
use crate::context::open_local_store;
use crate::exit::CliError;
use crate::format::format_rfc3339;

pub(crate) fn key_create(
    global: &GlobalOpts,
    req: &CreateKeyRequest,
) -> Result<CreatedKeyResponse, CliError> {
    let (store, now, org_id) = open_local_store(global)?;
    let label = req.label.trim().to_string();
    if label.is_empty() || label.len() > 128 {
        return Err(CliError::usage("`--label` must be 1..=128 characters"));
    }
    let site_id = match (req.scope, req.site.as_ref()) {
        (KeyScope::Ingest | KeyScope::Read, Some(s_ref)) => Some(
            store
                .resolve_site_sync(org_id, s_ref)?
                .ok_or_else(|| CliError::runtime(format!("unknown site `{s_ref}`")))?
                .site_id,
        ),
        (KeyScope::Ingest | KeyScope::Read, None) => {
            return Err(CliError::usage("`--site` is required for ingest/read keys"));
        }
        (KeyScope::Admin, Some(_)) => {
            return Err(CliError::usage(
                "`admin` keys cannot bind a single `--site`",
            ));
        }
        (KeyScope::Admin, None) => None,
    };
    let raw = random_bytes::<32>()?;
    let mut token = String::with_capacity(req.scope.token_prefix().len() + 64);
    token.push_str(req.scope.token_prefix());
    for byte in raw {
        let _ = write!(&mut token, "{byte:02x}");
    }
    let key_prefix: String = token.chars().take(12).collect();
    let key_id = KeyId::from_uuidv7_parts(
        u64::try_from(now.max(0)).unwrap_or(0),
        random_bytes::<10>()?,
    );
    store.create_api_key_sync(&ApiKeyRecord {
        key_id,
        org_id,
        key_prefix: key_prefix.clone(),
        key_hash: compute_api_key_hash(&token),
        scope: req.scope,
        site_id,
        label: label.clone(),
        created_at_ms: now,
        last_used_ms: None,
        revoked_at_ms: None,
    })?;
    Ok(CreatedKeyResponse {
        key_id,
        org_id,
        scope: req.scope,
        site_id,
        key_prefix,
        label,
        token,
        created_at: format_rfc3339(now),
    })
}

pub(crate) fn key_list(global: &GlobalOpts) -> Result<Vec<KeyMetadataResponse>, CliError> {
    let (store, _, org_id) = open_local_store(global)?;
    Ok(store
        .list_api_keys_sync(org_id)?
        .into_iter()
        .map(|k| KeyMetadataResponse {
            key_id: k.key_id,
            org_id: k.org_id,
            scope: k.scope,
            site_id: k.site_id,
            key_prefix: k.key_prefix,
            label: k.label,
            created_at: format_rfc3339(k.created_at_ms),
            last_used_at: k.last_used_ms.map(format_rfc3339),
            revoked_at: k.revoked_at_ms.map(format_rfc3339),
        })
        .collect())
}

pub(crate) fn key_revoke(global: &GlobalOpts, raw_id: &str) -> Result<RevokeKeyResponse, CliError> {
    let key_id = KeyId::from_str(raw_id)?;
    let (store, now, org_id) = open_local_store(global)?;
    let owned = store
        .list_api_keys_sync(org_id)?
        .into_iter()
        .any(|k| k.key_id == key_id);
    if !owned {
        return Err(CliError::runtime(format!("unknown key `{key_id}`")));
    }
    let revoked = store.revoke_api_key_sync(key_id, now)?;
    Ok(RevokeKeyResponse { revoked, key_id })
}

pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], CliError> {
    let mut buf = [0_u8; N];
    getrandom::fill(&mut buf).map_err(|err| CliError::runtime(format!("CSPRNG failure: {err}")))?;
    Ok(buf)
}
