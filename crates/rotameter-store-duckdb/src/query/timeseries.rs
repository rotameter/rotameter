#![forbid(unsafe_code)]
//! Time-series bucketing in site `IANA` timezones for `DuckDB` sessions and events.

use std::collections::{BTreeMap, HashSet};

use chrono_tz::Tz;
use duckdb::{Connection, params_from_iter, types::Value};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{Granularity, QueryFilter};
use rotameter_core::storage::TimeseriesPoint;
use rotameter_core::types::Site;

use crate::query::filters::build_filter_plan;
use crate::query::rollup_summary::{raw_session_hour_floor, rollup_cutoffs};
use crate::query::summary::non_neg_u64;
use crate::query::window::{floor_bucket_ms, utc_day_start_ms};
use crate::store::map_duck_err;

#[derive(Default)]
struct BucketAcc {
    visitors: HashSet<Vec<u8>>,
    sessions: HashSet<Vec<u8>>,
    pageviews: u64,
    visits: u64,
}

impl BucketAcc {
    fn into_point(self, bucket_ms: i64, count_distinct_sessions: bool) -> TimeseriesPoint {
        let visits = if count_distinct_sessions {
            u64::try_from(self.sessions.len()).unwrap_or(u64::MAX)
        } else {
            self.visits
        };
        TimeseriesPoint {
            bucket_ms,
            visitors: u64::try_from(self.visitors.len()).unwrap_or(u64::MAX),
            pageviews: self.pageviews,
            visits,
        }
    }
}

pub(crate) fn query_timeseries(
    conn: &Connection,
    site: &Site,
    from_ms: i64,
    to_ms: i64,
    granularity: Granularity,
    tz: Tz,
    filters: &[QueryFilter],
) -> Result<Vec<TimeseriesPoint>, CoreError> {
    let mut params = vec![
        Value::Blob(site.site_id.as_bytes().to_vec()),
        Value::BigInt(from_ms),
        Value::BigInt(to_ms),
    ];
    let plan = build_filter_plan(filters, &[], &mut params)?;
    let where_tail = plan.where_sql();

    let mut buckets: BTreeMap<i64, BucketAcc> = BTreeMap::new();
    if plan.needs_events {
        fold_event_rows(conn, granularity, tz, &where_tail, &params, &mut buckets)?;
        return Ok(buckets
            .into_iter()
            .map(|(ms, acc)| acc.into_point(ms, true))
            .collect());
    }

    if filters.is_empty() {
        let (raw_floor, has_live) = raw_session_hour_floor(conn, site.site_id, to_ms)?;
        if from_ms < raw_floor {
            fold_historical_buckets(
                conn,
                site.site_id,
                from_ms,
                raw_floor.min(to_ms),
                raw_floor,
                has_live,
                granularity,
                tz,
                &mut buckets,
            )?;
        }
        let live_start = from_ms.max(raw_floor);
        if live_start < to_ms {
            let live_params = [
                Value::Blob(site.site_id.as_bytes().to_vec()),
                Value::BigInt(live_start),
                Value::BigInt(to_ms),
            ];
            fold_session_rows(conn, granularity, tz, "", &live_params, &mut buckets)?;
        }
    } else {
        fold_session_rows(conn, granularity, tz, &where_tail, &params, &mut buckets)?;
    }

    Ok(buckets
        .into_iter()
        .map(|(ms, acc)| acc.into_point(ms, false))
        .collect())
}

fn fold_session_rows(
    conn: &Connection,
    granularity: Granularity,
    tz: Tz,
    where_tail: &str,
    params: &[Value],
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let sql = format!(
        "SELECT s.started_at_ms, s.visitor_id, CAST(s.pageview_count AS BIGINT)
         FROM sessions s
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_duck_err)?;
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_duck_err)?;
    while let Some(row) = rows.next().map_err(map_duck_err)? {
        let started_at_ms: i64 = row.get(0).map_err(map_duck_err)?;
        let visitor_id: Vec<u8> = row.get(1).map_err(map_duck_err)?;
        let pageview_count: i64 = row.get(2).map_err(map_duck_err)?;
        let bucket_ms = floor_bucket_ms(started_at_ms, granularity, tz);
        let acc = buckets.entry(bucket_ms).or_default();
        acc.visitors.insert(visitor_id);
        acc.pageviews += non_neg_u64(pageview_count);
        acc.visits += 1;
    }
    Ok(())
}

