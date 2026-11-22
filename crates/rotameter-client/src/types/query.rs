#![forbid(unsafe_code)]
//! Wire DTOs for `POST /v1/query`.

use rotameter_core::id::{SiteId, SiteRef};
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFilter, QueryFormat, QuerySort, QuerySpec, RangePreset,
    TimeRange,
};
use rotameter_core::storage::{BreakdownRow, KpiSummary, QueryMeta};
use serde::{Deserialize, Serialize};

/// Time window selector accepted by `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WireTimeRange {
    /// Predefined trailing preset (`24h`, `7d`, `30d`, `12m`, `all`).
    Preset {
        /// Preset window identifier.
        preset: RangePreset,
    },
    /// Explicit `RFC 3339` UTC timestamp bounds.
    Iso {
        /// Inclusive window start in `RFC 3339` format.
        from: String,
        /// Exclusive window end in `RFC 3339` format.
        to: String,
    },
    /// Explicit Unix millisecond bounds `[from_ms, to_ms)`.
    Millis {
        /// Inclusive window start in Unix milliseconds.
        from_ms: i64,
        /// Exclusive window end in Unix milliseconds.
        to_ms: i64,
    },
}

impl From<TimeRange> for WireTimeRange {
    fn from(range: TimeRange) -> Self {
        match range {
            TimeRange::Preset { preset } => Self::Preset { preset },
            TimeRange::Explicit { from_ms, to_ms } => Self::Millis { from_ms, to_ms },
        }
    }
}

/// Request payload sent to `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Target site identifier (`site_...` or `slug`).
    pub site_id: SiteRef,
    /// Query time range.
    pub range: WireTimeRange,
    /// Optional time-series bucket granularity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    /// Requested metrics (`>= 1`).
    pub metrics: Vec<Metric>,
    /// Breakdown dimensions.
    #[serde(default)]
    pub group_by: Vec<Dimension>,
    /// Composable filter predicates.
    #[serde(default)]
    pub filters: Vec<QueryFilter>,
    /// Whether to also return KPIs for the immediately preceding comparison window.
    #[serde(default)]
    pub compare_previous: bool,
    /// Optional breakdown sort order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<QuerySort>,
    /// Maximum breakdown rows returned (`1..=1000`).
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Pagination offset into breakdown rows.
    #[serde(default)]
    pub offset: u32,
    /// Serialization format (`json` or `csv`).
    #[serde(default)]
    pub format: QueryFormat,
}

const fn default_limit() -> u32 {
    50
}

impl QueryRequest {
    /// Builds a [`QueryRequest`] with default limit (`50`), empty filters/grouping, and `json` format.
    pub fn new(site_id: SiteRef, range: WireTimeRange, metrics: Vec<Metric>) -> Self {
        Self {
            site_id,
            range,
            granularity: None,
            metrics,
            group_by: Vec::new(),
            filters: Vec::new(),
            compare_previous: false,
            sort: None,
            limit: default_limit(),
            offset: 0,
            format: QueryFormat::Json,
        }
    }
}

impl From<QuerySpec> for QueryRequest {
    fn from(spec: QuerySpec) -> Self {
        Self {
            site_id: spec.site_id,
            range: WireTimeRange::from(spec.range),
            granularity: spec.granularity,
            metrics: spec.metrics,
            group_by: spec.group_by,
            filters: spec.filters,
            compare_previous: spec.compare_previous,
            sort: spec.sort,
            limit: spec.limit,
            offset: spec.offset,
            format: spec.format,
        }
    }
}

/// Resolved `RFC 3339` time bounds returned inside [`QueryResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryWindow {
    /// Inclusive window start (`RFC 3339` UTC).
    pub from: String,
    /// Exclusive window end (`RFC 3339` UTC).
    pub to: String,
}

/// Single time-series bucket point returned by `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeseriesRow {
    /// Bucket start timestamp (`RFC 3339` UTC).
    pub bucket: String,
    /// Distinct visitors in this bucket.
    pub visitors: u64,
    /// Pageviews in this bucket.
    pub pageviews: u64,
    /// Visits started in this bucket.
    pub visits: u64,
}

/// Typed `JSON` response returned by `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    /// Immutable target site UID.
    pub site_id: SiteId,
    /// Org-unique site slug.
    pub slug: String,
    /// Site display timezone.
    pub timezone: String,
    /// Effective query time bounds.
    pub window: QueryWindow,
    /// Headline KPI summary for the primary window.
    pub summary: KpiSummary,
    /// Optional KPI summary for the preceding comparison window.
    #[serde(default)]
    pub previous_summary: Option<KpiSummary>,
    /// Bucketed time-series points when `granularity` was requested.
    pub timeseries: Vec<TimeseriesRow>,
    /// Grouped breakdown rows when `group_by` was requested.
    pub rows: Vec<BreakdownRow>,
    /// Execution latency and cardinality metadata.
    pub meta: QueryMeta,
}
