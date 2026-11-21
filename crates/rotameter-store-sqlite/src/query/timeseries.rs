#![forbid(unsafe_code)]
//! Time-series bucketing in site `IANA` timezones across sessions and events.

use std::collections::{BTreeMap, HashSet};

use chrono_tz::Tz;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::query::{Granularity, QueryFilter};
use rotameter_core::storage::TimeseriesPoint;
use rusqlite::{Connection, params_from_iter, types::Value};

use crate::config::map_sql_err;
use crate::query::filter_sql::build_filter_plan;
use crate::query::summary::{non_neg_u64, raw_session_hour_floor};
use crate::query::timeseries_rollup::{RollupBucketQuery, fold_historical_buckets};
use crate::query::window::floor_bucket_ms;

#[derive(Default)]
pub(crate) struct BucketAcc {
    pub(crate) visitors: HashSet<Vec<u8>>,
    pub(crate) sessions: HashSet<Vec<u8>>,
    pub(crate) pageviews: u64,
    pub(crate) visits: u64,
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
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
    granularity: Granularity,
    tz: Tz,
    filters: &[QueryFilter],
) -> Result<Vec<TimeseriesPoint>, CoreError> {
    let mut params = vec![
        Value::Blob(site_id.as_bytes().to_vec()),
        Value::Integer(from_ms),
        Value::Integer(to_ms),
    ];
    let plan = build_filter_plan(filters, &[], &mut params)?;
    let joins = plan.join_sql();
    let where_tail = plan.where_sql();

    let mut buckets: BTreeMap<i64, BucketAcc> = BTreeMap::new();
    if plan.needs_events {
        fold_event_rows(
            conn,
            granularity,
            tz,
            &joins,
            &where_tail,
            &params,
            &mut buckets,
        )?;
        return Ok(buckets
            .into_iter()
            .map(|(ms, acc)| acc.into_point(ms, true))
            .collect());
    }

    if filters.is_empty() {
        let (raw_floor, has_live) = raw_session_hour_floor(conn, site_id, to_ms)?;
        if from_ms < raw_floor {
            fold_historical_buckets(
                conn,
                RollupBucketQuery {
                    site_id,
                    from_ms,
                    to_ms: raw_floor.min(to_ms),
                    raw_floor,
                    has_live_sessions: has_live,
                    granularity,
                    tz,
                },
                &mut buckets,
            )?;
        }
        let live_start = from_ms.max(raw_floor);
        if live_start < to_ms {
            let live_params = [
                Value::Blob(site_id.as_bytes().to_vec()),
                Value::Integer(live_start),
                Value::Integer(to_ms),
            ];
            fold_session_rows(conn, granularity, tz, "", "", &live_params, &mut buckets)?;
        }
    } else {
        fold_session_rows(
            conn,
            granularity,
            tz,
            &joins,
            &where_tail,
            &params,
            &mut buckets,
        )?;
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
    joins: &str,
    where_tail: &str,
    params: &[Value],
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let sql = format!(
        "SELECT s.started_at_ms, s.visitor_id, s.pageview_count
         FROM sessions s {joins}
         WHERE s.site_id = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms < ?3 {where_tail}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sql_err)?;
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_sql_err)?;
    while let Some(row) = rows.next().map_err(map_sql_err)? {
        let started_at_ms: i64 = row.get(0).map_err(map_sql_err)?;
        let visitor_id: Vec<u8> = row.get(1).map_err(map_sql_err)?;
        let pageview_count: i64 = row.get(2).map_err(map_sql_err)?;
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
    joins: &str,
    where_tail: &str,
    params: &[Value],
    buckets: &mut BTreeMap<i64, BucketAcc>,
) -> Result<(), CoreError> {
    let sql = format!(
        "SELECT e.ts_ms, e.visitor_id, e.session_id,
                CASE WHEN en_pv.value = 'pageview' THEN 1 ELSE 0 END
         FROM events e
         JOIN sessions s ON s.site_id = e.site_id AND s.session_id = e.session_id
         JOIN dim_strings en_pv ON en_pv.id = e.event_name_id
         {joins}
         WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3 {where_tail}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sql_err)?;
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(map_sql_err)?;
    while let Some(row) = rows.next().map_err(map_sql_err)? {
        let ts_ms: i64 = row.get(0).map_err(map_sql_err)?;
        let visitor_id: Vec<u8> = row.get(1).map_err(map_sql_err)?;
        let session_id: Vec<u8> = row.get(2).map_err(map_sql_err)?;
        let is_pv: i64 = row.get(3).map_err(map_sql_err)?;
        let bucket_ms = floor_bucket_ms(ts_ms, granularity, tz);
        let acc = buckets.entry(bucket_ms).or_default();
        acc.visitors.insert(visitor_id);
        acc.sessions.insert(session_id);
        if is_pv > 0 {
            acc.pageviews += 1;
        }
    }
    Ok(())
}
