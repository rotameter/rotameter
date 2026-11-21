#![forbid(unsafe_code)]
//! Multi-dimension breakdown grouping, sorting, cardinality flagging, and pagination.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use rotameter_core::cardinality::OVERFLOW_BUCKET;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::query::{Dimension, Metric, QueryFilter, QuerySort, SortDir};
use rotameter_core::storage::BreakdownRow;
use rusqlite::{Connection, params_from_iter, types::Value};

use crate::config::map_sql_err;
use crate::query::breakdown_sql::{build_event_breakdown_sql, build_session_breakdown_sql};
use crate::query::dim_map::dimension_key_name;
use crate::query::filter_sql::build_filter_plan;
use crate::query::summary::{build_kpi_summary, non_neg_u64, u64_to_f64};

pub(crate) struct BreakdownOutcome {
    pub(crate) rows: Vec<BreakdownRow>,
    pub(crate) total_rows: u64,
    pub(crate) truncated_by_cardinality_cap: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn query_breakdown(
    conn: &Connection,
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
    group_by: &[Dimension],
    filters: &[QueryFilter],
    primary_metric: Metric,
    sort: Option<&QuerySort>,
    limit: u32,
    offset: u32,
) -> Result<BreakdownOutcome, CoreError> {
    if group_by.is_empty() {
        return Ok(BreakdownOutcome {
            rows: Vec::new(),
            total_rows: 0,
            truncated_by_cardinality_cap: false,
        });
    }

    let mut params = vec![
        Value::Blob(site_id.as_bytes().to_vec()),
        Value::Integer(from_ms),
        Value::Integer(to_ms),
    ];
    let mut plan = build_filter_plan(filters, group_by, &mut params)?;
    let dim_cols: Vec<(&'static str, &'static str, String)> = group_by
        .iter()
        .enumerate()
        .map(|(i, dim)| {
            (
                dimension_key_name(*dim),
                plan.ensure_dimension(*dim),
                format!("k{i}"),
            )
        })
        .collect();
    let joins = plan.join_sql();
    let where_tail = plan.where_sql();

    let sql = if plan.needs_events {
        build_event_breakdown_sql(&dim_cols, &joins, &where_tail)
    } else {
        build_session_breakdown_sql(&dim_cols, &joins, &where_tail)
    };

    let (mut rows, truncated) = read_breakdown_rows(conn, &sql, &params, &dim_cols)?;
    let sort_metric = sort.map_or(primary_metric, |s| s.by);
    let sort_dir = sort.map_or(SortDir::Desc, |s| s.dir);
    rows.sort_by(|a, b| compare_rows(a, b, sort_metric, sort_dir));

    let total_rows = u64::try_from(rows.len()).unwrap_or(u64::MAX);
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(rows.len());
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let sliced = rows.into_iter().skip(start).take(take).collect();

    Ok(BreakdownOutcome {
        rows: sliced,
        total_rows,
        truncated_by_cardinality_cap: truncated,
    })
}

fn read_breakdown_rows(
    conn: &Connection,
    sql: &str,
    params: &[Value],
    dim_cols: &[(&'static str, &'static str, String)],
) -> Result<(Vec<BreakdownRow>, bool), CoreError> {
    let mut stmt = conn.prepare(sql).map_err(map_sql_err)?;
    let mut cursor = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_sql_err)?;
    let offset = dim_cols.len();
    let mut out = Vec::new();
    let mut truncated = false;

    while let Some(row) = cursor.next().map_err(map_sql_err)? {
        let mut key = BTreeMap::new();
        for (idx, (dim_name, _, _)) in dim_cols.iter().enumerate() {
            let val: String = row.get(idx).map_err(map_sql_err)?;
            if val == OVERFLOW_BUCKET {
                truncated = true;
            }
            key.insert((*dim_name).to_string(), val);
        }
        let visitors = non_neg_u64(row.get(offset).map_err(map_sql_err)?);
        let pageviews = non_neg_u64(row.get(offset + 1).map_err(map_sql_err)?);
        let visits = non_neg_u64(row.get(offset + 2).map_err(map_sql_err)?);
        let bounced = non_neg_u64(row.get(offset + 3).map_err(map_sql_err)?);
        let duration = non_neg_u64(row.get(offset + 4).map_err(map_sql_err)?);
        let kpi = build_kpi_summary(visitors, pageviews, visits, bounced, duration);
        out.push(BreakdownRow {
            key,
            visitors: kpi.visitors,
            pageviews: kpi.pageviews,
            visits: kpi.visits,
            bounce_rate: kpi.bounce_rate,
            avg_visit_duration_ms: kpi.avg_visit_duration_ms,
        });
    }
    Ok((out, truncated))
}

fn compare_rows(a: &BreakdownRow, b: &BreakdownRow, metric: Metric, dir: SortDir) -> Ordering {
    let ord = match metric {
        Metric::Visitors => a.visitors.cmp(&b.visitors),
        Metric::Pageviews => a.pageviews.cmp(&b.pageviews),
        Metric::Visits => a.visits.cmp(&b.visits),
        Metric::ViewsPerVisit => {
            let av = u64_to_f64(a.pageviews) / u64_to_f64(a.visits.max(1));
            let bv = u64_to_f64(b.pageviews) / u64_to_f64(b.visits.max(1));
            av.total_cmp(&bv)
        }
        Metric::BounceRate => a.bounce_rate.total_cmp(&b.bounce_rate),
        Metric::AvgVisitDurationMs => a.avg_visit_duration_ms.cmp(&b.avg_visit_duration_ms),
    };
    let directed = match dir {
        SortDir::Asc => ord,
        SortDir::Desc => ord.reverse(),
    };
    directed.then_with(|| a.key.cmp(&b.key))
}
