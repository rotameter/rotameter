#![forbid(unsafe_code)]
//! Integration tests for the `rotameter-mcp` stdio JSON-RPC 2.0 protocol and 7 analytics tools.

use std::collections::BTreeMap;
use std::io::Cursor;

use rotameter_client::ClientError;
use rotameter_client::types::{
    BreakdownRow, KpiSummary, OrgId, QueryMeta, QueryRequest, QueryResponse, QueryWindow,
    RealtimePage, RealtimePulsePoint, RealtimeResponse, SaltRotation, SiteId, SiteRef,
    SiteResponse, TimeseriesRow,
};
use rotameter_mcp::{AnalyticsBackend, serve_stdio};
use serde_json::{Value, json};

struct MockBackend {
    sites_forbidden: bool,
}

impl AnalyticsBackend for MockBackend {
    fn list_sites(&self) -> Result<Vec<SiteResponse>, ClientError> {
        if self.sites_forbidden {
            return Err(ClientError::Api {
                status: 403,
                code: "forbidden".into(),
                message: "read key cannot list sites".into(),
                details: None,
            });
        }
        Ok(vec![SiteResponse {
            site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
            org_id: OrgId::from_uuidv7_parts(1_700_000_000_000, [2; 10]),
            slug: "colinwolff.dev".into(),
            display_name: "Colin".into(),
            timezone: "UTC".into(),
            salt_rotation: SaltRotation::Daily,
            respect_gpc: false,
            is_self_site: false,
            allowed_origins: Vec::new(),
            allowed_query_keys: Vec::new(),
            raw_retention_days: None,
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
        }])
    }

    fn query(&self, req: &QueryRequest) -> Result<QueryResponse, ClientError> {
        let mut key = BTreeMap::new();
        key.insert("path".into(), "/projects/rotameter".into());
        let is_prev = matches!(
            &req.range,
            rotameter_client::types::WireTimeRange::Iso { from, .. }
                if from.starts_with("2026-11-03")
        );
        let visits = if is_prev { 110 } else { 220 };
        Ok(QueryResponse {
            site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
            slug: req.site_id.to_string(),
            timezone: "UTC".into(),
            window: QueryWindow {
                from: "2026-10-24T00:00:00Z".into(),
                to: "2026-11-23T00:00:00Z".into(),
            },
            summary: KpiSummary {
                visitors: 200,
                pageviews: 450,
                visits,
                views_per_visit: 2.04,
                bounce_rate: 0.4,
                avg_visit_duration_ms: 90_000,
            },
            previous_summary: Some(KpiSummary {
                visitors: 100,
                pageviews: 300,
                visits: 110,
                views_per_visit: 2.72,
                bounce_rate: 0.5,
                avg_visit_duration_ms: 60_000,
            }),
            timeseries: vec![TimeseriesRow {
                bucket: "2026-11-22T00:00:00Z".into(),
                visitors: 40,
                pageviews: 90,
                visits: 45,
            }],
            rows: vec![BreakdownRow {
                key,
                visitors: 120,
                pageviews: 280,
                visits: 130,
                bounce_rate: 0.35,
                avg_visit_duration_ms: 100_000,
            }],
            meta: QueryMeta {
                total_rows: 1,
                query_ms: 1.5,
                truncated_by_cardinality_cap: false,
            },
        })
    }

    fn realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, ClientError> {
        Ok(RealtimeResponse {
            site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
            slug: site.to_string(),
            active_visitors: 7,
            active_pages: vec![RealtimePage {
                path: "/".into(),
                active_visitors: 5,
            }],
            pulse: vec![RealtimePulsePoint {
                minute: "2026-11-22T12:00:00Z".into(),
                events: 9,
            }],
        })
    }

    fn default_site(&self) -> Option<&str> {
        Some("colinwolff.dev")
    }
}

fn rpc_roundtrip(lines: &[Value], backend: &dyn AnalyticsBackend) -> Vec<Value> {
    let mut input = String::new();
    for v in lines {
        input.push_str(&v.to_string());
        input.push('\n');
    }
    let mut out = Vec::new();
    serve_stdio(Cursor::new(input.as_bytes()), &mut out, backend).ok();
    String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

#[test]
fn stdio_protocol_lists_seven_readonly_tools_and_executes_all() {
    let backend = MockBackend {
        sites_forbidden: false,
    };
    let reqs = vec![
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "rotameter_sites", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "rotameter_summary", "arguments": { "range": "30d" } } }),
        json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "rotameter_timeseries", "arguments": { "range": "24h" } } }),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "rotameter_breakdown", "arguments": { "dimension": "path", "limit": 10 } } }),
        json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": { "name": "rotameter_realtime", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": "rotameter_compare_periods", "arguments": { "current": { "from": "2026-11-10T00:00:00Z", "to": "2026-11-17T00:00:00Z" }, "previous": { "from": "2026-11-03T00:00:00Z", "to": "2026-11-10T00:00:00Z" } } } }),
        json!({ "jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": { "name": "rotameter_query", "arguments": { "site_id": "colinwolff.dev", "range": { "preset": "7d" }, "metrics": ["visitors", "pageviews"] } } }),
        json!({ "jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": { "name": "unknown_tool", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 12, "method": "unknown/method" }),
    ];

    let replies = rpc_roundtrip(&reqs, &backend);
    assert_eq!(replies.len(), 12);
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "rotameter-mcp");
    let tools = replies[2]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
    for t in tools {
        assert_eq!(t["annotations"]["readOnlyHint"], true);
        assert_eq!(t["annotations"]["destructiveHint"], false);
    }
    let sc = |i: usize| &replies[i]["result"]["structuredContent"];
    assert_eq!(sc(3)["fallback"], false);
    assert_eq!(sc(4)["deltas"]["visitors_pct"], 100.0);
    assert_eq!(sc(5)["granularity"], "hour");
    assert_eq!(sc(6)["dimension"], "path");
    assert_eq!(sc(7)["active_visitors"], 7);
    assert_eq!(sc(8)["deltas"]["visits_pct"], 100.0);
    assert_eq!(sc(9)["summary"]["visitors"], 200);
    assert_eq!(replies[10]["result"]["isError"], true);
    assert_eq!(replies[11]["error"]["code"], -32601);

    let ro = rpc_roundtrip(
        &[
            json!({ "jsonrpc": "2.0", "id": 99, "method": "tools/call", "params": { "name": "rotameter_sites", "arguments": {} } }),
        ],
        &MockBackend {
            sites_forbidden: true,
        },
    );
    assert_eq!(ro[0]["result"]["structuredContent"]["fallback"], true);
    assert_eq!(
        ro[0]["result"]["structuredContent"]["sites"][0]["site_id"],
        "colinwolff.dev"
    );
}
