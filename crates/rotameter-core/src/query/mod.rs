#![forbid(unsafe_code)]
//! Typed analytics query contract matching `POST /v1/query` in `SPEC-API.md`.

pub mod fields;
pub mod filter;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::id::SiteRef;

pub use fields::{Dimension, Metric};
pub use filter::{FilterOp, FilterValue, QueryFilter, QueryFormat, QuerySort, SortDir};

/// Hard upper bound on query rows returned by a single `v1.0` breakdown request.
pub const MAX_QUERY_LIMIT: u32 = 1_000;

/// Relative time-range presets offered in API, CLI, MCP, and TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangePreset {
    /// Trailing 24 hours.
    #[serde(rename = "24h")]
    Hours24,
    /// Trailing 7 days.
    #[serde(rename = "7d")]
    Days7,
    /// Trailing 30 days.
    #[serde(rename = "30d")]
    Days30,
    /// Trailing 12 months.
    #[serde(rename = "12m")]
    Months12,
    /// All available history.
    All,
}

/// Time window selector (`preset` or explicit millisecond bounds `[from_ms, to_ms)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TimeRange {
    /// Predefined trailing preset (`24h`, `7d`, `30d`, `12m`, `all`).
    Preset {
        /// Preset identifier.
        preset: RangePreset,
    },
    /// Explicit UTC millisecond bounds `[from_ms, to_ms)`.
    Explicit {
        /// Inclusive window start in Unix milliseconds.
        from_ms: i64,
        /// Exclusive window end in Unix milliseconds.
        to_ms: i64,
    },
}

/// Time-series bucket granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    /// Hourly buckets.
    Hour,
    /// Daily buckets.
    Day,
    /// Monthly buckets.
    Month,
}

/// Canonical `POST /v1/query` request model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySpec {
    /// Site identifier (`site_...` or `slug`).
    pub site_id: SiteRef,
    /// Query time range.
    pub range: TimeRange,
    /// Optional time-series bucket granularity.
    pub granularity: Option<Granularity>,
    /// Requested metrics (`>= 1`).
    pub metrics: Vec<Metric>,
    /// Breakdown dimensions.
    #[serde(default)]
    pub group_by: Vec<Dimension>,
    /// Composable filter predicates.
    #[serde(default)]
    pub filters: Vec<QueryFilter>,
    /// Whether to compute KPI deltas against the immediately preceding window.
    #[serde(default)]
    pub compare_previous: bool,
    /// Optional breakdown row sort specification.
    pub sort: Option<QuerySort>,
    /// Maximum breakdown rows returned (`1..=1000`).
    #[serde(default = "default_query_limit")]
    pub limit: u32,
    /// Pagination offset into breakdown rows.
    #[serde(default)]
    pub offset: u32,
    /// Output format (`json` or `csv`).
    #[serde(default)]
    pub format: QueryFormat,
}

const fn default_query_limit() -> u32 {
    50
}

impl QuerySpec {
    /// Validates semantic constraints on the query before execution.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidQuery`] if metrics are empty, explicit time bounds are inverted,
    /// limit is outside `1..=1000`, or filter operators mismatch their value variants.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.metrics.is_empty() {
            return Err(CoreError::InvalidQuery(
                "at least one metric must be requested".to_string(),
            ));
        }
        if let TimeRange::Explicit { from_ms, to_ms } = self.range
            && from_ms >= to_ms
        {
            return Err(CoreError::InvalidQuery(
                "`from_ms` must be strictly less than `to_ms`".to_string(),
            ));
        }
        if self.limit == 0 || self.limit > MAX_QUERY_LIMIT {
            return Err(CoreError::InvalidQuery(format!(
                "`limit` must be within 1..={MAX_QUERY_LIMIT}"
            )));
        }
        for filter in &self.filters {
            match (filter.op, &filter.value) {
                (FilterOp::In, FilterValue::List(items)) if !items.is_empty() => {}
                (FilterOp::In, _) => {
                    return Err(CoreError::InvalidQuery(
                        "`in` filter requires a non-empty list value".to_string(),
                    ));
                }
                (_, FilterValue::Scalar(_)) => {}
                (_, FilterValue::List(_)) => {
                    return Err(CoreError::InvalidQuery(
                        "scalar filter operator cannot accept a list value".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}
