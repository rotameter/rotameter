#![forbid(unsafe_code)]
//! Read-only analytics tool dispatchers backing MCP `tools/call`.

use rotameter_client::types::{Granularity, Metric, QueryRequest, RangePreset, WireTimeRange};
use serde_json::{Value, json};

use crate::admin_tools::call_admin_tool;
use crate::parse::{
    all_metrics, default_granularity, kpi_deltas, parse_dimension, parse_filters, parse_limit,
    parse_opt, parse_range, parse_site_ref,
};
use crate::{AnalyticsBackend, McpOptions};

pub(crate) fn call_tool(
    backend: &dyn AnalyticsBackend,
    name: &str,
    args: &Value,
    options: McpOptions,
) -> Result<Value, String> {
    match name {
        "rotameter_sites" => run_sites(backend),
        "rotameter_summary" => run_summary(backend, args),
        "rotameter_timeseries" => run_timeseries(backend, args),
        "rotameter_breakdown" => run_breakdown(backend, args),
        "rotameter_realtime" => run_realtime(backend, args),
        "rotameter_compare_periods" => run_compare_periods(backend, args),
        "rotameter_query" => run_query(backend, args),
        other => {
            if options.allow_admin
                && let Some(result) = call_admin_tool(backend, other, args)
            {
                return result;
            }
            Err(format!("unknown MCP tool `{other}`"))
        }
    }
}

fn run_sites(backend: &dyn AnalyticsBackend) -> Result<Value, String> {
    match backend.list_sites() {
        Ok(sites) => Ok(json!({ "sites": sites, "fallback": false })),
        Err(err) if err.is_auth_error() => {
            let sites: Vec<Value> = backend.default_site().map_or_else(Vec::new, |slug| {
                vec![json!({ "site_id": slug, "source": "config.default_site" })]
            });
            Ok(json!({
                "sites": sites,
                "fallback": true,
                "note": "read-only token cannot enumerate /v1/admin/sites; using configured default_site"
            }))
        }
        Err(err) => Err(err.to_string()),
    }
}

fn run_summary(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site_id = parse_site_ref(args.get("site_id"), backend.default_site())?;
    let range = parse_range(args.get("range"), RangePreset::Days30)?;
    let filters = parse_filters(args.get("filters"))?;
    let compare = args
        .get("compare_previous")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut req = QueryRequest::new(site_id, range, all_metrics());
    req.filters = filters;
    req.compare_previous = compare;
    req.limit = 1;

    let res = backend.query(&req).map_err(|err| err.to_string())?;
    let deltas = res
        .previous_summary
        .as_ref()
        .map(|prev| kpi_deltas(&res.summary, prev));
    Ok(json!({
        "site_id": res.site_id,
        "slug": res.slug,
        "timezone": res.timezone,
        "window": res.window,
        "summary": res.summary,
        "previous_summary": res.previous_summary,
        "deltas": deltas,
        "meta": res.meta,
    }))
}

fn run_timeseries(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site_id = parse_site_ref(args.get("site_id"), backend.default_site())?;
    let range = parse_range(args.get("range"), RangePreset::Days30)?;
    let granularity = parse_opt::<Granularity>(args.get("granularity"), "granularity")?
        .unwrap_or_else(|| default_granularity(&range));
    let metrics = parse_opt::<Vec<Metric>>(args.get("metrics"), "metrics")?
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| vec![Metric::Visitors, Metric::Pageviews, Metric::Visits]);

    let mut req = QueryRequest::new(site_id, range, metrics);
    req.granularity = Some(granularity);
    req.filters = parse_filters(args.get("filters"))?;
    req.limit = 1;

    let res = backend.query(&req).map_err(|err| err.to_string())?;
    Ok(json!({
        "site_id": res.site_id,
        "slug": res.slug,
        "window": res.window,
        "granularity": granularity,
        "timeseries": res.timeseries,
        "meta": res.meta,
    }))
}

fn run_breakdown(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site_id = parse_site_ref(args.get("site_id"), backend.default_site())?;
    let dim = parse_dimension(args.get("dimension"))?;
    let range = parse_range(args.get("range"), RangePreset::Days30)?;
    let limit = parse_limit(args.get("limit"), 20)?;

    let mut req = QueryRequest::new(site_id, range, all_metrics());
    req.group_by = vec![dim];
    req.filters = parse_filters(args.get("filters"))?;
    req.limit = limit;

    let res = backend.query(&req).map_err(|err| err.to_string())?;
    Ok(json!({
        "site_id": res.site_id,
        "slug": res.slug,
        "window": res.window,
        "dimension": dim,
        "rows": res.rows,
        "meta": res.meta,
    }))
}

fn run_realtime(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site_id = parse_site_ref(args.get("site_id"), backend.default_site())?;
    let rt = backend.realtime(&site_id).map_err(|err| err.to_string())?;
    serde_json::to_value(rt).map_err(|err| err.to_string())
}

fn run_compare_periods(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let site_id = parse_site_ref(args.get("site_id"), backend.default_site())?;
    let cur_range: WireTimeRange = parse_opt(args.get("current"), "current")?
        .ok_or_else(|| "missing required `current` window".to_string())?;
    let prev_range: WireTimeRange = parse_opt(args.get("previous"), "previous")?
        .ok_or_else(|| "missing required `previous` window".to_string())?;
    let dim = match args.get("dimension").filter(|v| !v.is_null()) {
        Some(v) => Some(parse_dimension(Some(v))?),
        None => None,
    };
    let filters = parse_filters(args.get("filters"))?;
    let limit = parse_limit(args.get("limit"), 20)?;

    let build = |range| {
        let mut r = QueryRequest::new(site_id.clone(), range, all_metrics());
        r.group_by = dim.into_iter().collect();
        r.filters.clone_from(&filters);
        r.limit = limit;
        r
    };
    let cur = backend
        .query(&build(cur_range))
        .map_err(|e| e.to_string())?;
    let prev = backend
        .query(&build(prev_range))
        .map_err(|e| e.to_string())?;
    let deltas = kpi_deltas(&cur.summary, &prev.summary);
    Ok(json!({
        "site_id": cur.site_id,
        "slug": cur.slug,
        "current_window": cur.window,
        "previous_window": prev.window,
        "current": cur.summary,
        "previous": prev.summary,
        "deltas": deltas,
        "rows_current": cur.rows,
        "rows_previous": prev.rows,
    }))
}

fn run_query(backend: &dyn AnalyticsBackend, args: &Value) -> Result<Value, String> {
    let mut payload = args.clone();
    if let Some(obj) = payload.as_object_mut()
        && (!obj.contains_key("site_id") || obj.get("site_id") == Some(&Value::Null))
        && let Some(site) = backend.default_site()
    {
        obj.insert("site_id".to_string(), Value::String(site.to_string()));
    }
    let req: QueryRequest =
        serde_json::from_value(payload).map_err(|err| format!("invalid query payload: {err}"))?;
    let res = backend.query(&req).map_err(|err| err.to_string())?;
    serde_json::to_value(res).map_err(|err| err.to_string())
}
