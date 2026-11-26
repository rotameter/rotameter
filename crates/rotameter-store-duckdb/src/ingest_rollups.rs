#![forbid(unsafe_code)]
//! Hourly and daily `DuckDB` rollup upsert helpers for [`DuckDbStore`] ingest.

use duckdb::{Connection, params};
use rotameter_core::error::CoreError;
use rotameter_core::storage::RollupDelta;

use crate::store::map_duck_err;

pub(crate) fn upsert_rollup_delta(conn: &Connection, r: &RollupDelta) -> Result<(), CoreError> {
    let site_blob = r.site_id.as_bytes().as_slice();
    let updated_h = conn
        .execute(
            "UPDATE rollups_hourly
             SET pageviews = pageviews + ?3,
                 events_total = events_total + ?4,
                 bounced_visits = GREATEST(0, bounced_visits + ?5),
                 visits_started = visits_started + ?6,
                 total_duration_ms = total_duration_ms + ?7
             WHERE site_id = ?1 AND bucket_ts_ms = ?2",
            params![
                site_blob,
                r.hour_bucket_ms,
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
            ],
        )
        .map_err(map_duck_err)?;
    if updated_h == 0 {
        conn.execute(
            "INSERT INTO rollups_hourly (
               site_id, bucket_ts_ms, pageviews, events_total,
               bounced_visits, visits_started, total_duration_ms
             ) VALUES (?1, ?2, ?3, ?4, GREATEST(0, ?5), ?6, ?7)",
            params![
                site_blob,
                r.hour_bucket_ms,
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
            ],
        )
        .map_err(map_duck_err)?;
    }

    let updated_d = conn
        .execute(
            "UPDATE rollups_daily
             SET pageviews = pageviews + ?3,
                 events_total = events_total + ?4,
                 bounced_visits = GREATEST(0, bounced_visits + ?5),
                 visits_started = visits_started + ?6,
                 total_duration_ms = total_duration_ms + ?7
             WHERE site_id = ?1 AND day_utc = ?2",
            params![
                site_blob,
                r.day_utc,
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
            ],
        )
        .map_err(map_duck_err)?;
    if updated_d == 0 {
        conn.execute(
            "INSERT INTO rollups_daily (
               site_id, day_utc, pageviews, events_total,
               bounced_visits, visits_started, total_duration_ms, visitors
             ) VALUES (?1, ?2, ?3, ?4, GREATEST(0, ?5), ?6, ?7, 0)",
            params![
                site_blob,
                r.day_utc,
                r.pageviews,
                r.events_total,
                r.bounced_visits,
                r.visits_started,
                r.total_duration_ms,
            ],
        )
        .map_err(map_duck_err)?;
    }
    Ok(())
}
