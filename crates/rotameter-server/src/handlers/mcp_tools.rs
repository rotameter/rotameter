#![forbid(unsafe_code)]
//! Read-only analytics tool dispatchers for the Streamable HTTP `/mcp` endpoint.

use rotameter_core::id::SiteRef;
use rotameter_core::query::{Dimension, Granularity, Metric, QueryFilter, QuerySpec, RangePreset};
use rotameter_core::types::KeyScope;
use serde_json::{Value, json};

use crate::auth::AuthContext;
use crate::handlers::admin_wire::SiteResponse;
use crate::handlers::mcp_parse::{
    all_metrics, build_spec, default_granularity, kpi_deltas, parse_limit, parse_opt, parse_range,
    resolve_mcp_site, wire_to_core_range,
};
use crate::handlers::query_wire::{
    WireQueryRequest, WireQueryResult, WireRealtimeSnapshot, WireTimeRange,
};
use crate::state::{AppState, now_ms};

pub(crate) async fn call_read_tool(
    state: &AppState,
    auth: AuthContext,
    name: &str,
    args: &Value,
) -> Option<Result<Value, String>> {
    match name {
        "rotameter_sites" => Some(run_sites(state, auth).await),
        "rotameter_summary" => Some(run_summary(state, auth, args).await),
        "rotameter_timeseries" => Some(run_timeseries(state, auth, args).await),
        "rotameter_breakdown" => Some(run_breakdown(state, auth, args).await),
        "rotameter_realtime" => Some(run_realtime(state, auth, args).await),
        "rotameter_compare_periods" => Some(run_compare_periods(state, auth, args).await),
        "rotameter_query" => Some(run_query(state, auth, args).await),
        _ => None,
    }
}

async fn run_sites(state: &AppState, auth: AuthContext) -> Result<Value, String> {
    if auth.scope == KeyScope::Admin {
        let sites = state
            .control_plane
            .list_sites(auth.org_id)
            .await
            .map_err(|e| e.to_string())?;
        let out: Vec<SiteResponse> = sites.into_iter().map(SiteResponse::from).collect();
        return Ok(json!({ "sites": out, "fallback": false }));
    }
    let mut sites = Vec::new();
    if let Some(site_id) = auth.site_id {
        if let Some(site) = state
            .control_plane
            .resolve_site(auth.org_id, &SiteRef::Id(site_id))
            .await
            .map_err(|e| e.to_string())?
        {
            sites.push(serde_json::to_value(SiteResponse::from(site)).map_err(|e| e.to_string())?);
        } else {
            sites.push(json!({ "site_id": site_id, "source": "api_key.site_id" }));
        }
    }
    Ok(json!({
        "sites": sites,
        "fallback": true,
        "note": "read-only token cannot enumerate /v1/admin/sites; using key-bound site"
    }))
}