fn fold_event_rows(
    conn: &Connection,
    granularity: Granularity,
    tz: Tz,
    where_tail: &str,
    params: &[Value],
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let sql = format!(
        "SELECT e.ts_ms, e.visitor_id, e.session_id,
                CASE WHEN e.event_kind = 'pageview' THEN 1 ELSE 0 END
         FROM events e
         JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
         WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_duck_err)?;
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_duck_err)?;
    while let Some(row) = rows.next().map_err(map_duck_err)? {
        let ts_ms: i64 = row.get(0).map_err(map_duck_err)?;
        let visitor_id: Vec<u8> = row.get(1).map_err(map_duck_err)?;
        let session_id: Vec<u8> = row.get(2).map_err(map_duck_err)?;
        let is_pv: i64 = row.get(3).map_err(map_duck_err)?;
        let bucket_ms = floor_bucket_ms(ts_ms, granularity, tz);
        let acc = buckets.entry(bucket_ms).or_default();
        acc.visitors.insert(visitor_id);
        acc.sessions.insert(session_id);
        acc.pageviews += non_neg_u64(is_pv);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn fold_historical_buckets(
    conn: &Connection,
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
    raw_floor: i64,
    has_live: bool,
    granularity: Granularity,
    tz: Tz,
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let site_blob = site_id.as_bytes().as_slice();
    let (min_hourly, exclude_day) = rollup_cutoffs(conn, site_id, raw_floor, has_live)?;

    if let Some(min_h) = min_hourly
        && to_ms > min_h
    {
        let h_start = from_ms.max(min_h).div_euclid(MS_PER_HOUR) * MS_PER_HOUR;
        let mut stmt = conn
            .prepare(
                "SELECT bucket_ts_ms, pageviews, visits_started
                 FROM rollups_hourly
                 WHERE site_id = ?1 AND bucket_ts_ms >= ?2 AND bucket_ts_ms < ?3",
            )
            .map_err(map_duck_err)?;
        let mut rows = stmt
            .query(duckdb::params![site_blob, h_start, to_ms])
            .map_err(map_duck_err)?;
        while let Some(row) = rows.next().map_err(map_duck_err)? {
            let bucket_ts: i64 = row.get(0).map_err(map_duck_err)?;
            let pv: i64 = row.get(1).map_err(map_duck_err)?;
            let vs: i64 = row.get(2).map_err(map_duck_err)?;
            let key = floor_bucket_ms(bucket_ts, granularity, tz);
            let acc = buckets.entry(key).or_default();
            acc.pageviews += non_neg_u64(pv);
            acc.visits += non_neg_u64(vs);
        }
    }

    let daily_end = min_hourly.map_or(to_ms, |h| to_ms.min(h));
    if from_ms < daily_end {
        let day_from = utc_yyyymmdd(from_ms);
        let day_to = utc_yyyymmdd(daily_end.saturating_sub(1).max(from_ms));
        let mut stmt = conn
            .prepare(
                "SELECT day_utc, pageviews, visits_started
                 FROM rollups_daily
                 WHERE site_id = ?1 AND day_utc >= ?2 AND day_utc <= ?3
                   AND (?4 IS NULL OR day_utc < ?4)",
            )
            .map_err(map_duck_err)?;
        let mut rows = stmt
            .query(duckdb::params![site_blob, day_from, day_to, exclude_day])
            .map_err(map_duck_err)?;
        while let Some(row) = rows.next().map_err(map_duck_err)? {
            let day_utc: i32 = row.get(0).map_err(map_duck_err)?;
            let pv: i64 = row.get(1).map_err(map_duck_err)?;
            let vs: i64 = row.get(2).map_err(map_duck_err)?;
            let ts = utc_day_start_ms(day_utc).unwrap_or(from_ms);
            let key = floor_bucket_ms(ts, granularity, tz);
            let acc = buckets.entry(key).or_default();
            acc.pageviews += non_neg_u64(pv);
            acc.visits += non_neg_u64(vs);
        }
    }
    Ok(())
}
