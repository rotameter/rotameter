#![forbid(unsafe_code)]
//! Shared parameter parsing, site-scope resolution, and delta helpers for `/mcp` tools.

use rotameter_core::id::SiteRef;
use rotameter_core::query::{Granularity, Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_core::storage::KpiSummary;
use rotameter_core::types::{KeyScope, Site};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::auth::AuthContext;
use crate::handlers::query_wire::{WireQueryRequest, WireTimeRange};
use crate::state::AppState;

pub(crate) fn all_metrics() -> Vec<Metric> {
    vec![
        Metric::Visitors,
        Metric::Pageviews,
        Metric::Visits,
        Metric::ViewsPerVisit,
        Metric::BounceRate,
        Metric::AvgVisitDurationMs,
    ]
}

pub(crate) fn build_spec(
    site_id: rotameter_core::id::SiteId,
    range: TimeRange,
    metrics: Vec<Metric>,
) -> QuerySpec {
    QuerySpec {
        site_id: SiteRef::Id(site_id),
        range,
        granularity: None,
        metrics,
        group_by: Vec::new(),
        filters: Vec::new(),
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::Json,
    }
}

pub(crate) async fn resolve_mcp_site(
    state: &AppState,
    auth: AuthContext,
    raw: Option<&Value>,
) -> Result<Site, String> {
    let site_ref = match raw
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(text) => SiteRef::parse(text).map_err(|e| e.to_string())?,
        None if auth.scope == KeyScope::Read => {
            let bound = auth
                .site_id
                .ok_or_else(|| "missing required `site_id`".to_string())?;
            SiteRef::Id(bound)
        }
        None => return Err("missing required `site_id`".to_string()),
    };
    let site = state
        .control_plane
        .resolve_site(auth.org_id, &site_ref)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "unknown site identifier".to_string())?;
    if auth.scope == KeyScope::Read && auth.site_id != Some(site.site_id) {
        return Err("read key is not scoped to the requested site".to_string());
    }
    Ok(site)
}

pub(crate) fn parse_opt<T: DeserializeOwned>(
    val: Option<&Value>,
    field: &str,
) -> Result<Option<T>, String> {
    match val {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|err| format!("invalid `{field}`: {err}")),
    }
}

pub(crate) fn parse_range(
    raw: Option<&Value>,
    default: RangePreset,
) -> Result<WireTimeRange, String> {
    let Some(val) = raw else {
        return Ok(WireTimeRange::Preset { preset: default });
    };
    if let Some(s) = val.as_str() {
        let preset = match s.trim().to_ascii_lowercase().as_str() {
            "24h" => RangePreset::Hours24,
            "7d" => RangePreset::Days7,
            "30d" => RangePreset::Days30,
            "12m" => RangePreset::Months12,
            "all" => RangePreset::All,
            other => return Err(format!("invalid range preset `{other}`")),
        };
        return Ok(WireTimeRange::Preset { preset });
    }
    serde_json::from_value(val.clone()).map_err(|err| format!("invalid `range`: {err}"))
}

pub(crate) fn wire_to_core_range(range: WireTimeRange) -> Result<TimeRange, String> {
    WireQueryRequest::range_to_core(range).map_err(|err| err.to_string())
}

pub(crate) const fn default_granularity(range: &TimeRange) -> Granularity {
    match range {
        TimeRange::Preset {
            preset: RangePreset::Hours24,
        } => Granularity::Hour,
        TimeRange::Preset {
            preset: RangePreset::Months12 | RangePreset::All,
        } => Granularity::Month,
        _ => Granularity::Day,
    }
}

pub(crate) fn parse_limit(val: Option<&Value>, default: u32) -> Result<u32, String> {
    match val {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|&n| (1..=1_000).contains(&n))
            .ok_or_else(|| "`limit` must be an integer between 1 and 1000".to_string()),
    }
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn kpi_deltas(cur: &KpiSummary, prev: &KpiSummary) -> Value {
    let pct = |c: f64, p: f64| {
        if p.abs() <= f64::EPSILON {
            None
        } else {
            Some(((c - p) / p) * 100.0)
        }
    };
    json!({
        "visitors_pct": pct(cur.visitors as f64, prev.visitors as f64),
        "pageviews_pct": pct(cur.pageviews as f64, prev.pageviews as f64),
        "visits_pct": pct(cur.visits as f64, prev.visits as f64),
        "views_per_visit_pct": pct(cur.views_per_visit, prev.views_per_visit),
        "bounce_rate_pct": pct(cur.bounce_rate, prev.bounce_rate),
        "avg_visit_duration_ms_pct": pct(cur.avg_visit_duration_ms as f64, prev.avg_visit_duration_ms as f64),
    })
}
