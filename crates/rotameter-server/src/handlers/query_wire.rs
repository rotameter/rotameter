#![forbid(unsafe_code)]
//! Wire JSON request/response DTOs matching `SPEC-API.md` §5 (`RFC 3339` timestamps).

use chrono::DateTime;
use rotameter_core::id::{SiteId, SiteRef};
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFilter, QueryFormat, QuerySort, QuerySpec, RangePreset,
    TimeRange,
};
use rotameter_core::storage::{
    BreakdownRow, KpiSummary, QueryMeta, QueryResult, RealtimePage, RealtimeSnapshot,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::format_rfc3339;

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum WireTimeRange {
    Preset { preset: RangePreset },
    Iso { from: String, to: String },
    Millis { from_ms: i64, to_ms: i64 },
}

#[derive(Deserialize)]
pub(crate) struct WireQueryRequest {
    #[serde(alias = "site")]
    pub(crate) site_id: SiteRef,
    pub(crate) range: WireTimeRange,
    pub(crate) granularity: Option<Granularity>,
    pub(crate) metrics: Vec<Metric>,
    #[serde(default)]
    pub(crate) group_by: Vec<Dimension>,
    #[serde(default)]
    pub(crate) filters: Vec<QueryFilter>,
    #[serde(default)]
    pub(crate) compare_previous: bool,
    pub(crate) sort: Option<QuerySort>,
    #[serde(default = "default_limit")]
    pub(crate) limit: u32,
    #[serde(default)]
    pub(crate) offset: u32,
    #[serde(default)]
    pub(crate) format: QueryFormat,
}

const fn default_limit() -> u32 {
    50
}

impl WireQueryRequest {
    pub(crate) fn range_to_core(range: WireTimeRange) -> Result<TimeRange, ApiError> {
        match range {
            WireTimeRange::Preset { preset } => Ok(TimeRange::Preset { preset }),
            WireTimeRange::Iso { from, to } => Ok(TimeRange::Explicit {
                from_ms: parse_iso_ms(&from, "from")?,
                to_ms: parse_iso_ms(&to, "to")?,
            }),
            WireTimeRange::Millis { from_ms, to_ms } => Ok(TimeRange::Explicit { from_ms, to_ms }),
        }
    }

    pub(crate) fn into_core(self) -> Result<QuerySpec, ApiError> {
        let range = Self::range_to_core(self.range)?;
        Ok(QuerySpec {
            site_id: self.site_id,
            range,
            granularity: self.granularity,
            metrics: self.metrics,
            group_by: self.group_by,
            filters: self.filters,
            compare_previous: self.compare_previous,
            sort: self.sort,
            limit: self.limit,
            offset: self.offset,
            format: self.format,
        })
    }
}

#[derive(Serialize)]
pub(crate) struct WireWindow {
    from: String,
    to: String,
}

#[derive(Serialize)]
pub(crate) struct WireTimeseriesPoint {
    bucket: String,
    visitors: u64,
    pageviews: u64,
    visits: u64,
}

#[derive(Serialize)]
pub(crate) struct WireQueryResult {
    site_id: SiteId,
    slug: String,
    timezone: String,
    window: WireWindow,
    summary: KpiSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_summary: Option<KpiSummary>,
    timeseries: Vec<WireTimeseriesPoint>,
    rows: Vec<BreakdownRow>,
    meta: QueryMeta,
}

impl From<QueryResult> for WireQueryResult {
    fn from(res: QueryResult) -> Self {
        Self {
            site_id: res.site_id,
            slug: res.slug,
            timezone: res.timezone,
            window: WireWindow {
                from: format_rfc3339(res.window_from_ms),
                to: format_rfc3339(res.window_to_ms),
            },
            summary: res.summary,
            previous_summary: res.previous_summary,
            timeseries: res
                .timeseries
                .into_iter()
                .map(|p| WireTimeseriesPoint {
                    bucket: format_rfc3339(p.bucket_ms),
                    visitors: p.visitors,
                    pageviews: p.pageviews,
                    visits: p.visits,
                })
                .collect(),
            rows: res.rows,
            meta: res.meta,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct WireRealtimePulse {
    minute: String,
    events: u64,
}

#[derive(Serialize)]
pub(crate) struct WireRealtimeSnapshot {
    site_id: SiteId,
    slug: String,
    active_visitors: u64,
    active_pages: Vec<RealtimePage>,
    pulse: Vec<WireRealtimePulse>,
}

impl From<RealtimeSnapshot> for WireRealtimeSnapshot {
    fn from(snap: RealtimeSnapshot) -> Self {
        Self {
            site_id: snap.site_id,
            slug: snap.slug,
            active_visitors: snap.active_visitors,
            active_pages: snap.active_pages,
            pulse: snap
                .pulse
                .into_iter()
                .map(|p| WireRealtimePulse {
                    minute: format_rfc3339(p.minute_ms),
                    events: p.events,
                })
                .collect(),
        }
    }
}

fn parse_iso_ms(raw: &str, field: &'static str) -> Result<i64, ApiError> {
    DateTime::parse_from_rfc3339(raw.trim())
        .map(|dt| dt.timestamp_millis())
        .map_err(|err| {
            ApiError::bad_request(
                "invalid_query_range",
                format!("invalid `{field}` RFC 3339 timestamp: {err}"),
            )
            .with_details(serde_json::json!({ "field": field }))
        })
}
