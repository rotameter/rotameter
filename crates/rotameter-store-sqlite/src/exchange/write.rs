#![forbid(unsafe_code)]
//! Chunked analytics writers for `SQLite` restore and cross-store migration.

use rotameter_core::error::CoreError;
use rotameter_core::privacy::utc_yyyymmdd;
use rotameter_core::storage::{
    IngestEvent, RollupDailyRow, RollupHourlyRow, RollupPathDailyRow, SessionUpsert,
};
use rotameter_core::types::DimensionKind;
use rusqlite::{TransactionBehavior, params};

use crate::config::map_sql_err;
use crate::ingest::writer::{write_event, write_session};
use crate::store::WriterState;

pub(crate) fn import_events_chunk(
    state: &mut WriterState,
    rows: &[IngestEvent],
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let tx = state
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    for ev in rows {
        let _ = write_event(&tx, &mut state.interner, ev)?;
    }
    tx.commit().map_err(map_sql_err)
}

pub(crate) fn import_sessions_chunk(
    state: &mut WriterState,
    rows: &[SessionUpsert],
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let tx = state
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    {
        let mut seen_stmt = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO seen_visitors_daily (site_id, day_utc, visitor_id)
                 VALUES (?1, ?2, ?3)",
            )
            .map_err(map_sql_err)?;
        for sess in rows {
            write_session(&tx, &mut state.interner, sess)?;
            let day_utc = i64::from(utc_yyyymmdd(sess.started_at_ms));
            seen_stmt
                .execute(params![
                    sess.site_id.as_bytes().as_slice(),
                    day_utc,
                    sess.visitor_id.as_slice(),
                ])
                .map_err(map_sql_err)?;
        }
    }
    tx.commit().map_err(map_sql_err)
}

pub(crate) fn import_rollups_hourly_chunk(
    state: &mut WriterState,
    rows: &[RollupHourlyRow],
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let tx = state
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO rollups_hourly (
                   site_id, bucket_ts_ms, pageviews, events_total,
                   bounced_visits, visits_started, total_duration_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(map_sql_err)?;
        for r in rows {
            stmt.execute(params![
                r.site_id.as_bytes().as_slice(),
                r.bucket_ts_ms,
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
            ])
            .map_err(map_sql_err)?;
        }
    }
    tx.commit().map_err(map_sql_err)
}

pub(crate) fn import_rollups_daily_chunk(
    state: &mut WriterState,
    rows: &[RollupDailyRow],
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let tx = state
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO rollups_daily (
                   site_id, day_utc, pageviews, events_total,
                   bounced_visits, visits_started, total_duration_ms, visitors
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .map_err(map_sql_err)?;
        for r in rows {
            stmt.execute(params![
                r.site_id.as_bytes().as_slice(),
                i64::from(r.day_utc),
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
                r.visitors,
            ])
            .map_err(map_sql_err)?;
        }
    }
    tx.commit().map_err(map_sql_err)
}

pub(crate) fn import_rollups_path_daily_chunk(
    state: &mut WriterState,
    rows: &[RollupPathDailyRow],
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Ok(());
    }
    let tx = state
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    {
        let mut stmt = tx
            .prepare_cached(
                "INSERT OR REPLACE INTO rollups_path_daily (
                   site_id, day_utc, path_id, pageviews, events_total,
                   visits, visitors, bounced_visits, total_duration_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .map_err(map_sql_err)?;
        for r in rows {
            let path_id = state
                .interner
                .intern(&tx, r.site_id, DimensionKind::Path, &r.path)?;
            stmt.execute(params![
                r.site_id.as_bytes().as_slice(),
                i64::from(r.day_utc),
                path_id,
                r.pageviews,
                r.events_total,
                r.visits,
                r.visitors,
                r.bounced_visits,
                r.total_duration_ms,
            ])
            .map_err(map_sql_err)?;
        }
    }
    tx.commit().map_err(map_sql_err)
}
