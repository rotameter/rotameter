#![forbid(unsafe_code)]
//! Parameter parsing and percentage-delta calculation helpers for MCP tools.

use rotameter_client::types::{
    Dimension, Granularity, KpiSummary, Metric, QueryFilter, RangePreset, SiteRef, WireTimeRange,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

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

pub(crate) fn parse_site_ref(
    raw: Option<&Value>,
    default_site: Option<&str>,
) -> Result<SiteRef, String> {
    let text = raw
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| default_site.map(str::trim).filter(|s| !s.is_empty()))
        .ok_or_else(|| {
            "missing required `site_id` (and no profile default site set)".to_string()
        })?;
    text.parse::<SiteRef>().map_err(|err| err.to_string())
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

pub(crate) fn parse_filters(val: Option<&Value>) -> Result<Vec<QueryFilter>, String> {
    match parse_opt::<Vec<QueryFilter>>(val, "filters")? {
        Some(list) => Ok(list),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn parse_dimension(val: Option<&Value>) -> Result<Dimension, String> {
    let raw = val.ok_or_else(|| "missing required `dimension`".to_string())?;
    serde_json::from_value(raw.clone()).map_err(|err| format!("invalid `dimension`: {err}"))
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

pub(crate) const fn default_granularity(range: &WireTimeRange) -> Granularity {
    match range {
        WireTimeRange::Preset {
            preset: RangePreset::Hours24,
        } => Granularity::Hour,
        WireTimeRange::Preset {
            preset: RangePreset::Months12 | RangePreset::All,
        } => Granularity::Month,
        _ => Granularity::Day,
    }
}

#[allow(clippy::cast_precision_loss)]
fn u64_f64(v: u64) -> f64 {
    v as f64
}

fn pct_delta(cur: f64, prev: f64) -> Option<f64> {
    if prev.abs() <= f64::EPSILON {
        None
    } else {
        Some(((cur - prev) / prev) * 100.0)
    }
}

pub(crate) fn kpi_deltas(cur: &KpiSummary, prev: &KpiSummary) -> Value {
    json!({
        "visitors_pct": pct_delta(u64_f64(cur.visitors), u64_f64(prev.visitors)),
        "pageviews_pct": pct_delta(u64_f64(cur.pageviews), u64_f64(prev.pageviews)),
        "visits_pct": pct_delta(u64_f64(cur.visits), u64_f64(prev.visits)),
        "views_per_visit_pct": pct_delta(cur.views_per_visit, prev.views_per_visit),
        "bounce_rate_pct": pct_delta(cur.bounce_rate, prev.bounce_rate),
        "avg_visit_duration_ms_pct": pct_delta(u64_f64(cur.avg_visit_duration_ms), u64_f64(prev.avg_visit_duration_ms)),
    })
}
