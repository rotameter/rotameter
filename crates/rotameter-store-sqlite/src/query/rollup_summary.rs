#![forbid(unsafe_code)]
//! `SQL` executors for rollup, session-slice, and filtered event KPI summaries.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::storage::KpiSummary;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};

use crate::config::map_sql_err;
use crate::query::summary::{RollupAgg, build_kpi_summary, non_neg_u64};

pub(crate) fn rollup_cutoffs(
    conn: &Connection,
    site_id: SiteId,
    raw_floor: i64,
    has_live_sessions: bool,
) -> Result<(Option<i64>, Option<i32>), CoreError> {
    let min_hourly: Option<i64> = conn
        .query_row(
            "SELECT MIN(bucket_ts_ms) FROM rollups_hourly WHERE site_id = ?1",
            [site_id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_sql_err)?
        .flatten();
    let raw_day = has_live_sessions.then(|| utc_yyyymmdd(raw_floor));
    let hourly_day = min_hourly.map(utc_yyyymmdd);
    let exclude_day = [raw_day, hourly_day].into_iter().flatten().min();
    Ok((min_hourly, exclude_day))
}

pub(crate) fn query_historical_rollup(
    conn: &Connection,
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
    raw_floor: i64,
    has_live_sessions: bool,
) -> Result<RollupAgg, CoreError> {
    let site_blob = site_id.as_bytes().as_slice();
    let (min_hourly, exclude_day) = rollup_cutoffs(conn, site_id, raw_floor, has_live_sessions)?;
    let mut agg = RollupAgg::default();

    if let Some(min_h) = min_hourly
        && to_ms > min_h
    {
        let h_start = from_ms.max(min_h).div_euclid(MS_PER_HOUR) * MS_PER_HOUR;
        let (pv, vs, bv, dur): (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(pageviews), 0),
                        COALESCE(SUM(visits_started), 0),
                        COALESCE(SUM(bounced_visits), 0),
                        COALESCE(SUM(total_duration_ms), 0)
                 FROM rollups_hourly
                 WHERE site_id = ?1 AND bucket_ts_ms >= ?2 AND bucket_ts_ms < ?3",
                params![site_blob, h_start, to_ms],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(map_sql_err)?;
        agg.pageviews += non_neg_u64(pv);
        agg.visits += non_neg_u64(vs);
        agg.bounced_visits += non_neg_u64(bv);
        agg.total_duration_ms += non_neg_u64(dur);
    }

    let daily_end = min_hourly.map_or(to_ms, |h| to_ms.min(h));
    if from_ms < daily_end {
        let day_from = utc_yyyymmdd(from_ms);
        let day_to = utc_yyyymmdd(daily_end.saturating_sub(1).max(from_ms));
        let (pv, vs, bv, dur): (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT COALESCE(SUM(pageviews), 0),
                        COALESCE(SUM(visits_started), 0),
                        COALESCE(SUM(bounced_visits), 0),
                        COALESCE(SUM(total_duration_ms), 0)
                 FROM rollups_daily
                 WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3
                   AND (?4 IS NULL OR day_utc < ?4)",
                params![site_blob, day_from, day_to, exclude_day],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(map_sql_err)?;
        agg.pageviews += non_neg_u64(pv);
        agg.visits += non_neg_u64(vs);
        agg.bounced_visits += non_neg_u64(bv);
        agg.total_duration_ms += non_neg_u64(dur);
    }
    Ok(agg)
}

pub(crate) fn query_session_slice_raw(
    conn: &Connection,
    joins: &str,
    where_tail: &str,
    params: &[Value],
) -> Result<(u64, RollupAgg), CoreError> {
    let sql = format!(
        "SELECT COUNT(DISTINCT s.visitor_id),
                COALESCE(SUM(s.pageview_count), 0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN s.event_count <= 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(MAX(0, s.last_event_ms - s.started_at_ms)), 0)
         FROM sessions s {joins}
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}"
    );
    let (visitors, pageviews, visits, bounced, duration): (i64, i64, i64, i64, i64) = conn
        .query_row(&sql, params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(map_sql_err)?;
    Ok((
        non_neg_u64(visitors),
        RollupAgg {
            pageviews: non_neg_u64(pageviews),
            visits: non_neg_u64(visits),
            bounced_visits: non_neg_u64(bounced),
            total_duration_ms: non_neg_u64(duration),
        },
    ))
}

pub(crate) fn query_session_slice_summary(
    conn: &Connection,
    joins: &str,
    where_tail: &str,
    params: &[Value],
) -> Result<KpiSummary, CoreError> {
    let (visitors, agg) = query_session_slice_raw(conn, joins, where_tail, params)?;
    Ok(build_kpi_summary(
        visitors,
        agg.pageviews,
        agg.visits,
        agg.bounced_visits,
        agg.total_duration_ms,
    ))
}

pub(crate) fn query_filtered_event_summary(
    conn: &Connection,
    joins: &str,
    where_tail: &str,
    params: &[Value],
) -> Result<KpiSummary, CoreError> {
    let ev_sql = format!(
        "SELECT COUNT(DISTINCT e.visitor_id),
                COALESCE(SUM(CASE WHEN en_pv.value = 'pageview' THEN 1 ELSE 0 END), 0)
         FROM events e
         JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
         JOIN dim_strings en_pv ON en_pv.id = e.event_name_id
         {joins}
         WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}"
    );
    let (visitors, pageviews): (i64, i64) = conn
        .query_row(&ev_sql, params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(map_sql_err)?;

    let sess_sql = format!(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN s_out.event_count <= 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(MAX(0, s_out.last_event_ms - s_out.started_at_ms)), 0)
         FROM sessions s_out
         WHERE s_out.site_id = ?1 AND s_out.session_id IN (
             SELECT DISTINCT e.session_id
             FROM events e
             JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
             {joins}
             WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}
         )"
    );
    let (visits, bounced, duration): (i64, i64, i64) = conn
        .query_row(&sess_sql, params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(map_sql_err)?;

    Ok(build_kpi_summary(
        non_neg_u64(visitors),
        non_neg_u64(pageviews),
        non_neg_u64(visits),
        non_neg_u64(bounced),
        non_neg_u64(duration),
    ))
}
