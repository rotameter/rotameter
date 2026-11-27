#![forbid(unsafe_code)]
//! Newline-delimited JSON-RPC 2.0 protocol router for MCP over `stdio`.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::admin_tools::admin_tool_definitions;
use crate::schema::tool_definitions;
use crate::tools::call_tool;
use crate::{AnalyticsBackend, McpOptions};

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn ok_response(id: &Value, result: &Value) -> Option<String> {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .ok()
}

fn err_response(id: &Value, code: i64, message: &str) -> Option<String> {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
    .ok()
}

/// Processes one JSON-RPC line; returns `None` for blank lines or notifications.
pub(crate) fn handle_line(
    line: &str,
    backend: &dyn AnalyticsBackend,
    options: McpOptions,
) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let req: RpcRequest = match serde_json::from_str(trimmed) {
        Ok(parsed) => parsed,
        Err(err) => return err_response(&Value::Null, -32700, &format!("parse error: {err}")),
    };

    if let Some(ver) = req.jsonrpc.as_deref()
        && ver != "2.0"
    {
        return req
            .id
            .as_ref()
            .and_then(|id| err_response(id, -32600, "invalid jsonrpc version"));
    }

    let id = req.id?;

    match req.method.as_str() {
        "initialize" => ok_response(
            &id,
            &json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "rotameter-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        ),
        "ping" => ok_response(&id, &json!({})),
        "tools/list" => {
            let mut tools = tool_definitions();
            if options.allow_admin {
                tools.extend(admin_tool_definitions());
            }
            ok_response(&id, &json!({ "tools": tools }))
        }
        "tools/call" => handle_tool_call(&id, &req.params, backend, options),
        "resources/list" => ok_response(&id, &json!({ "resources": [] })),
        "prompts/list" => ok_response(&id, &json!({ "prompts": [] })),
        other => err_response(&id, -32601, &format!("method `{other}` not found")),
    }
}

fn handle_tool_call(
    id: &Value,
    params: &Value,
    backend: &dyn AnalyticsBackend,
    options: McpOptions,
) -> Option<String> {
    let parsed: ToolCallParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(err) => return err_response(id, -32602, &format!("invalid params: {err}")),
    };
    let args = if parsed.arguments.is_null() {
        json!({})
    } else {
        parsed.arguments
    };
    match call_tool(backend, &parsed.name, &args, options) {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            ok_response(
                id,
                &json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": value,
                    "isError": false
                }),
            )
        }
        Err(msg) => ok_response(
            id,
            &json!({
                "content": [{ "type": "text", "text": msg }],
                "isError": true
            }),
        ),
    }
}
