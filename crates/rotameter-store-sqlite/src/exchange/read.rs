#![forbid(unsafe_code)]
//! Chunked analytics readers for `events` and rollup tables (`SQLite` exchange).

use std::collections::BTreeMap;

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::storage::{IngestEvent, RollupDailyRow, RollupHourlyRow, RollupPathDailyRow};
use rotameter_core::types::{EventKind, EventProps};
use rusqlite::{Connection, Error as SqlError, Row, params, types::Type};

use crate::config::map_sql_err;

pub(crate) fn read_events_chunk(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<IngestEvent>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT e.site_id, e.ts_ms, e.session_id, e.seq_in_session, e.visitor_id,
                    en.value, dp.value, e.props_json
             FROM events e
             JOIN dim_strings en ON en.id = e.event_name_id
             JOIN dim_strings dp ON dp.id = e.path_id
             ORDER BY e.site_id, e.ts_ms, e.session_id, e.seq_in_session
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(map_sql_err)?;
    stmt.query_map(params![i64_limit(limit), i64_limit(offset)], decode_event)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)
}

pub(crate) fn read_rollups_hourly_chunk(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<RollupHourlyRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT site_id, bucket_ts_ms, pageviews, events_total,
                    bounced_visits, visits_started, total_duration_ms
             FROM rollups_hourly
             ORDER BY site_id, bucket_ts_ms
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(map_sql_err)?;
    stmt.query_map(params![i64_limit(limit), i64_limit(offset)], |row| {
        Ok(RollupHourlyRow {
            site_id: SiteId::from_bytes(blob_col::<16>(row, 0)?),
            bucket_ts_ms: row.get(1)?,
            pageviews: row.get(2)?,
            events_total: row.get(3)?,
            bounced_visits: row.get(4)?,
            visits_started: row.get(5)?,
            total_duration_ms: row.get(6)?,
        })
    })
    .map_err(map_sql_err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(map_sql_err)
}

pub(crate) fn read_rollups_daily_chunk(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<RollupDailyRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT site_id, day_utc, pageviews, events_total,
                    bounced_visits, visits_started, total_duration_ms, visitors
             FROM rollups_daily
             ORDER BY site_id, day_utc
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(map_sql_err)?;
    stmt.query_map(params![i64_limit(limit), i64_limit(offset)], |row| {
        let day_raw: i64 = row.get(1)?;
        Ok(RollupDailyRow {
            site_id: SiteId::from_bytes(blob_col::<16>(row, 0)?),
            day_utc: i32::try_from(day_raw).map_err(|err| invalid_col(1, err))?,
            pageviews: row.get(2)?,
            events_total: row.get(3)?,
            bounced_visits: row.get(4)?,
            visits_started: row.get(5)?,
            total_duration_ms: row.get(6)?,
            visitors: row.get(7)?,
        })
    })
    .map_err(map_sql_err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(map_sql_err)
}

pub(crate) fn read_rollups_path_daily_chunk(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<RollupPathDailyRow>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT r.site_id, r.day_utc, dp.value, r.pageviews, r.events_total,
                    r.visits, r.visitors, r.bounced_visits, r.total_duration_ms
             FROM rollups_path_daily r
             JOIN dim_strings dp ON dp.id = r.path_id
             ORDER BY r.site_id, r.day_utc, r.path_id
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(map_sql_err)?;
    stmt.query_map(params![i64_limit(limit), i64_limit(offset)], |row| {
        let day_raw: i64 = row.get(1)?;
        Ok(RollupPathDailyRow {
            site_id: SiteId::from_bytes(blob_col::<16>(row, 0)?),
            day_utc: i32::try_from(day_raw).map_err(|err| invalid_col(1, err))?,
            path: row.get(2)?,
            pageviews: row.get(3)?,
            events_total: row.get(4)?,
            visits: row.get(5)?,
            visitors: row.get(6)?,
            bounced_visits: row.get(7)?,
            total_duration_ms: row.get(8)?,
        })
    })
    .map_err(map_sql_err)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(map_sql_err)
}

pub(crate) fn blob_col<const N: usize>(row: &Row<'_>, col: usize) -> rusqlite::Result<[u8; N]> {
    let bytes: Vec<u8> = row.get(col)?;
    bytes.try_into().map_err(|_| {
        SqlError::FromSqlConversionFailure(
            col,
            Type::Blob,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected {N}-byte BLOB"),
            )),
        )
    })
}

pub(crate) fn i64_limit(val: usize) -> i64 {
    i64::try_from(val).unwrap_or(i64::MAX)
}

pub(crate) fn invalid_col<E>(col: usize, err: E) -> SqlError
where
    E: std::error::Error + Send + Sync + 'static,
{
    SqlError::FromSqlConversionFailure(col, Type::Text, Box::new(err))
}

fn decode_event(row: &Row<'_>) -> rusqlite::Result<IngestEvent> {
    let seq_raw: i64 = row.get(3)?;
    let seq_in_session = u32::try_from(seq_raw.max(0)).map_err(|err| invalid_col(3, err))?;
    let kind_str: String = row.get(5)?;
    let event_kind = EventKind::parse(&kind_str).map_err(|err| invalid_col(5, err))?;
    let props_raw: Option<String> = row.get(7)?;
    let props = match props_raw.as_deref() {
        None => EventProps::default(),
        Some(raw) => {
            let map: BTreeMap<String, String> =
                serde_json::from_str(raw).map_err(|err| invalid_col(7, err))?;
            EventProps::from_pairs(map).map_err(|err| invalid_col(7, err))?
        }
    };
    Ok(IngestEvent {
        site_id: SiteId::from_bytes(blob_col::<16>(row, 0)?),
        ts_ms: row.get(1)?,
        session_id: blob_col::<16>(row, 2)?,
        seq_in_session,
        visitor_id: blob_col::<16>(row, 4)?,
        event_kind,
        path: row.get(6)?,
        props,
    })
}
