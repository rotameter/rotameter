#![forbid(unsafe_code)]
//! Streamable HTTP `/mcp` JSON-RPC 2.0 endpoint gated behind `--enable-mcp-http`.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::ALLOW;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rotameter_core::types::KeyScope;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{AuthContext, extract_optional_auth};
use crate::error::ApiError;
use crate::handlers::mcp_admin::call_admin_tool;
use crate::handlers::mcp_schema::list_tools;
use crate::handlers::mcp_tools::call_read_tool;
use crate::state::AppState;

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

fn rpc_ok(id: &Value, result: &Value) -> Response {
    let mut resp = Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .into_response();
    resp.headers_mut().insert(
        HeaderName::from_static("mcp-protocol-version"),
        HeaderValue::from_static(PROTOCOL_VERSION),
    );
    resp
}

fn rpc_err(id: &Value, code: i64, message: &str) -> Response {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
    .into_response()
}

pub(crate) async fn get_mcp() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(ALLOW, "POST")], "").into_response()
}

pub(crate) async fn delete_mcp() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(ALLOW, "POST")], "").into_response()
}

pub(crate) async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let auth = extract_optional_auth(&state, &headers)
        .await?
        .ok_or_else(|| ApiError::unauthorized("missing Bearer API key"))?;
    if !matches!(auth.scope, KeyScope::Read | KeyScope::Admin) {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "MCP endpoint requires a `read` or `admin` API key",
        ));
    }

    let req: RpcRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(err) => {
            return Ok(rpc_err(
                &Value::Null,
                -32700,
                &format!("parse error: {err}"),
            ));
        }
    };
    if let Some(ver) = req.jsonrpc.as_deref()
        && ver != "2.0"
    {
        return Ok(req.id.map_or_else(
            || StatusCode::ACCEPTED.into_response(),
            |id| rpc_err(&id, -32600, "invalid jsonrpc version"),
        ));
    }
    let Some(id) = req.id else {
        return Ok(StatusCode::ACCEPTED.into_response());
    };

    let allow_admin = state.mcp_allow_admin && auth.scope == KeyScope::Admin;
    let resp = match req.method.as_str() {
        "initialize" => rpc_ok(
            &id,
            &json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "rotameter-server",
                    "version": rotameter_core::WORKSPACE_VERSION,
                }
            }),
        ),
        "ping" => rpc_ok(&id, &json!({})),
        "tools/list" => rpc_ok(&id, &json!({ "tools": list_tools(allow_admin) })),
        "tools/call" => handle_tool_call(&state, auth, &id, &req.params, allow_admin).await,
        "resources/list" => rpc_ok(&id, &json!({ "resources": [] })),
        "prompts/list" => rpc_ok(&id, &json!({ "prompts": [] })),
        other => rpc_err(&id, -32601, &format!("method `{other}` not found")),
    };
    Ok(resp)
}

async fn handle_tool_call(
    state: &AppState,
    auth: AuthContext,
    id: &Value,
    params: &Value,
    allow_admin: bool,
) -> Response {
    let parsed: ToolCallParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(err) => return rpc_err(id, -32602, &format!("invalid params: {err}")),
    };
    let args = if parsed.arguments.is_null() {
        json!({})
    } else {
        parsed.arguments
    };
    let result = if let Some(res) = call_read_tool(state, auth, &parsed.name, &args).await {
        res
    } else if allow_admin && let Some(res) = call_admin_tool(state, auth, &parsed.name, &args).await
    {
        res
    } else {
        Err(format!("unknown MCP tool `{}`", parsed.name))
    };

    match result {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            rpc_ok(
                id,
                &json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": value,
                    "isError": false
                }),
            )
        }
        Err(msg) => rpc_ok(
            id,
            &json!({
                "content": [{ "type": "text", "text": msg }],
                "isError": true
            }),
        ),
    }
}
