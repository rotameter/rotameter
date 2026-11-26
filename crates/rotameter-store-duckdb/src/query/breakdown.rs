#![forbid(unsafe_code)]
//! Multi-dimension breakdown grouping, sorting, cardinality flagging, and pagination for `DuckDB`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use duckdb::{Connection, params_from_iter, types::Value};
use rotameter_core::cardinality::OVERFLOW_BUCKET;
use rotameter_core::error::CoreError;
use rotameter_core::query::{Dimension, Metric, QueryFilter, QuerySort, SortDir};
use rotameter_core::storage::BreakdownRow;
use rotameter_core::types::Site;

use crate::query::filters::{build_filter_plan, dim_binding};
use crate::query::summary::{build_kpi_summary, non_neg_u64, u64_to_f64};
use crate::store::map_duck_err;

pub(crate) struct BreakdownOutcome {
    pub(crate) rows: Vec<BreakdownRow>,
    pub(crate) total_rows: u64,
    pub(crate) truncated_by_cardinality_cap: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn query_breakdown(
    conn: &Connection,
    site: &Site,
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
        Value::Blob(site.site_id.as_bytes().to_vec()),
        Value::BigInt(from_ms),
        Value::BigInt(to_ms),
    ];
    let plan = build_filter_plan(filters, group_by, &mut params)?;
    let dim_cols: Vec<(&'static str, &'static str, String)> = group_by
        .iter()
        .enumerate()
        .map(|(i, dim)| {
            let b = dim_binding(*dim);
            (b.key, b.expr, format!("k{i}"))
        })
        .collect();
    let where_tail = plan.where_sql();

    let sql = if plan.needs_events {
        build_event_breakdown_sql(&dim_cols, &where_tail)
    } else {
        build_session_breakdown_sql(&dim_cols, &where_tail)
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

fn build_session_breakdown_sql(
    dim_cols: &[(&'static str, &'static str, String)],
    where_tail: &str,
) -> String {
    let select_dims: Vec<String> = dim_cols
        .iter()
        .map(|(_, expr, alias)| format!("{expr} AS {alias}"))
        .collect();
    let group_aliases: Vec<&str> = dim_cols.iter().map(|(_, _, a)| a.as_str()).collect();
    format!(
        "SELECT {dims},
                COUNT(DISTINCT s.visitor_id),
                CAST(COALESCE(SUM(s.pageview_count), 0) AS BIGINT),
                COUNT(*),
                CAST(COALESCE(SUM(CASE WHEN s.event_count <= 1 THEN 1 ELSE 0 END), 0) AS BIGINT),
                CAST(COALESCE(SUM(GREATEST(0, s.last_event_ms - s.started_at_ms)), 0) AS BIGINT)
         FROM sessions s
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}
         GROUP BY {groups}",
        dims = select_dims.join(", "),
        groups = group_aliases.join(", "),
    )
}

fn build_event_breakdown_sql(
    dim_cols: &[(&'static str, &'static str, String)],
    where_tail: &str,
) -> String {
    let select_dims: Vec<String> = dim_cols
        .iter()
        .map(|(_, expr, alias)| format!("{expr} AS {alias}"))
        .collect();
    let group_aliases: Vec<&str> = dim_cols.iter().map(|(_, _, a)| a.as_str()).collect();
    let alias_csv = group_aliases.join(", ");
    format!(
        "WITH matched AS (
             SELECT {dims},
                    e.visitor_id AS visitor_id,
                    e.session_id AS session_id,
                    CASE WHEN e.event_kind = 'pageview' THEN 1 ELSE 0 END AS is_pv,
                    s.event_count AS event_count,
                    GREATEST(0, s.last_event_ms - s.started_at_ms) AS dur_ms
             FROM events e
             JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
             WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}
         ),
         sess_agg AS (
             SELECT {alias_csv},
                    session_id,
                    visitor_id,
                    SUM(is_pv) AS pv_cnt,
                    MAX(CASE WHEN event_count <= 1 THEN 1 ELSE 0 END) AS is_bounce,
                    MAX(dur_ms) AS dur_ms
             FROM matched
             GROUP BY {alias_csv}, session_id, visitor_id
         )
         SELECT {alias_csv},
                COUNT(DISTINCT visitor_id),
                CAST(COALESCE(SUM(pv_cnt), 0) AS BIGINT),
                COUNT(*),
                CAST(COALESCE(SUM(is_bounce), 0) AS BIGINT),
                CAST(COALESCE(SUM(dur_ms), 0) AS BIGINT)
         FROM sess_agg
         GROUP BY {alias_csv}",
        dims = select_dims.join(", "),
    )
}

fn read_breakdown_rows(
    conn: &Connection,
    sql: &str,
    params: &[Value],
    dim_cols: &[(&'static str, &'static str, String)],
) -> Result<(Vec<BreakdownRow>, bool), CoreError> {
    let mut stmt = conn.prepare(sql).map_err(map_duck_err)?;
    let mut cursor = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_duck_err)?;
    let offset = dim_cols.len();
    let mut out = Vec::new();
    let mut truncated = false;

    while let Some(row) = cursor.next().map_err(map_duck_err)? {
        let mut key = BTreeMap::new();
        for (idx, (dim_name, _, _)) in dim_cols.iter().enumerate() {
            let val: String = row.get(idx).map_err(map_duck_err)?;
            if val == OVERFLOW_BUCKET {
                truncated = true;
            }
            key.insert((*dim_name).to_string(), val);
        }
        let visitors = non_neg_u64(row.get(offset).map_err(map_duck_err)?);
        let pageviews = non_neg_u64(row.get(offset + 1).map_err(map_duck_err)?);
        let visits = non_neg_u64(row.get(offset + 2).map_err(map_duck_err)?);
        let bounced = non_neg_u64(row.get(offset + 3).map_err(map_duck_err)?);
        let duration = non_neg_u64(row.get(offset + 4).map_err(map_duck_err)?);
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
