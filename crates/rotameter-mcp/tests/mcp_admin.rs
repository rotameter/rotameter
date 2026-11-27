#![forbid(unsafe_code)]
//! Integration tests for opt-in `--allow-admin` MCP tools in `rotameter-mcp`.

use std::io::Cursor;

use rotameter_client::ClientError;
use rotameter_client::types::{
    CreateKeyRequest, CreatedKeyResponse, DeleteSiteResponse, KeyId, KeyMetadataResponse, KeyScope,
    OrgId, QueryRequest, QueryResponse, RealtimeResponse, RevokeKeyResponse, SaltRotation, SiteId,
    SiteRef, SiteResponse, UpsertSiteRequest,
};
use rotameter_mcp::{AnalyticsBackend, McpOptions, serve_stdio_with_options};
use serde_json::{Value, json};

struct AdminMockBackend;

fn sample_site(slug: &str) -> SiteResponse {
    SiteResponse {
        site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
        org_id: OrgId::from_uuidv7_parts(1_700_000_000_000, [2; 10]),
        slug: slug.to_string(),
        display_name: slug.to_string(),
        timezone: "UTC".into(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at: "2026-11-01T00:00:00Z".into(),
        updated_at: "2026-11-01T00:00:00Z".into(),
    }
}

impl AnalyticsBackend for AdminMockBackend {
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError> {
        Ok(vec![sample_site("example.com")])
    }
    fn query(&self, _req: &QueryRequest) -> Result<QueryResponse, ClientError> {
        Err(ClientError::InvalidRequest("unused".into()))
    }
    fn realtime(&self, _site: &SiteRef) -> Result<RealtimeResponse, ClientError> {
        Err(ClientError::InvalidRequest("unused".into()))
    }
    fn default_site(&self) -> Option<&str> {
        Some("example.com")
    }
    fn upsert_site(
        &self,
        site: &SiteRef,
        req: &UpsertSiteRequest,
    ) -> Result<SiteResponse, ClientError> {
        let mut s = sample_site(&site.to_string());
        if let Some(name) = &req.display_name {
            s.display_name.clone_from(name);
        }
        Ok(s)
    }
    fn delete_site(
        &self,
        _site: &SiteRef,
        confirm: bool,
    ) -> Result<DeleteSiteResponse, ClientError> {
        assert!(confirm);
        Ok(DeleteSiteResponse {
            deleted: true,
            site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
        })
    }
    fn create_key(&self, req: &CreateKeyRequest) -> Result<CreatedKeyResponse, ClientError> {
        Ok(CreatedKeyResponse {
            key_id: KeyId::from_uuidv7_parts(1_700_000_000_000, [3; 10]),
            org_id: OrgId::from_uuidv7_parts(1_700_000_000_000, [2; 10]),
            scope: req.scope,
            site_id: Some(SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10])),
            key_prefix: "rm_rd_012345".into(),
            label: req.label.clone(),
            token: "rm_rd_plaintext_token".into(),
            created_at: "2026-11-01T00:00:00Z".into(),
        })
    }
    fn list_keys(&self) -> Result<Vec<KeyMetadataResponse>, ClientError> {
        Ok(vec![KeyMetadataResponse {
            key_id: KeyId::from_uuidv7_parts(1_700_000_000_000, [3; 10]),
            org_id: OrgId::from_uuidv7_parts(1_700_000_000_000, [2; 10]),
            scope: KeyScope::Read,
            site_id: Some(SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10])),
            key_prefix: "rm_rd_012345".into(),
            label: "ci-reader".into(),
            created_at: "2026-11-01T00:00:00Z".into(),
            last_used_at: None,
            revoked_at: None,
        }])
    }
    fn revoke_key(&self, key_id: KeyId) -> Result<RevokeKeyResponse, ClientError> {
        Ok(RevokeKeyResponse {
            revoked: true,
            key_id,
        })
    }
}

fn roundtrip(lines: &[Value], allow_admin: bool) -> Vec<Value> {
    let mut input = String::new();
    for v in lines {
        input.push_str(&v.to_string());
        input.push('\n');
    }
    let mut out = Vec::new();
    serve_stdio_with_options(
        Cursor::new(input.as_bytes()),
        &mut out,
        &AdminMockBackend,
        McpOptions { allow_admin },
    )
    .ok();
    String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

#[test]
fn admin_tools_are_hidden_unless_allow_admin_is_enabled() {
    let reqs = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "rotameter_key_list", "arguments": {} } }),
    ];
    let replies = roundtrip(&reqs, false);
    assert_eq!(replies[0]["result"]["tools"].as_array().unwrap().len(), 7);
    assert_eq!(replies[1]["result"]["isError"], true);
}

#[test]
fn allow_admin_exposes_and_executes_guarded_admin_tools() {
    let key_id = KeyId::from_uuidv7_parts(1_700_000_000_000, [3; 10]).to_string();
    let reqs = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "rotameter_site_upsert", "arguments": { "site": "example.com", "display_name": "Example" } } }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "rotameter_site_delete", "arguments": { "site": "example.com", "confirm": false } } }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "rotameter_site_delete", "arguments": { "site": "example.com", "confirm": true } } }),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "rotameter_key_create", "arguments": { "scope": "read", "site_id": "example.com", "label": "ci" } } }),
        json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "rotameter_key_list", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "rotameter_key_revoke", "arguments": { "key_id": key_id } } }),
    ];
    let replies = roundtrip(&reqs, true);
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 12);
    assert_eq!(
        replies[1]["result"]["structuredContent"]["display_name"],
        "Example"
    );
    assert_eq!(replies[2]["result"]["isError"], true);
    assert_eq!(replies[3]["result"]["structuredContent"]["deleted"], true);
    assert_eq!(
        replies[4]["result"]["structuredContent"]["token"],
        "rm_rd_plaintext_token"
    );
    assert_eq!(
        replies[5]["result"]["structuredContent"]["keys"][0]["label"],
        "ci-reader"
    );
    assert_eq!(replies[6]["result"]["structuredContent"]["revoked"], true);
}
