#![forbid(unsafe_code)]
//! Storage capability flags and analytics query/realtime response models.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::SiteId;

/// Capability flags describing what a [`Storage`] backend supports (`SPEC-ARCHITECTURE.md` §2).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageCapabilities {
    /// Backend preserves raw ordered per-session event logs (`events` table).
    pub supports_raw_event_log: bool,
    /// Backend supports exact `COUNT(DISTINCT visitor_id)` across arbitrary date ranges.
    pub supports_arbitrary_distinct_visitors: bool,
    /// Backend supports concurrent writes from multiple horizontal compute nodes.
    pub supports_multi_node_concurrent_writes: bool,
    /// Backend supports multi-step conversion funnel queries directly in SQL (`v2.0`).
    pub supports_sql_funnels: bool,
}

/// Headline KPI metrics matching `SPEC-API.md` §5.1 and `COMPARISON-METRICS.md`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KpiSummary {
    /// Distinct pseudonymous visitors.
    pub visitors: u64,
    /// Total `pageview` events.
    pub pageviews: u64,
    /// Total 30-minute sliding sessions started.
    pub visits: u64,
    /// `pageviews / max(visits, 1)`.
    pub views_per_visit: f64,
    /// Single-event session fraction in `[0.0, 1.0]`.
    pub bounce_rate: f64,
    /// Arithmetic mean session duration in milliseconds.
    pub avg_visit_duration_ms: u64,
}

/// Single bucketed time-series point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeseriesPoint {
    /// Bucket start timestamp in Unix milliseconds (UTC).
    pub bucket_ms: i64,
    /// Distinct visitors in this bucket.
    pub visitors: u64,
    /// Pageviews in this bucket.
    pub pageviews: u64,
    /// Visits started in this bucket.
    pub visits: u64,
}

/// Single grouped breakdown row returned by `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakdownRow {
    /// Grouped dimension key-value pairs (e.g., `{ "path": "/projects/rotameter" }`).
    pub key: BTreeMap<String, String>,
    /// Distinct visitors in this slice.
    pub visitors: u64,
    /// Pageviews in this slice.
    pub pageviews: u64,
    /// Visits started in this slice.
    pub visits: u64,
    /// Single-event session fraction in `[0.0, 1.0]`.
    pub bounce_rate: f64,
    /// Arithmetic mean session duration in milliseconds.
    pub avg_visit_duration_ms: u64,
}

/// Query execution diagnostics returned alongside query results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryMeta {
    /// Total matched groups before `limit`/`offset` pagination.
    pub total_rows: u64,
    /// Server-side query latency in milliseconds.
    pub query_ms: f64,
    /// `true` if any queried dimension hit the `(other)` cardinality overflow bucket.
    pub truncated_by_cardinality_cap: bool,
}

/// Full typed response payload backing `POST /v1/query`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    /// Target site UID.
    pub site_id: SiteId,
    /// Org-unique site slug.
    pub slug: String,
    /// Site display timezone.
    pub timezone: String,
    /// Inclusive query start in Unix milliseconds.
    pub window_from_ms: i64,
    /// Exclusive query end in Unix milliseconds.
    pub window_to_ms: i64,
    /// Aggregate KPIs for the primary window.
    pub summary: KpiSummary,
    /// Optional KPIs for the immediately preceding comparison window.
    pub previous_summary: Option<KpiSummary>,
    /// Bucketed time series when `granularity` was requested.
    pub timeseries: Vec<TimeseriesPoint>,
    /// Paginated breakdown rows when `group_by` was requested.
    pub rows: Vec<BreakdownRow>,
    /// Execution metadata.
    pub meta: QueryMeta,
}

/// Active page summary entry inside a [`RealtimeSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimePage {
    /// Normalized page path.
    pub path: String,
    /// Active visitors seen on this page in the last 5 minutes.
    pub active_visitors: u64,
}

/// Single 1-minute activity sample in the 30-minute realtime pulse strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimePulse {
    /// Minute floor timestamp in Unix milliseconds.
    pub minute_ms: i64,
    /// Events observed during this minute.
    pub events: u64,
}

/// Snapshot backing `GET /v1/sites/{site}/realtime`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimeSnapshot {
    /// Target site UID.
    pub site_id: SiteId,
    /// Org-unique site slug.
    pub slug: String,
    /// Distinct non-bot visitors active in the trailing 5 minutes.
    pub active_visitors: u64,
    /// Ranked active pages in the trailing 5 minutes.
    pub active_pages: Vec<RealtimePage>,
    /// Trailing 30-minute per-minute pulse.
    pub pulse: Vec<RealtimePulse>,
}
