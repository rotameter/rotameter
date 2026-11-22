#![forbid(unsafe_code)]
//! Fast-path multi-day summary and time-series readers backed by `rollups_daily`.

use chrono_tz::Tz;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_DAY, utc_yyyymmdd};
use rotameter_core::query::Granularity;
use rotameter_core::storage::{KpiSummary, TimeseriesPoint};
use rotameter_core::types::{SaltRotation, Site};
use rusqlite::{Connection, OptionalExtension, params};

use crate::config::map_sql_err;
use crate::query::summary::{build_kpi_summary, non_neg_u64};
use crate::query::window::utc_day_start_ms;

pub(crate) fn daily_window_bounds(from_ms: i64, to_ms: i64) -> Option<(i32, i32, i64, i64)> {
    if to_ms.saturating_sub(from_ms) < 2 * MS_PER_DAY {
        return None;
    }
    let day_from = utc_yyyymmdd(from_ms);
    let day_to = utc_yyyymmdd(to_ms.saturating_sub(1));
    let day_from_start = utc_day_start_ms(day_from)?;
    let day_to_end = utc_day_start_ms(day_to)?.checked_add(MS_PER_DAY)?;
    Some((day_from, day_to, day_from_start, day_to_end))
}

fn has_row_in_range(
    conn: &Connection,
    sql: &str,
    site_blob: &[u8],
    start: i64,
    end: i64,
) -> Result<bool, CoreError> {
    if start >= end {
        return Ok(false);
    }
    conn.query_row(sql, params![site_blob, start, end], |_| Ok(()))
        .optional()
        .map_err(map_sql_err)
        .map(|row| row.is_some())
}

fn sessions_edge_clean(
    conn: &Connection,
    site_id: SiteId,
    day_from_start: i64,
    from_ms: i64,
    to_ms: i64,
    day_to_end: i64,
) -> Result<bool, CoreError> {
    let sql = "SELECT 1 FROM sessions WHERE site_id = ?1 AND started_at_ms >= ?2 AND started_at_ms < ?3 LIMIT 1";
    let blob = site_id.as_bytes().as_slice();
    Ok(!has_row_in_range(conn, sql, blob, day_from_start, from_ms)?
        && !has_row_in_range(conn, sql, blob, to_ms, day_to_end)?)
}

pub(crate) fn events_edge_clean(
    conn: &Connection,
    site_id: SiteId,
    day_from_start: i64,
    from_ms: i64,
    to_ms: i64,
    day_to_end: i64,
) -> Result<bool, CoreError> {
    let sql = "SELECT 1 FROM events WHERE site_id = ?1 AND ts_ms >= ?2 AND ts_ms < ?3 LIMIT 1";
    let blob = site_id.as_bytes().as_slice();
    Ok(!has_row_in_range(conn, sql, blob, day_from_start, from_ms)?
        && !has_row_in_range(conn, sql, blob, to_ms, day_to_end)?)
}

pub(crate) fn try_daily_summary(
    conn: &Connection,
    site: &Site,
    from_ms: i64,
    to_ms: i64,
) -> Result<Option<KpiSummary>, CoreError> {
    if site.salt_rotation != SaltRotation::Daily {
        return Ok(None);
    }
    let Some((day_from, day_to, day_from_start, day_to_end)) = daily_window_bounds(from_ms, to_ms)
    else {
        return Ok(None);
    };
    if !sessions_edge_clean(
        conn,
        site.site_id,
        day_from_start,
        from_ms,
        to_ms,
        day_to_end,
    )? {
        return Ok(None);
    }
    let (vis, pv, vs, bv, dur): (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(visitors), 0), COALESCE(SUM(pageviews), 0),
                    COALESCE(SUM(visits_started), 0), COALESCE(SUM(bounced_visits), 0),
                    COALESCE(SUM(total_duration_ms), 0)
             FROM rollups_daily
             WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3",
            params![site.site_id.as_bytes().as_slice(), day_from, day_to],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(map_sql_err)?;
    if vs <= 0 || vis <= 0 {
        return Ok(None);
    }
    Ok(Some(build_kpi_summary(
        non_neg_u64(vis),
        non_neg_u64(pv),
        non_neg_u64(vs),
        non_neg_u64(bv),
        non_neg_u64(dur),
    )))
}

pub(crate) fn try_daily_timeseries(
    conn: &Connection,
    site: &Site,
    from_ms: i64,
    to_ms: i64,
    granularity: Granularity,
    tz: Tz,
) -> Result<Option<Vec<TimeseriesPoint>>, CoreError> {
    if site.salt_rotation != SaltRotation::Daily || granularity != Granularity::Day || tz != Tz::UTC
    {
        return Ok(None);
    }
    let Some((day_from, day_to, day_from_start, day_to_end)) = daily_window_bounds(from_ms, to_ms)
    else {
        return Ok(None);
    };
    if !sessions_edge_clean(
        conn,
        site.site_id,
        day_from_start,
        from_ms,
        to_ms,
        day_to_end,
    )? {
        return Ok(None);
    }
    let mut stmt = conn
        .prepare(
            "SELECT day_utc, visitors, pageviews, visits_started
             FROM rollups_daily
             WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3
             ORDER BY day_utc ASC",
        )
        .map_err(map_sql_err)?;
    let mut rows = stmt
        .query(params![
            site.site_id.as_bytes().as_slice(),
            day_from,
            day_to
        ])
        .map_err(map_sql_err)?;
    let mut points = Vec::new();
    let (mut total_visits, mut total_visitors) = (0_u64, 0_u64);
    while let Some(row) = rows.next().map_err(map_sql_err)? {
        let day_utc: i32 = row.get(0).map_err(map_sql_err)?;
        let Some(bucket_ms) = utc_day_start_ms(day_utc) else {
            continue;
        };
        let visitors = non_neg_u64(row.get(1).map_err(map_sql_err)?);
        let visits = non_neg_u64(row.get(3).map_err(map_sql_err)?);
        total_visitors += visitors;
        total_visits += visits;
        points.push(TimeseriesPoint {
            bucket_ms,
            visitors,
            pageviews: non_neg_u64(row.get(2).map_err(map_sql_err)?),
            visits,
        });
    }
    if points.is_empty() || total_visits == 0 || total_visitors == 0 {
        return Ok(None);
    }
    Ok(Some(points))
}
