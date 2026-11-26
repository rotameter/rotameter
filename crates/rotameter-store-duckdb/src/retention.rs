#![forbid(unsafe_code)]
//! Retention pruning (`purge_before`) for `DuckDB` OLAP tables.

use duckdb::params;
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::utc_yyyymmdd;

use crate::store::{DuckDbStore, map_duck_err};

impl DuckDbStore {
    /// Synchronously deletes raw and rollup analytics for `site_id` strictly older than `before_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if any deletion fails.
    pub fn purge_before_sync(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError> {
        let cutoff_day = utc_yyyymmdd(before_ms);
        self.with_conn(|conn| {
            conn.execute_batch("BEGIN TRANSACTION")
                .map_err(map_duck_err)?;
            let site_blob = site_id.as_bytes().as_slice();
            let res = (|| {
                let ev_del = conn
                    .execute(
                        "DELETE FROM events WHERE site_id = ?1 AND ts_ms < ?2",
                        params![site_blob, before_ms],
                    )
                    .map_err(map_duck_err)?;
                let sess_del = conn
                    .execute(
                        "DELETE FROM sessions WHERE site_id = ?1 AND last_event_ms < ?2",
                        params![site_blob, before_ms],
                    )
                    .map_err(map_duck_err)?;
                let hr_del = conn
                    .execute(
                        "DELETE FROM rollups_hourly WHERE site_id = ?1 AND bucket_ts_ms < ?2",
                        params![site_blob, before_ms],
                    )
                    .map_err(map_duck_err)?;
                let dy_del = conn
                    .execute(
                        "DELETE FROM rollups_daily WHERE site_id = ?1 AND day_utc < ?2",
                        params![site_blob, cutoff_day],
                    )
                    .map_err(map_duck_err)?;
                let pd_del = conn
                    .execute(
                        "DELETE FROM rollups_path_daily WHERE site_id = ?1 AND day_utc < ?2",
                        params![site_blob, cutoff_day],
                    )
                    .map_err(map_duck_err)?;
                Ok(u64::try_from(ev_del + sess_del + hr_del + dy_del + pd_del).unwrap_or(0))
            })();
            match res {
                Ok(count) => {
                    conn.execute_batch("COMMIT").map_err(map_duck_err)?;
                    Ok(count)
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(err)
                }
            }
        })
    }
}
