#![forbid(unsafe_code)]
//! Guarded admin tool schemas and dispatchers for `rotameter-mcp --allow-admin`.

use rotameter_client::types::{
    CreateKeyRequest, KeyId, KeyScope, SaltRotation, SiteRef, UpsertSiteRequest,
};
use serde_json::{Value, json};

use crate::AnalyticsBackend;
use crate::parse::parse_opt;

fn tool_with_hints(
    name: &str,
    description: &str,
    input_schema: &Value,
    read_only: bool,
    destructive: bool,
    idempotent: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": idempotent,
            "openWorldHint": false
        }
    })
}

/// Returns MCP `tools/list` descriptors for the 5 opt-in admin tools.
pub(crate) fn admin_tool_definitions() -> Vec<Value> {
    vec![
        tool_with_hints(
            "rotameter_site_upsert",
            "Idempotently create or update a tracked site configuration (`rm_adm_` required).",
            &json!({
                "type": "object",
                "required": ["site"],
                "properties": {
                    "site": { "type": "string", "description": "Target site UID or slug." },
                    "slug": { "type": "string" },
                    "display_name": { "type": "string" },
                    "timezone": { "type": "string" },
                    "salt_rotation": { "type": "string", "enum": ["daily", "weekly", "monthly"] },
                    "respect_gpc": { "type": "boolean" },
                    "allowed_origins": { "type": "array", "items": { "type": "string" } },
                    "allowed_query_keys": { "type": "array", "items": { "type": "string" } },
                    "raw_retention_days": { "type": "integer", "minimum": 1 }
                }
            }),
            false,
            false,
            true,
        ),
        tool_with_hints(
            "rotameter_site_delete",
            "Permanently delete a tracked site and cascade its data; requires `confirm: true` (`rm_adm_` required).",
            &json!({
                "type": "object",
                "required": ["site", "confirm"],
                "properties": {
                    "site": { "type": "string", "description": "Target site UID or slug." },
                    "confirm": { "type": "boolean", "description": "Must be true to confirm cascade deletion." }
                }
            }),
            false,
            true,
            true,
        ),
        tool_with_hints(
            "rotameter_key_create",
            "Mint a scoped API key (`ingest`, `read`, or `admin`); plaintext token is returned once (`rm_adm_` required).",
            &json!({
                "type": "object",
                "required": ["scope", "label"],
                "properties": {
                    "scope": { "type": "string", "enum": ["ingest", "read", "admin"] },
                    "site_id": { "type": "string", "description": "Required for `ingest`/`read` scopes; omitted for `admin`." },
                    "label": { "type": "string", "minLength": 1, "maxLength": 128 }
                }
            }),
            false,
            false,
            false,
        ),
        tool_with_hints(
            "rotameter_key_list",
            "List non-sensitive API key metadata for the authenticated organization (`rm_adm_` required).",
            &json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            true,
            false,
            true,
        ),
        tool_with_hints(
            "rotameter_key_revoke",
            "Revoke an API key by `key_id` (`rm_adm_` required).",
            &json!({
                "type": "object",
                "required": ["key_id"],
                "properties": {
                    "key_id": { "type": "string", "description": "Key UID (`key_...`)." }
                }
            }),
            false,
            true,
            true,
        ),
    ]
}

pub(crate) fn call_admin_tool(
    backend: &dyn AnalyticsBackend,
    name: &str,
    args: &Value,
) -> Option<Result<Value, String>> {
    match name {
        "rotameter_site_upsert" => Some(run_site_upsert(backend, args)),
        "rotameter_site_delete" => Some(run_site_delete(backend, args)),
        "rotameter_key_create" => Some(run_key_create(backend, args)),
        "rotameter_key_list" => Some(run_key_list(backend)),
        "rotameter_key_revoke" => Some(run_key_revoke(backend, args)),
        _ => None,
    }
}

fn parse_required_site(args: &Value) -> Result<SiteRef, String> {
    let raw = args
        .get("site")
        .or_else(|| args.get("site_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing required `site`".to_string())?;
    raw.parse::<SiteRef>().map_err(|err| err.to_string())
}

fn run_site_upsert(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site = parse_required_site(args)?;
    let req = UpsertSiteRequest {
        slug: parse_opt(args.get("slug"), "slug")?,
        display_name: parse_opt(args.get("display_name"), "display_name")?,
        timezone: parse_opt(args.get("timezone"), "timezone")?,
        salt_rotation: parse_opt::<SaltRotation>(args.get("salt_rotation"), "salt_rotation")?,
        respect_gpc: parse_opt(args.get("respect_gpc"), "respect_gpc")?,
        allowed_origins: parse_opt(args.get("allowed_origins"), "allowed_origins")?,
        allowed_query_keys: parse_opt(args.get("allowed_query_keys"), "allowed_query_keys")?,
        raw_retention_days: parse_opt(args.get("raw_retention_days"), "raw_retention_days")?,
    };
    let saved = backend
        .upsert_site(&site, &req)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(saved).map_err(|e| e.to_string())
}

fn run_site_delete(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site = parse_required_site(args)?;
    let confirm = args
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !confirm {
        return Err("`rotameter_site_delete` requires `confirm: true`".to_string());
    }
    let res = backend
        .delete_site(&site, true)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(res).map_err(|e| e.to_string())
}

fn run_key_create(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let scope: KeyScope = parse_opt(args.get("scope"), "scope")?
        .ok_or_else(|| "missing required `scope`".to_string())?;
    let label: String = parse_opt(args.get("label"), "label")?
        .ok_or_else(|| "missing required `label`".to_string())?;
    let site_val = args.get("site_id").or_else(|| args.get("site"));
    let site = parse_opt::<SiteRef>(site_val, "site_id")?;
    let created = backend
        .create_key(&CreateKeyRequest { scope, site, label })
        .map_err(|e| e.to_string())?;
    serde_json::to_value(created).map_err(|e| e.to_string())
}

fn run_key_list(backend: &dyn AnalyticsBackend) -> Result<Value, String> {
    let keys = backend.list_keys().map_err(|e| e.to_string())?;
    Ok(json!({ "keys": keys }))
}

fn run_key_revoke(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let key_id: KeyId = parse_opt(args.get("key_id"), "key_id")?
        .ok_or_else(|| "missing required `key_id`".to_string())?;
    let res = backend.revoke_key(key_id).map_err(|e| e.to_string())?;
    serde_json::to_value(res).map_err(|e| e.to_string())
}
