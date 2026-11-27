#![forbid(unsafe_code)]
//! Integration tests for Streamable HTTP `/mcp` transport and `--allow-admin` gating.

mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use rotameter_server::{
    AppState, McpHttpConfig, ServerConfig, ServerRole, ServerStores, build_router,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::{Value, json};

use crate::common::{TestStore, as_json, request};

const ADMIN_KEY: &str = "rm_adm_0123456789abcdef0123456789abcdef";

async fn mcp_app(enabled: bool, allow_admin: bool) -> axum::Router {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(
        ServerStores::unified(Arc::clone(&store)),
        &ServerConfig {
            role: ServerRole::All,
            bootstrap_admin_key: Some(ADMIN_KEY.to_string()),
            mcp_http: McpHttpConfig {
                enabled,
                allow_admin,
            },
            ..ServerConfig::default()
        },
    )
    .await
    else {
        std::process::abort()
    };
    build_router(state)
}

async fn mcp_call(app: &axum::Router, token: &str, body: Value) -> (StatusCode, Value) {
    let (st, bytes) = request(app, Method::POST, "/mcp", Some(token), &[], Some(&body)).await;
    (st, as_json(&bytes))
}

#[tokio::test]
async fn mcp_route_is_404_when_disabled_and_enforces_auth_when_enabled() {
    let disabled = mcp_app(false, false).await;
    let (st, _) = request(
        &disabled,
        Method::POST,
        "/mcp",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let app = mcp_app(true, false).await;
    let (get_st, _) = request(&app, Method::GET, "/mcp", Some(ADMIN_KEY), &[], None).await;
    assert_eq!(get_st, StatusCode::METHOD_NOT_ALLOWED);

    let (unauth_st, _) = request(
        &app,
        Method::POST,
        "/mcp",
        None,
        &[],
        Some(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })),
    )
    .await;
    assert_eq!(unauth_st, StatusCode::UNAUTHORIZED);

    let (site_st, site_body) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "slug": "example.com" })),
    )
    .await;
    assert_eq!(site_st, StatusCode::CREATED);
    let site_id = as_json(&site_body)["site_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let (ing_st, ing_body) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "scope": "ingest", "site": site_id, "label": "ing" })),
    )
    .await;
    assert_eq!(ing_st, StatusCode::CREATED);
    let ing_tok = as_json(&ing_body)["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let (forbid_st, _) = mcp_call(
        &app,
        &ing_tok,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
    )
    .await;
    assert_eq!(forbid_st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mcp_http_handles_notifications_and_initialize() {
    let app = mcp_app(true, false).await;
    let (notif_st, notif_body) = request(
        &app,
        Method::POST,
        "/mcp",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })),
    )
    .await;
    assert_eq!(notif_st, StatusCode::ACCEPTED);
    assert_eq!(notif_body, [] as [u8; 0]);

    let (init_st, init_json) = mcp_call(
        &app,
        ADMIN_KEY,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    )
    .await;
    assert_eq!(init_st, StatusCode::OK);
    assert_eq!(
        init_json["result"]["serverInfo"]["name"],
        "rotameter-server"
    );

    let (_, list_json) = mcp_call(
        &app,
        ADMIN_KEY,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    assert_eq!(
        list_json["result"]["tools"].as_array().map(Vec::len),
        Some(7)
    );
}

#[tokio::test]
async fn mcp_http_supports_read_scope_and_opt_in_admin_tools() {
    let app = mcp_app(true, true).await;
    let (_, adm_list) = mcp_call(
        &app,
        ADMIN_KEY,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    assert_eq!(
        adm_list["result"]["tools"].as_array().map(Vec::len),
        Some(12)
    );

    let (_, upsert) = mcp_call(
        &app,
        ADMIN_KEY,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "rotameter_site_upsert", "arguments": { "site": "demo.dev", "display_name": "Demo" } }
        }),
    )
    .await;
    assert_eq!(upsert["result"]["structuredContent"]["slug"], "demo.dev");

    let (_, key_res) = mcp_call(
        &app,
        ADMIN_KEY,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "rotameter_key_create", "arguments": { "scope": "read", "site_id": "demo.dev", "label": "mcp-ro" } }
        }),
    )
    .await;
    let rd_tok = key_res["result"]["structuredContent"]["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(rd_tok.starts_with("rm_rd_"));

    let (_, rd_list) = mcp_call(
        &app,
        &rd_tok,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }),
    )
    .await;
    assert_eq!(rd_list["result"]["tools"].as_array().map(Vec::len), Some(7));

    let (_, rd_summary) = mcp_call(
        &app,
        &rd_tok,
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "rotameter_summary", "arguments": { "range": "30d" } }
        }),
    )
    .await;
    assert_eq!(rd_summary["result"]["isError"], false);

    let (_, rd_adm) = mcp_call(
        &app,
        &rd_tok,
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "rotameter_key_list", "arguments": {} }
        }),
    )
    .await;
    assert_eq!(rd_adm["result"]["isError"], true);
}
