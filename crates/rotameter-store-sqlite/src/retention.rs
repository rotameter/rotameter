#![forbid(unsafe_code)]
//! Retention pruning (`raw_retention_days`) and full site purge before cutoff timestamps.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_DAY, utc_yyyymmdd};
use rusqlite::{Connection, Transaction, params};

use crate::config::map_sql_err;
use crate::store::SqliteStore;

impl SqliteStore {
    /// Deletes raw `events` and inactive `sessions` strictly older than `before_ms` for `site_id`,
    /// while preserving `rollups_hourly` and `rollups_daily`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if the transaction fails.
    pub fn prune_raw_before(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError> {
        self.with_writer(|state| {
            let tx = state.conn.transaction().map_err(map_sql_err)?;
            let deleted = delete_raw_before(&tx, site_id, before_ms)?;
            tx.commit().map_err(map_sql_err)?;
            Ok(deleted)
        })
    }

    /// Scans all sites with configured `raw_retention_days` and prunes expired raw rows relative
    /// to `now_ms`, preserving hourly and daily rollups.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if querying sites or committing deletions fails.
    pub fn prune_expired_sites(&self, now_ms: i64) -> Result<u64, CoreError> {
        self.with_writer(|state| prune_all_expired(&mut state.conn, now_ms))
    }
}

struct PurgedHourlyRow {
    day_utc: i32,
    pageviews: i64,
    events_total: i64,
    bounced_visits: i64,
    visits_started: i64,
    total_duration_ms: i64,
}

pub(crate) fn purge_site_before(
    conn: &mut Connection,
    site_id: SiteId,
    before_ms: i64,
) -> Result<u64, CoreError> {
    let tx = conn.transaction().map_err(map_sql_err)?;
    let site_blob = site_id.as_bytes().as_slice();
    let mut total = delete_raw_before(&tx, site_id, before_ms)?;
    let cutoff_day = utc_yyyymmdd(before_ms);

    let d_del = tx
        .execute(
            "DELETE FROM rollups_daily WHERE site_id = ?1 AND day_utc < ?2",
            params![site_blob, cutoff_day],
        )
        .map_err(map_sql_err)?;

    let hourly_buckets = collect_purged_hourly_buckets(&tx, site_blob, before_ms)?;
    for bucket in hourly_buckets {
        if bucket.day_utc == cutoff_day {
            tx.execute(
                "UPDATE rollups_daily
                 SET pageviews = MAX(0, pageviews - ?3),
                     events_total = MAX(0, events_total - ?4),
                     bounced_visits = MAX(0, bounced_visits - ?5),
                     visits_started = MAX(0, visits_started - ?6),
                     total_duration_ms = MAX(0, total_duration_ms - ?7)
                 WHERE site_id = ?1 AND day_utc = ?2",
                params![
                    site_blob,
                    bucket.day_utc,
                    bucket.pageviews,
                    bucket.events_total,
                    bucket.bounced_visits,
                    bucket.visits_started,
                    bucket.total_duration_ms
                ],
            )
            .map_err(map_sql_err)?;
        }
    }
    let zero_del = tx
        .execute(
            "DELETE FROM rollups_daily
             WHERE site_id = ?1
               AND pageviews = 0 AND events_total = 0
               AND bounced_visits = 0 AND visits_started = 0 AND total_duration_ms = 0",
            [site_blob],
        )
        .map_err(map_sql_err)?;

    let h_del = tx
        .execute(
            "DELETE FROM rollups_hourly WHERE site_id = ?1 AND bucket_ts_ms < ?2",
            params![site_blob, before_ms],
        )
        .map_err(map_sql_err)?;

    tx.commit().map_err(map_sql_err)?;
    total += u64::try_from(h_del + d_del + zero_del).unwrap_or(0);
    Ok(total)
}

fn delete_raw_before(
    tx: &Transaction<'_>,
    site_id: SiteId,
    before_ms: i64,
) -> Result<u64, CoreError> {
    let site_blob = site_id.as_bytes().as_slice();
    let ev_del = tx
        .execute(
            "DELETE FROM events WHERE site_id = ?1 AND ts_ms < ?2",
            params![site_blob, before_ms],
        )
        .map_err(map_sql_err)?;
    let sess_del = tx
        .execute(
            "DELETE FROM sessions WHERE site_id = ?1 AND last_event_ms < ?2",
            params![site_blob, before_ms],
        )
        .map_err(map_sql_err)?;
    Ok(u64::try_from(ev_del + sess_del).unwrap_or(0))
}

fn collect_purged_hourly_buckets(
    tx: &Transaction<'_>,
    site_blob: &[u8],
    before_ms: i64,
) -> Result<Vec<PurgedHourlyRow>, CoreError> {
    let mut stmt = tx
        .prepare(
            "SELECT bucket_ts_ms, pageviews, events_total, bounced_visits, visits_started, total_duration_ms
             FROM rollups_hourly
             WHERE site_id = ?1 AND bucket_ts_ms < ?2",
        )
        .map_err(map_sql_err)?;
    let mut rows = stmt
        .query(params![site_blob, before_ms])
        .map_err(map_sql_err)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(map_sql_err)? {
        let bucket_ts_ms: i64 = row.get(0).map_err(map_sql_err)?;
        out.push(PurgedHourlyRow {
            day_utc: utc_yyyymmdd(bucket_ts_ms),
            pageviews: row.get(1).map_err(map_sql_err)?,
            events_total: row.get(2).map_err(map_sql_err)?,
            bounced_visits: row.get(3).map_err(map_sql_err)?,
            visits_started: row.get(4).map_err(map_sql_err)?,
            total_duration_ms: row.get(5).map_err(map_sql_err)?,
        });
    }
    Ok(out)
}

fn prune_all_expired(conn: &mut Connection, now_ms: i64) -> Result<u64, CoreError> {
    let targets: Vec<(SiteId, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT site_id, raw_retention_days
                 FROM sites
                 WHERE raw_retention_days IS NOT NULL AND raw_retention_days > 0",
            )
            .map_err(map_sql_err)?;
        let mut rows = stmt.query([]).map_err(map_sql_err)?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().map_err(map_sql_err)? {
            let raw_id: Vec<u8> = row.get(0).map_err(map_sql_err)?;
            let days: i64 = row.get(1).map_err(map_sql_err)?;
            if let Ok(bytes) = <[u8; 16]>::try_from(raw_id.as_slice()) {
                let cutoff = now_ms.saturating_sub(days.saturating_mul(MS_PER_DAY));
                list.push((SiteId::from_bytes(bytes), cutoff));
            }
        }
        list
    };
    let tx = conn.transaction().map_err(map_sql_err)?;
    let mut total = 0_u64;
    for (site_id, cutoff) in targets {
        total += delete_raw_before(&tx, site_id, cutoff)?;
    }
    tx.commit().map_err(map_sql_err)?;
    Ok(total)
}
