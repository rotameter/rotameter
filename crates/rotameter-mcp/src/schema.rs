#![forbid(unsafe_code)]
//! JSON Schema definitions for the 7 read-only `v1.0` Rotameter MCP tools.

use serde_json::{Value, json};

fn readonly_annotations() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false
    })
}

fn range_schema() -> Value {
    json!({
        "description": "Preset string (24h, 7d, 30d, 12m, all) or WireTimeRange object",
        "oneOf": [
            { "type": "string", "enum": ["24h", "7d", "30d", "12m", "all"] },
            { "type": "object", "properties": { "preset": { "type": "string" } }, "required": ["preset"] },
            { "type": "object", "properties": { "from": { "type": "string" }, "to": { "type": "string" } }, "required": ["from", "to"] },
            { "type": "object", "properties": { "from_ms": { "type": "integer" }, "to_ms": { "type": "integer" } }, "required": ["from_ms", "to_ms"] }
        ]
    })
}

fn tool(name: &str, description: &str, input_schema: &Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
        "annotations": readonly_annotations()
    })
}

/// Returns MCP `tools/list` descriptors for all 7 strictly read-only analytics tools.
pub(crate) fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "rotameter_sites",
            "List tracked sites, timezones, and salt rotation settings (falls back to configured default site for read-only keys).",
            &json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        tool(
            "rotameter_summary",
            "Fetch headline KPIs (visitors, pageviews, visits, views_per_visit, bounce_rate, avg_visit_duration_ms) and optional previous-period % deltas.",
            &json!({
                "type": "object",
                "properties": {
                    "site_id": { "type": "string", "description": "Target site UID or slug (defaults to profile site)." },
                    "range": range_schema(),
                    "filters": { "type": "array", "items": { "type": "object" } },
                    "compare_previous": { "type": "boolean", "default": true }
                }
            }),
        ),
        tool(
            "rotameter_timeseries",
            "Fetch timestamped visitor, pageview, and visit buckets for a site.",
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
        tool(
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
        tool(
            "rotameter_realtime",
            "Fetch trailing 5-minute active visitors, active pages, and 30-minute pulse strip.",
            &json!({
                "type": "object",
                "properties": { "site_id": { "type": "string" } }
            }),
        ),
        tool(
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
        tool(
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
