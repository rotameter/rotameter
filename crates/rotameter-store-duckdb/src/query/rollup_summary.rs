#![forbid(unsafe_code)]
//! Rollup, session-slice, and filtered event KPI executors for `DuckDB`.

use duckdb::{Connection, OptionalExt, params, params_from_iter, types::Value};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::storage::KpiSummary;

use crate::query::summary::{RollupAgg, build_kpi_summary, non_neg_u64};
use crate::store::map_duck_err;

pub(crate) fn raw_session_hour_floor(
    conn: &Connection,
    site_id: SiteId,
    to_ms: i64,
) -> Result<(i64, bool), CoreError> {
    let min_started: Option<i64> = conn
        .query_row(
            "SELECT MIN(started_at_ms) FROM sessions WHERE site_id = ?1",
            [site_id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_duck_err)?
        .flatten();
    Ok(match min_started {
        Some(ts) => ((ts.div_euclid(MS_PER_HOUR) * MS_PER_HOUR).min(to_ms), true),
        None => (to_ms, false),
    })
}

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
        .map_err(map_duck_err)?
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
                "SELECT CAST(COALESCE(SUM(pageviews), 0) AS BIGINT),
                        CAST(COALESCE(SUM(visits_started), 0) AS BIGINT),
                        CAST(COALESCE(SUM(bounced_visits), 0) AS BIGINT),
                        CAST(COALESCE(SUM(total_duration_ms), 0) AS BIGINT)
                 FROM rollups_hourly
                 WHERE site_id = ?1 AND bucket_ts_ms >= ?2 AND bucket_ts_ms < ?3",
                params![site_blob, h_start, to_ms],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(map_duck_err)?;
        agg.pageviews += non_neg_u64(pv);
        agg.visits += non_neg_u64(vs);
        agg.bounced_visits += non_neg_u64(bv);
        agg.total_duration_ms += non_neg_u64(dur);
    }

    let daily_end = min_hourly.map_or(to_ms, |h| to_ms.min(h));
    if from_ms < daily_end {
        let day_from = utc_yyyymmdd(from_ms);
        let day_to = utc_yyyymmdd(daily_end.saturating_sub(1).max(from_ms));
        let (pv, vs, bv, dur, vis): (i64, i64, i64, i64, i64) = conn
            .query_row(
                "SELECT CAST(COALESCE(SUM(pageviews), 0) AS BIGINT),
                        CAST(COALESCE(SUM(visits_started), 0) AS BIGINT),
                        CAST(COALESCE(SUM(bounced_visits), 0) AS BIGINT),
                        CAST(COALESCE(SUM(total_duration_ms), 0) AS BIGINT),
                        CAST(COALESCE(SUM(visitors), 0) AS BIGINT)
                 FROM rollups_daily
                 WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3
                   AND (?4 IS NULL OR day_utc < ?4)",
                params![site_blob, day_from, day_to, exclude_day],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .map_err(map_duck_err)?;
        agg.pageviews += non_neg_u64(pv);
        agg.visits += non_neg_u64(vs);
        agg.bounced_visits += non_neg_u64(bv);
        agg.total_duration_ms += non_neg_u64(dur);
        agg.hist_visitors += non_neg_u64(vis);
    }
    Ok(agg)
}

pub(crate) fn query_session_slice_raw(
    conn: &Connection,
    where_tail: &str,
    params: &[Value],
) -> Result<(u64, RollupAgg), CoreError> {
    let sql = format!(
        "SELECT COUNT(DISTINCT s.visitor_id),
                CAST(COALESCE(SUM(s.pageview_count), 0) AS BIGINT),
                COUNT(*),
                CAST(COALESCE(SUM(CASE WHEN s.event_count <= 1 THEN 1 ELSE 0 END), 0) AS BIGINT),
                CAST(COALESCE(SUM(GREATEST(0, s.last_event_ms - s.started_at_ms)), 0) AS BIGINT)
         FROM sessions s
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}"
    );
    let (visitors, pageviews, visits, bounced, duration): (i64, i64, i64, i64, i64) = conn
        .query_row(&sql, params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(map_duck_err)?;
    Ok((
        non_neg_u64(visitors),
        RollupAgg {
            pageviews: non_neg_u64(pageviews),
            visits: non_neg_u64(visits),
            bounced_visits: non_neg_u64(bounced),
            total_duration_ms: non_neg_u64(duration),
            hist_visitors: 0,
        },
    ))
}

pub(crate) fn query_filtered_event_summary(
    conn: &Connection,
    where_tail: &str,
    params: &[Value],
) -> Result<KpiSummary, CoreError> {
    let sql = format!(
        "WITH matched AS (
             SELECT e.visitor_id,
                    e.session_id,
                    CASE WHEN e.event_kind = 'pageview' THEN 1 ELSE 0 END AS is_pv,
                    s.event_count,
                    GREATEST(0, s.last_event_ms - s.started_at_ms) AS dur_ms
             FROM events e
             JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
             WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}
         ),
         sess_agg AS (
             SELECT session_id,
                    visitor_id,
                    SUM(is_pv) AS pv_cnt,
                    MAX(CASE WHEN event_count <= 1 THEN 1 ELSE 0 END) AS is_bounce,
                    MAX(dur_ms) AS dur_ms
             FROM matched
             GROUP BY session_id, visitor_id
         )
         SELECT COUNT(DISTINCT visitor_id),
                CAST(COALESCE(SUM(pv_cnt), 0) AS BIGINT),
                COUNT(*),
                CAST(COALESCE(SUM(is_bounce), 0) AS BIGINT),
                CAST(COALESCE(SUM(dur_ms), 0) AS BIGINT)
         FROM sess_agg"
    );
    let (visitors, pv, visits, bounced, dur): (i64, i64, i64, i64, i64) = conn
        .query_row(&sql, params_from_iter(params.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(map_duck_err)?;
    Ok(build_kpi_summary(
        non_neg_u64(visitors),
        non_neg_u64(pv),
        non_neg_u64(visits),
        non_neg_u64(bounced),
        non_neg_u64(dur),
    ))
}
