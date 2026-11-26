#![forbid(unsafe_code)]
//! `QueryResult <-> RecordBatch` adapters (`query_timeseries`, `query_rows`).

use std::sync::Arc;

use arrow_array::builder::{Float64Builder, Int64Builder, StringBuilder, UInt64Builder};
use arrow_array::{ArrayRef, RecordBatch};
use rotameter_core::id::SiteId;
use rotameter_core::storage::{BreakdownRow, KpiSummary, QueryMeta, QueryResult, TimeseriesPoint};

use crate::ArrowError;
use crate::col_ext::{col_f64, col_i64, col_str, col_u64};
use crate::schema::{query_rows_schema, query_timeseries_schema};

/// Arrow columnar projection of a typed [`QueryResult`].
#[derive(Debug, Clone)]
pub struct QueryResultArrow {
    /// Target site UID.
    pub site_id: SiteId,
    /// Org-unique site slug.
    pub slug: String,
    /// Site display timezone.
    pub timezone: String,
    /// Inclusive window start in Unix milliseconds.
    pub window_from_ms: i64,
    /// Exclusive window end in Unix milliseconds.
    pub window_to_ms: i64,
    /// Primary KPI summary.
    pub summary: KpiSummary,
    /// Optional previous-window KPI summary.
    pub previous_summary: Option<KpiSummary>,
    /// Execution diagnostics metadata.
    pub meta: QueryMeta,
    /// Time-series bucket points ([`query_timeseries_schema`]).
    pub timeseries: RecordBatch,
    /// Grouped breakdown rows ([`query_rows_schema`]).
    pub rows: RecordBatch,
}

/// Converts a [`QueryResult`] into [`QueryResultArrow`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow array building or JSON key serialization fails.
pub fn query_result_to_arrow(res: &QueryResult) -> Result<QueryResultArrow, ArrowError> {
    let tn = res.timeseries.len();
    let mut t_bucket = Int64Builder::with_capacity(tn);
    let mut t_vis = UInt64Builder::with_capacity(tn);
    let mut t_pv = UInt64Builder::with_capacity(tn);
    let mut t_visits = UInt64Builder::with_capacity(tn);
    for p in &res.timeseries {
        t_bucket.append_value(p.bucket_ms);
        t_vis.append_value(p.visitors);
        t_pv.append_value(p.pageviews);
        t_visits.append_value(p.visits);
    }
    let ts_cols: Vec<ArrayRef> = vec![
        Arc::new(t_bucket.finish()),
        Arc::new(t_vis.finish()),
        Arc::new(t_pv.finish()),
        Arc::new(t_visits.finish()),
    ];
    let timeseries = RecordBatch::try_new(query_timeseries_schema(), ts_cols)?;

    let rn = res.rows.len();
    let mut r_key = StringBuilder::with_capacity(rn, rn * 24);
    let mut r_vis = UInt64Builder::with_capacity(rn);
    let mut r_pv = UInt64Builder::with_capacity(rn);
    let mut r_visits = UInt64Builder::with_capacity(rn);
    let mut r_bounce = Float64Builder::with_capacity(rn);
    let mut r_dur = UInt64Builder::with_capacity(rn);
    for row in &res.rows {
        r_key.append_value(serde_json::to_string(&row.key)?);
        r_vis.append_value(row.visitors);
        r_pv.append_value(row.pageviews);
        r_visits.append_value(row.visits);
        r_bounce.append_value(row.bounce_rate);
        r_dur.append_value(row.avg_visit_duration_ms);
    }
    let row_cols: Vec<ArrayRef> = vec![
        Arc::new(r_key.finish()),
        Arc::new(r_vis.finish()),
        Arc::new(r_pv.finish()),
        Arc::new(r_visits.finish()),
        Arc::new(r_bounce.finish()),
        Arc::new(r_dur.finish()),
    ];
    let rows = RecordBatch::try_new(query_rows_schema(), row_cols)?;

    Ok(QueryResultArrow {
        site_id: res.site_id,
        slug: res.slug.clone(),
        timezone: res.timezone.clone(),
        window_from_ms: res.window_from_ms,
        window_to_ms: res.window_to_ms,
        summary: res.summary.clone(),
        previous_summary: res.previous_summary.clone(),
        meta: res.meta.clone(),
        timeseries,
        rows,
    })
}

/// Reconstructs a typed [`QueryResult`] from [`QueryResultArrow`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow columns are missing or JSON key maps fail to parse.
pub fn arrow_to_query_result(arrow: &QueryResultArrow) -> Result<QueryResult, ArrowError> {
    let t_bucket = col_i64(&arrow.timeseries, "bucket_ms")?;
    let t_vis = col_u64(&arrow.timeseries, "visitors")?;
    let t_pv = col_u64(&arrow.timeseries, "pageviews")?;
    let t_visits = col_u64(&arrow.timeseries, "visits")?;
    let mut timeseries = Vec::with_capacity(arrow.timeseries.num_rows());
    for i in 0..arrow.timeseries.num_rows() {
        timeseries.push(TimeseriesPoint {
            bucket_ms: t_bucket.value(i),
            visitors: t_vis.value(i),
            pageviews: t_pv.value(i),
            visits: t_visits.value(i),
        });
    }

    let r_key = col_str(&arrow.rows, "key_json")?;
    let r_vis = col_u64(&arrow.rows, "visitors")?;
    let r_pv = col_u64(&arrow.rows, "pageviews")?;
    let r_visits = col_u64(&arrow.rows, "visits")?;
    let r_bounce = col_f64(&arrow.rows, "bounce_rate")?;
    let r_dur = col_u64(&arrow.rows, "avg_visit_duration_ms")?;
    let mut rows = Vec::with_capacity(arrow.rows.num_rows());
    for i in 0..arrow.rows.num_rows() {
        rows.push(BreakdownRow {
            key: serde_json::from_str(r_key.value(i))?,
            visitors: r_vis.value(i),
            pageviews: r_pv.value(i),
            visits: r_visits.value(i),
            bounce_rate: r_bounce.value(i),
            avg_visit_duration_ms: r_dur.value(i),
        });
    }

    Ok(QueryResult {
        site_id: arrow.site_id,
        slug: arrow.slug.clone(),
        timezone: arrow.timezone.clone(),
        window_from_ms: arrow.window_from_ms,
        window_to_ms: arrow.window_to_ms,
        summary: arrow.summary.clone(),
        previous_summary: arrow.previous_summary.clone(),
        timeseries,
        rows,
        meta: arrow.meta.clone(),
    })
}