async fn run_summary(state: &AppState, auth: AuthContext, args: &Value) -> Result<Value, String> {
    let site = resolve_mcp_site(state, auth, args.get("site_id")).await?;
    let range = wire_to_core_range(parse_range(args.get("range"), RangePreset::Days30)?)?;
    let filters: Vec<QueryFilter> = parse_opt(args.get("filters"), "filters")?.unwrap_or_default();
    let compare = args
        .get("compare_previous")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut spec = build_spec(site.site_id, range, all_metrics());
    spec.filters = filters;
    spec.compare_previous = compare;
    spec.limit = 1;
    spec.validate().map_err(|e| e.to_string())?;
    let res = state
        .analytics
        .query(&site, &spec, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    let deltas = res
        .previous_summary
        .as_ref()
        .map(|prev| kpi_deltas(&res.summary, prev));
    let mut obj = serde_json::to_value(WireQueryResult::from(res)).map_err(|e| e.to_string())?;
    if let Some(map) = obj.as_object_mut() {
        map.insert(
            "deltas".to_string(),
            serde_json::to_value(deltas).unwrap_or(Value::Null),
        );
    }
    Ok(obj)
}

async fn run_timeseries(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let site = resolve_mcp_site(state, auth, args.get("site_id")).await?;
    let range = wire_to_core_range(parse_range(args.get("range"), RangePreset::Days30)?)?;
    let granularity = parse_opt::<Granularity>(args.get("granularity"), "granularity")?
        .unwrap_or_else(|| default_granularity(&range));
    let metrics = parse_opt::<Vec<Metric>>(args.get("metrics"), "metrics")?
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| vec![Metric::Visitors, Metric::Pageviews, Metric::Visits]);
    let mut spec = build_spec(site.site_id, range, metrics);
    spec.granularity = Some(granularity);
    spec.filters = parse_opt(args.get("filters"), "filters")?.unwrap_or_default();
    spec.limit = 1;
    spec.validate().map_err(|e| e.to_string())?;
    let res = state
        .analytics
        .query(&site, &spec, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    let val = serde_json::to_value(WireQueryResult::from(res)).map_err(|e| e.to_string())?;
    Ok(json!({
        "site_id": val["site_id"], "slug": val["slug"], "window": val["window"],
        "granularity": granularity, "timeseries": val["timeseries"], "meta": val["meta"],
    }))
}

async fn run_breakdown(state: &AppState, auth: AuthContext, args: &Value) -> Result<Value, String> {
    let site = resolve_mcp_site(state, auth, args.get("site_id")).await?;
    let dim: Dimension = parse_opt(args.get("dimension"), "dimension")?
        .ok_or_else(|| "missing required `dimension`".to_string())?;
    let range = wire_to_core_range(parse_range(args.get("range"), RangePreset::Days30)?)?;
    let limit = parse_limit(args.get("limit"), 20)?;
    let mut spec = build_spec(site.site_id, range, all_metrics());
    spec.group_by = vec![dim];
    spec.filters = parse_opt(args.get("filters"), "filters")?.unwrap_or_default();
    spec.limit = limit;
    spec.validate().map_err(|e| e.to_string())?;
    let res = state
        .analytics
        .query(&site, &spec, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    let val = serde_json::to_value(WireQueryResult::from(res)).map_err(|e| e.to_string())?;
    Ok(json!({
        "site_id": val["site_id"], "slug": val["slug"], "window": val["window"],
        "dimension": dim, "rows": val["rows"], "meta": val["meta"],
    }))
}

async fn run_realtime(state: &AppState, auth: AuthContext, args: &Value) -> Result<Value, String> {
    let site = resolve_mcp_site(state, auth, args.get("site_id")).await?;
    let snap = state
        .analytics
        .realtime(&site, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(WireRealtimeSnapshot::from(snap)).map_err(|e| e.to_string())
}

async fn run_compare_periods(
    state: &AppState,
    auth: AuthContext,
    args: &Value,
) -> Result<Value, String> {
    let site = resolve_mcp_site(state, auth, args.get("site_id")).await?;
    let cur_wire: WireTimeRange = parse_opt(args.get("current"), "current")?
        .ok_or_else(|| "missing required `current` window".to_string())?;
    let prev_wire: WireTimeRange = parse_opt(args.get("previous"), "previous")?
        .ok_or_else(|| "missing required `previous` window".to_string())?;
    let cur_range = wire_to_core_range(cur_wire)?;
    let prev_range = wire_to_core_range(prev_wire)?;
    let dim: Option<Dimension> = parse_opt(args.get("dimension"), "dimension")?;
    let filters: Vec<QueryFilter> = parse_opt(args.get("filters"), "filters")?.unwrap_or_default();
    let limit = parse_limit(args.get("limit"), 20)?;
    let build = |range| -> Result<QuerySpec, String> {
        let mut s = build_spec(site.site_id, range, all_metrics());
        s.group_by = dim.into_iter().collect();
        s.filters.clone_from(&filters);
        s.limit = limit;
        s.validate().map_err(|e| e.to_string())?;
        Ok(s)
    };
    let now = now_ms();
    let cur = state
        .analytics
        .query(&site, &build(cur_range)?, now)
        .await
        .map_err(|e| e.to_string())?;
    let prev = state
        .analytics
        .query(&site, &build(prev_range)?, now)
        .await
        .map_err(|e| e.to_string())?;
    let deltas = kpi_deltas(&cur.summary, &prev.summary);
    let cur_w = serde_json::to_value(WireQueryResult::from(cur)).map_err(|e| e.to_string())?;
    let prev_w = serde_json::to_value(WireQueryResult::from(prev)).map_err(|e| e.to_string())?;
    Ok(json!({
        "site_id": cur_w["site_id"], "slug": cur_w["slug"],
        "current_window": cur_w["window"], "previous_window": prev_w["window"],
        "current": cur_w["summary"], "previous": prev_w["summary"],
        "deltas": deltas, "rows_current": cur_w["rows"], "rows_previous": prev_w["rows"],
    }))
}

async fn run_query(state: &AppState, auth: AuthContext, args: &Value) -> Result<Value, String> {
    let mut payload = args.clone();
    if let Some(obj) = payload.as_object_mut()
        && (!obj.contains_key("site_id") || obj.get("site_id") == Some(&Value::Null))
        && auth.scope == KeyScope::Read
        && let Some(site_id) = auth.site_id
    {
        obj.insert("site_id".to_string(), Value::String(site_id.to_string()));
    }
    let req: WireQueryRequest =
        serde_json::from_value(payload).map_err(|e| format!("invalid query payload: {e}"))?;
    let spec = req.into_core().map_err(|e| e.to_string())?;
    spec.validate().map_err(|e| e.to_string())?;
    let site = state
        .control_plane
        .resolve_site(auth.org_id, &spec.site_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "unknown site identifier".to_string())?;
    if auth.scope == KeyScope::Read && auth.site_id != Some(site.site_id) {
        return Err("read key is not scoped to the requested site".to_string());
    }
    let res = state
        .analytics
        .query(&site, &spec, now_ms())
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(WireQueryResult::from(res)).map_err(|e| e.to_string())
}
