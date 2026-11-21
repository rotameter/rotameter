#![forbid(unsafe_code)]
//! Filter, sort, and output format types for analytics queries.

use serde::{Deserialize, Serialize};

use crate::query::{Dimension, Metric};

/// Filter comparison operator supported in `v1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    /// Exact match.
    Eq,
    /// Negated exact match.
    Neq,
    /// Membership against a non-empty list of strings.
    In,
    /// Substring match.
    Contains,
    /// Prefix match.
    Prefix,
}

/// Scalar or list operand attached to a [`QueryFilter`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    /// Single string value for `eq`, `neq`, `contains`, or `prefix`.
    Scalar(String),
    /// List of allowed values for `in`.
    List(Vec<String>),
}

/// Single composable dimension filter predicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryFilter {
    /// Target dimension column.
    pub dimension: Dimension,
    /// Comparison operator.
    pub op: FilterOp,
    /// Comparison value or list.
    pub value: FilterValue,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    /// Ascending order.
    Asc,
    /// Descending order (default).
    #[default]
    Desc,
}

/// Sort instruction applied to breakdown rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySort {
    /// Metric to sort by.
    pub by: Metric,
    /// Sort direction.
    #[serde(default)]
    pub dir: SortDir,
}

/// Response serialization format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryFormat {
    /// Typed JSON envelope (default).
    #[default]
    Json,
    /// `text/csv` export.
    Csv,
}
