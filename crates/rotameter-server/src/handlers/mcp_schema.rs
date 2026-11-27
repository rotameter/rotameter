#![forbid(unsafe_code)]
//! JSON Schema descriptors for the `/mcp` HTTP endpoint.

use serde_json::{Value, json};

fn tool(
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

fn ro_tool(name: &str, description: &str, input_schema: &Value) -> Value {
    tool(name, description, input_schema, true, false, true)
}

fn range_schema() -> Value {
    json!({
        "description": "Preset string (24h, 7d, 30d, 12m, all) or time range object",
        "oneOf": [
            { "type": "string", "enum": ["24h", "7d", "30d", "12m", "all"] },
            { "type": "object", "properties": { "preset": { "type": "string" } }, "required": ["preset"] },
            { "type": "object", "properties": { "from": { "type": "string" }, "to": { "type": "string" } }, "required": ["from", "to"] },
            { "type": "object", "properties": { "from_ms": { "type": "integer" }, "to_ms": { "type": "integer" } }, "required": ["from_ms", "to_ms"] }
        ]
    })
}

pub(crate) fn list_tools(allow_admin: bool) -> Vec<Value> {
    let mut list = read_tools();
    if allow_admin {
        list.extend(admin_tools());
    }
    list
}

fn read_tools() -> Vec<Value> {
    vec![
        ro_tool(
            "rotameter_sites",
            "List tracked sites, timezones, and salt rotation settings.",
            &json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        ro_tool(
            "rotameter_summary",
            "Fetch headline KPIs and optional previous-period % deltas.",
            &json!({
                "type": "object",
                "properties": {
                    "site_id": { "type": "string" },
                    "range": range_schema(),
                    "filters": { "type": "array", "items": { "type": "object" } },
                    "compare_previous": { "type": "boolean", "default": true }
                }
            }),
        ),
        ro_tool(
            "rotameter_timeseries",
            "Fetch timestamped visitor, pageview, and visit buckets.",
            &json!({
                "type": "object",
                "properties": {
                    "site_id": { "type": "string" },
                    "range": range_schema(),
                    "granularity": { "type": "string", "enum": ["hour", "day", "month"] },
                    "metrics": { "type": "array", "items": { "type": "string" } },
                    "filters": { "type": "array", "items": { "type": "object" } }
                }
            }),
        ),
        ro_tool(
            "rotameter_breakdown",
            "Fetch ranked dimension rows (path, source, channel, country, browser, os, device, event_name, utm_*).",
            &json!({
                "type": "object",
                "required": ["dimension"],
                "properties": {
                    "site_id": { "type": "string" },
                    "dimension": { "type": "string" },
                    "range": range_schema(),
                    "filters": { "type": "array", "items": { "type": "object" } },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 20 }
                }
            }),
        ),
        ro_tool(
            "rotameter_realtime",
            "Fetch trailing 5-minute active visitors, active pages, and 30-minute pulse strip.",
            &json!({ "type": "object", "properties": { "site_id": { "type": "string" } } }),
        ),
        ro_tool(
            "rotameter_compare_periods",
            "Compare headline KPIs and optional dimension breakdown across two explicit time windows.",
            &json!({
                "type": "object",
                "required": ["current", "previous"],
                "properties": {
                    "site_id": { "type": "string" },
                    "current": { "type": "object" },
                    "previous": { "type": "object" },
                    "dimension": { "type": "string" },
                    "filters": { "type": "array", "items": { "type": "object" } },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 20 }
                }
            }),
        ),
        ro_tool(
            "rotameter_query",
            "Execute a composable POST /v1/query analytics payload directly.",
            &json!({
                "type": "object",
                "required": ["site_id", "range", "metrics"],
                "properties": {
                    "site_id": { "type": "string" },
                    "range": { "type": "object" },
                    "granularity": { "type": "string" },
                    "metrics": { "type": "array", "items": { "type": "string" } },
                    "group_by": { "type": "array", "items": { "type": "string" } },
                    "filters": { "type": "array", "items": { "type": "object" } },
                    "compare_previous": { "type": "boolean" },
                    "sort": { "type": "object" },
                    "limit": { "type": "integer" },
                    "offset": { "type": "integer" }
                }
            }),
        ),
    ]
}

fn admin_tools() -> Vec<Value> {
    vec![
        tool(
            "rotameter_site_upsert",
            "Idempotently create or update a tracked site configuration (`rm_adm_` required).",
            &json!({
                "type": "object",
                "required": ["site"],
                "properties": {
                    "site": { "type": "string" },
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
        tool(
            "rotameter_site_delete",
            "Permanently delete a tracked site and cascade its data; requires `confirm: true`.",
            &json!({
                "type": "object",
                "required": ["site", "confirm"],
                "properties": { "site": { "type": "string" }, "confirm": { "type": "boolean" } }
            }),
            false,
            true,
            true,
        ),
        tool(
            "rotameter_key_create",
            "Mint a scoped API key (`ingest`, `read`, or `admin`).",
            &json!({
                "type": "object",
                "required": ["scope", "label"],
                "properties": {
                    "scope": { "type": "string", "enum": ["ingest", "read", "admin"] },
                    "site_id": { "type": "string" },
                    "label": { "type": "string", "minLength": 1, "maxLength": 128 }
                }
            }),
            false,
            false,
            false,
        ),
        ro_tool(
            "rotameter_key_list",
            "List non-sensitive API key metadata for the authenticated organization.",
            &json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "rotameter_key_revoke",
            "Revoke an API key by `key_id`.",
            &json!({
                "type": "object",
                "required": ["key_id"],
                "properties": { "key_id": { "type": "string" } }
            }),
            false,
            true,
            true,
        ),
    ]
}
