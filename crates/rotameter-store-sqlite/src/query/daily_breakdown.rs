#![forbid(unsafe_code)]
//! Fast-path multi-day `Dimension::Path` breakdown backed by `rollups_path_daily`.

use std::collections::BTreeMap;

use rotameter_core::cardinality::OVERFLOW_BUCKET;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::query::{Metric, QuerySort, SortDir};
use rotameter_core::storage::BreakdownRow;
use rusqlite::{Connection, params};

use crate::config::map_sql_err;
use crate::query::breakdown::{BreakdownOutcome, compare_rows};
use crate::query::daily_fast::{daily_window_bounds, events_edge_clean};
use crate::query::summary::{build_kpi_summary, non_neg_u64};

#[allow(clippy::too_many_arguments)]
pub(crate) fn try_daily_path_breakdown(
    conn: &Connection,
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
    primary_metric: Metric,
    sort: Option<&QuerySort>,
    limit: u32,
    offset: u32,
) -> Result<Option<BreakdownOutcome>, CoreError> {
    let Some((day_from, day_to, day_from_start, day_to_end)) = daily_window_bounds(from_ms, to_ms)
    else {
        return Ok(None);
    };
    if !events_edge_clean(conn, site_id, day_from_start, from_ms, to_ms, day_to_end)? {
        return Ok(None);
    }
    let mut stmt = conn
        .prepare(
            "SELECT d.value, COALESCE(SUM(r.visitors), 0), COALESCE(SUM(r.pageviews), 0),
                    COALESCE(SUM(r.visits), 0), COALESCE(SUM(r.bounced_visits), 0),
                    COALESCE(SUM(r.total_duration_ms), 0)
             FROM rollups_path_daily r
             JOIN dim_strings d ON d.id = r.path_id
             WHERE r.site_id = ?1 AND r.day_utc >= ?2 AND r.day_utc <= ?3
             GROUP BY r.path_id",
        )
        .map_err(map_sql_err)?;
    let mut cursor = stmt
        .query(params![site_id.as_bytes().as_slice(), day_from, day_to])
        .map_err(map_sql_err)?;
    let (mut rows, mut truncated) = (Vec::new(), false);
    while let Some(row) = cursor.next().map_err(map_sql_err)? {
        let path: String = row.get(0).map_err(map_sql_err)?;
        if path == OVERFLOW_BUCKET {
            truncated = true;
        }
        let mut key = BTreeMap::new();
        key.insert("path".to_string(), path);
        let kpi = build_kpi_summary(
            non_neg_u64(row.get(1).map_err(map_sql_err)?),
            non_neg_u64(row.get(2).map_err(map_sql_err)?),
            non_neg_u64(row.get(3).map_err(map_sql_err)?),
            non_neg_u64(row.get(4).map_err(map_sql_err)?),
            non_neg_u64(row.get(5).map_err(map_sql_err)?),
        );
        rows.push(BreakdownRow {
            key,
            visitors: kpi.visitors,
            pageviews: kpi.pageviews,
            visits: kpi.visits,
            bounce_rate: kpi.bounce_rate,
            avg_visit_duration_ms: kpi.avg_visit_duration_ms,
        });
    }
    if rows.is_empty() {
        return Ok(None);
    }
    rows.sort_by(|a, b| {
        compare_rows(
            a,
            b,
            sort.map_or(primary_metric, |s| s.by),
            sort.map_or(SortDir::Desc, |s| s.dir),
        )
    });
    let total_rows = u64::try_from(rows.len()).unwrap_or(u64::MAX);
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(rows.len());
    let sliced = rows
        .into_iter()
        .skip(start)
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect();
    Ok(Some(BreakdownOutcome {
        rows: sliced,
        total_rows,
        truncated_by_cardinality_cap: truncated,
    }))
}
