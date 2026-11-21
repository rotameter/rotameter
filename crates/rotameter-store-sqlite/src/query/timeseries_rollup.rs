#![forbid(unsafe_code)]
//! Historical `rollups_hourly` / `rollups_daily` folding for post-retention time series.

use std::collections::BTreeMap;

use chrono_tz::Tz;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::Granularity;
use rusqlite::{Connection, params};

use crate::config::map_sql_err;
use crate::query::rollup_summary::rollup_cutoffs;
use crate::query::summary::non_neg_u64;
use crate::query::timeseries::BucketAcc;
use crate::query::window::{floor_bucket_ms, utc_day_start_ms};

#[derive(Clone, Copy)]
pub(crate) struct RollupBucketQuery {
    pub(crate) site_id: SiteId,
    pub(crate) from_ms: i64,
    pub(crate) to_ms: i64,
    pub(crate) raw_floor: i64,
    pub(crate) has_live_sessions: bool,
    pub(crate) granularity: Granularity,
    pub(crate) tz: Tz,
}

pub(crate) fn fold_historical_buckets(
    conn: &Connection,
    req: RollupBucketQuery,
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let site_blob = req.site_id.as_bytes().as_slice();
    let (min_hourly, exclude_day) =
        rollup_cutoffs(conn, req.site_id, req.raw_floor, req.has_live_sessions)?;

    if let Some(min_h) = min_hourly
        && req.to_ms > min_h
    {
        let h_start = req.from_ms.max(min_h).div_euclid(MS_PER_HOUR) * MS_PER_HOUR;
        let mut stmt = conn
            .prepare(
                "SELECT bucket_ts_ms, pageviews, visits_started
                 FROM rollups_hourly
                 WHERE site_id = ?1 AND bucket_ts_ms >= ?2 AND bucket_ts_ms < ?3",
            )
            .map_err(map_sql_err)?;
        let mut rows = stmt
            .query(params![site_blob, h_start, req.to_ms])
            .map_err(map_sql_err)?;
        while let Some(row) = rows.next().map_err(map_sql_err)? {
            let ts: i64 = row.get(0).map_err(map_sql_err)?;
            let pv: i64 = row.get(1).map_err(map_sql_err)?;
            let vs: i64 = row.get(2).map_err(map_sql_err)?;
            let bucket_ms = floor_bucket_ms(ts, req.granularity, req.tz);
            let acc = buckets.entry(bucket_ms).or_default();
            acc.pageviews += non_neg_u64(pv);
            acc.visits += non_neg_u64(vs);
        }
    }

    let daily_end = min_hourly.map_or(req.to_ms, |h| req.to_ms.min(h));
    if req.from_ms < daily_end {
        let day_from = utc_yyyymmdd(req.from_ms);
        let day_to = utc_yyyymmdd(daily_end.saturating_sub(1).max(req.from_ms));
        let mut stmt = conn
            .prepare(
                "SELECT day_utc, pageviews, visits_started
                 FROM rollups_daily
                 WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3
                   AND (?4 IS NULL OR day_utc < ?4)",
            )
            .map_err(map_sql_err)?;
        let mut rows = stmt
            .query(params![site_blob, day_from, day_to, exclude_day])
            .map_err(map_sql_err)?;
        while let Some(row) = rows.next().map_err(map_sql_err)? {
            let day_utc: i32 = row.get(0).map_err(map_sql_err)?;
            let pv: i64 = row.get(1).map_err(map_sql_err)?;
            let vs: i64 = row.get(2).map_err(map_sql_err)?;
            if let Some(day_ms) = utc_day_start_ms(day_utc) {
                let bucket_ms = floor_bucket_ms(day_ms, req.granularity, req.tz);
                let acc = buckets.entry(bucket_ms).or_default();
                acc.pageviews += non_neg_u64(pv);
                acc.visits += non_neg_u64(vs);
            }
        }
    }
    Ok(())
}
