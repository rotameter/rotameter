#![forbid(unsafe_code)]
//! Chunked `events` and rollup readers for portable Arrow/Parquet export from [`DuckDbStore`].

use std::collections::BTreeMap;

use duckdb::{Error as DuckError, Row, params, types::Type};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::storage::{IngestEvent, RollupDailyRow, RollupHourlyRow, RollupPathDailyRow};
use rotameter_core::types::{EventKind, EventProps};

use crate::store::{DuckDbStore, map_duck_err};

impl DuckDbStore {
    /// Reads an ordered slice of raw `events` rows (`[offset, offset + limit)`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_events_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<IngestEvent>, CoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT site_id, ts_ms, session_id, CAST(seq_in_session AS BIGINT),
                            visitor_id, event_kind, path, props_json
                     FROM events
                     ORDER BY site_id, ts_ms, session_id, seq_in_session
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_duck_err)?;
            let rows = stmt
                .query_map(params![i64_lim(limit), i64_lim(offset)], decode_event)
                .map_err(map_duck_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_duck_err)
        })
    }

    /// Reads aggregated `rollups_hourly` rows ordered by `(site_id, bucket_ts_ms)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_hourly_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupHourlyRow>, CoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT site_id, bucket_ts_ms,
                            CAST(COALESCE(SUM(pageviews), 0) AS BIGINT),
                            CAST(COALESCE(SUM(events_total), 0) AS BIGINT),
                            CAST(COALESCE(SUM(bounced_visits), 0) AS BIGINT),
                            CAST(COALESCE(SUM(visits_started), 0) AS BIGINT),
                            CAST(COALESCE(SUM(total_duration_ms), 0) AS BIGINT)
                     FROM rollups_hourly
                     GROUP BY site_id, bucket_ts_ms
                     ORDER BY site_id, bucket_ts_ms
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_duck_err)?;
            let rows = stmt
                .query_map(params![i64_lim(limit), i64_lim(offset)], |r| {
                    Ok(RollupHourlyRow {
                        site_id: SiteId::from_bytes(blob_col::<16>(r, 0)?),
                        bucket_ts_ms: r.get(1)?,
                        pageviews: r.get(2)?,
                        events_total: r.get(3)?,
                        bounced_visits: r.get(4)?,
                        visits_started: r.get(5)?,
                        total_duration_ms: r.get(6)?,
                    })
                })
                .map_err(map_duck_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_duck_err)
        })
    }

    /// Reads aggregated `rollups_daily` rows ordered by `(site_id, day_utc)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_daily_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupDailyRow>, CoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT site_id, CAST(day_utc AS BIGINT),
                            CAST(COALESCE(SUM(pageviews), 0) AS BIGINT),
                            CAST(COALESCE(SUM(events_total), 0) AS BIGINT),
                            CAST(COALESCE(SUM(bounced_visits), 0) AS BIGINT),
                            CAST(COALESCE(SUM(visits_started), 0) AS BIGINT),
                            CAST(COALESCE(SUM(total_duration_ms), 0) AS BIGINT),
                            CAST(COALESCE(MAX(visitors), 0) AS BIGINT)
                     FROM rollups_daily
                     GROUP BY site_id, day_utc
                     ORDER BY site_id, day_utc
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_duck_err)?;
            let rows = stmt
                .query_map(params![i64_lim(limit), i64_lim(offset)], |r| {
                    let day_raw: i64 = r.get(1)?;
                    Ok(RollupDailyRow {
                        site_id: SiteId::from_bytes(blob_col::<16>(r, 0)?),
                        day_utc: i32::try_from(day_raw).map_err(|_| bad_col(1))?,
                        pageviews: r.get(2)?,
                        events_total: r.get(3)?,
                        bounced_visits: r.get(4)?,
                        visits_started: r.get(5)?,
                        total_duration_ms: r.get(6)?,
                        visitors: r.get(7)?,
                    })
                })
                .map_err(map_duck_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_duck_err)
        })
    }

    /// Reads aggregated `rollups_path_daily` rows ordered by `(site_id, day_utc, path)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_rollups_path_daily_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<RollupPathDailyRow>, CoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT site_id, CAST(day_utc AS BIGINT), path,
                            CAST(COALESCE(SUM(pageviews), 0) AS BIGINT),
                            CAST(COALESCE(SUM(events_total), 0) AS BIGINT),
                            CAST(COALESCE(SUM(visits), 0) AS BIGINT),
                            CAST(COALESCE(MAX(visitors), 0) AS BIGINT),
                            CAST(COALESCE(SUM(bounced_visits), 0) AS BIGINT),
                            CAST(COALESCE(SUM(total_duration_ms), 0) AS BIGINT)
                     FROM rollups_path_daily
                     GROUP BY site_id, day_utc, path
                     ORDER BY site_id, day_utc, path
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_duck_err)?;
            let rows = stmt
                .query_map(params![i64_lim(limit), i64_lim(offset)], |r| {
                    let day_raw: i64 = r.get(1)?;
                    Ok(RollupPathDailyRow {
                        site_id: SiteId::from_bytes(blob_col::<16>(r, 0)?),
                        day_utc: i32::try_from(day_raw).map_err(|_| bad_col(1))?,
                        path: r.get(2)?,
                        pageviews: r.get(3)?,
                        events_total: r.get(4)?,
                        visits: r.get(5)?,
                        visitors: r.get(6)?,
                        bounced_visits: r.get(7)?,
                        total_duration_ms: r.get(8)?,
                    })
                })
                .map_err(map_duck_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_duck_err)
        })
    }
}

fn decode_event(r: &Row<'_>) -> duckdb::Result<IngestEvent> {
    let seq_raw: i64 = r.get(3)?;
    let kind_raw: String = r.get(5)?;
    let event_kind = EventKind::parse(&kind_raw).map_err(|_| bad_col(5))?;
    let props_raw: Option<String> = r.get(7)?;
    let props = match props_raw.as_deref() {
        None => EventProps::default(),
        Some(raw) => {
            let map: BTreeMap<String, String> =
                serde_json::from_str(raw).map_err(|_| bad_col(7))?;
            EventProps::from_pairs(map).map_err(|_| bad_col(7))?
        }
    };
    Ok(IngestEvent {
        site_id: SiteId::from_bytes(blob_col::<16>(r, 0)?),
        ts_ms: r.get(1)?,
        session_id: blob_col::<16>(r, 2)?,
        seq_in_session: u32::try_from(seq_raw.max(0)).map_err(|_| bad_col(3))?,
        visitor_id: blob_col::<16>(r, 4)?,
        event_kind,
        path: r.get(6)?,
        props,
    })
}

pub(crate) fn blob_col<const N: usize>(row: &Row<'_>, col: usize) -> duckdb::Result<[u8; N]> {
    let bytes: Vec<u8> = row.get(col)?;
    bytes
        .try_into()
        .map_err(|_| DuckError::InvalidColumnType(col, format!("blob[{N}]"), Type::Blob))
}

pub(crate) fn bad_col(col: usize) -> DuckError {
    DuckError::InvalidColumnType(col, format!("col_{col}"), Type::Any)
}

pub(crate) fn i64_lim(val: usize) -> i64 {
    i64::try_from(val).unwrap_or(i64::MAX)
}
