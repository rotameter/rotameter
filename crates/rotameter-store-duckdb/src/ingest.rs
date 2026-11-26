#![forbid(unsafe_code)]
//! Zero-copy Arrow `RecordBatch` ingestion and `EventIngestStore` implementation for [`DuckDbStore`].

use arrow_array::RecordBatch;
use duckdb::{Connection, OptionalExt, params};
use rotameter_arrow::{events_to_record_batch, record_batch_to_sessions, sessions_to_record_batch};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::VisitorId;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::storage::{BoxFuture, EventIngestStore, SessionUpsert, WriteBatch};

use crate::ingest_rollups::upsert_rollup_delta;
use crate::store::{DuckDbStore, map_duck_err};

impl DuckDbStore {
    /// Synchronously looks up the latest active session for `(site_id, visitor_id)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query failure.
    pub fn lookup_active_session_sync(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> Result<Option<SessionSnapshot>, CoreError> {
        self.with_conn(|conn| {
            let mut snap = conn
                .query_row(
                    "SELECT session_id, visitor_id, started_at_ms, last_event_ms,
                            CAST(event_count AS BIGINT), CAST(pageview_count AS BIGINT)
                     FROM sessions
                     WHERE site_id = ?1 AND visitor_id = ?2 AND last_event_ms >= ?3
                     ORDER BY last_event_ms DESC
                     LIMIT 1",
                    params![
                        site_id.as_bytes().as_slice(),
                        visitor_id.as_slice(),
                        min_last_event_ms
                    ],
                    |row| {
                        let s_blob: Vec<u8> = row.get(0)?;
                        let v_blob: Vec<u8> = row.get(1)?;
                        let ec_i64: i64 = row.get(4)?;
                        let pvc_i64: i64 = row.get(5)?;
                        let session_id: [u8; 16] = s_blob.try_into().map_err(|_| {
                            duckdb::Error::InvalidColumnType(
                                0,
                                "session_id".to_string(),
                                duckdb::types::Type::Blob,
                            )
                        })?;
                        let visitor_id: [u8; 16] = v_blob.try_into().map_err(|_| {
                            duckdb::Error::InvalidColumnType(
                                1,
                                "visitor_id".to_string(),
                                duckdb::types::Type::Blob,
                            )
                        })?;
                        let ec = u32::try_from(ec_i64.max(0)).unwrap_or(u32::MAX);
                        let pvc = u32::try_from(pvc_i64.max(0)).unwrap_or(u32::MAX);
                        Ok(SessionSnapshot {
                            session_id,
                            visitor_id,
                            started_at_ms: row.get(2)?,
                            last_event_ms: row.get(3)?,
                            event_count: ec,
                            pageview_count: pvc,
                            last_seq: ec,
                        })
                    },
                )
                .optional()
                .map_err(map_duck_err)?;

            if let Some(ref mut found) = snap {
                let max_seq: i64 = conn
                    .query_row(
                        "SELECT CAST(COALESCE(MAX(seq_in_session), 0) AS BIGINT)
                         FROM events WHERE site_id = ?1 AND session_id = ?2",
                        params![site_id.as_bytes().as_slice(), found.session_id.as_slice()],
                        |r| r.get(0),
                    )
                    .map_err(map_duck_err)?;
                let seq_u32 = u32::try_from(max_seq.max(0)).unwrap_or(u32::MAX);
                found.last_seq = found.event_count.max(seq_u32);
            }
            Ok(snap)
        })
    }

    /// Synchronously persists an atomic [`WriteBatch`] via zero-copy Arrow appending.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on encoding, transaction, or appender failure.
    pub fn write_batch_sync(&self, batch: &WriteBatch) -> Result<(), CoreError> {
        if batch.events.is_empty() && batch.sessions.is_empty() && batch.rollups.is_empty() {
            return Ok(());
        }
        let events_rb = (!batch.events.is_empty())
            .then(|| events_to_record_batch(&batch.events))
            .transpose()
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        let sessions_rb = (!batch.sessions.is_empty())
            .then(|| sessions_to_record_batch(&batch.sessions))
            .transpose()
            .map_err(|err| CoreError::Storage(err.to_string()))?;

        self.with_conn(|conn| {
            run_tx(conn, || {
                if let Some(rb) = events_rb {
                    append_batch(conn, "events", rb)?;
                }
                if let Some(rb) = sessions_rb {
                    delete_sessions_keys(conn, &batch.sessions)?;
                    append_batch(conn, "sessions", rb)?;
                }
                for r in &batch.rollups {
                    upsert_rollup_delta(conn, r)?;
                }
                Ok(())
            })
        })
    }

    /// Zero-copy appends an Arrow [`RecordBatch`] matching [`rotameter_arrow::events_schema`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if appending fails.
    pub fn append_events_batch_sync(&self, batch: RecordBatch) -> Result<(), CoreError> {
        if batch.num_rows() == 0 {
            return Ok(());
        }
        self.with_conn(|conn| run_tx(conn, || append_batch(conn, "events", batch)))
    }

    /// Zero-copy upserts an Arrow [`RecordBatch`] matching [`rotameter_arrow::sessions_schema`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if decoding keys or appending fails.
    pub fn append_sessions_batch_sync(&self, batch: RecordBatch) -> Result<(), CoreError> {
        if batch.num_rows() == 0 {
            return Ok(());
        }
        let rows = record_batch_to_sessions(&batch)
            .map_err(|err| CoreError::Storage(format!("arrow sessions decode failed: {err}")))?;
        self.with_conn(|conn| {
            run_tx(conn, || {
                delete_sessions_keys(conn, &rows)?;
                append_batch(conn, "sessions", batch)
            })
        })
    }

    /// Zero-copy appends an Arrow [`RecordBatch`] matching `rollups_hourly`, `rollups_daily`,
    /// or `rollups_path_daily` schemas into the named target table.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if `table` is unknown or appending fails.
    pub fn append_rollup_batch_sync(
        &self,
        table: &str,
        batch: RecordBatch,
    ) -> Result<(), CoreError> {
        if !matches!(
            table,
            "rollups_hourly" | "rollups_daily" | "rollups_path_daily"
        ) {
            return Err(CoreError::Storage(format!(
                "unsupported DuckDB rollup table `{table}`"
            )));
        }
        if batch.num_rows() == 0 {
            return Ok(());
        }
        self.with_conn(|conn| run_tx(conn, || append_batch(conn, table, batch)))
    }
}

impl EventIngestStore for DuckDbStore {
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> BoxFuture<'_, Result<Option<SessionSnapshot>, CoreError>> {
        Box::pin(std::future::ready(self.lookup_active_session_sync(
            site_id,
            visitor_id,
            min_last_event_ms,
        )))
    }

    fn write_batch<'a>(&'a self, batch: &'a WriteBatch) -> BoxFuture<'a, Result<(), CoreError>> {
        Box::pin(std::future::ready(self.write_batch_sync(batch)))
    }
}

fn run_tx<F>(conn: &Connection, f: F) -> Result<(), CoreError>
where
    F: FnOnce() -> Result<(), CoreError>,
{
    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(map_duck_err)?;
    match f() {
        Ok(()) => conn.execute_batch("COMMIT").map_err(map_duck_err),
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn append_batch(conn: &Connection, table: &str, batch: RecordBatch) -> Result<(), CoreError> {
    let mut appender = conn.appender(table).map_err(map_duck_err)?;
    appender.append_record_batch(batch).map_err(map_duck_err)?;
    appender.flush().map_err(map_duck_err)
}

fn delete_sessions_keys(conn: &Connection, sessions: &[SessionUpsert]) -> Result<(), CoreError> {
    let mut stmt = conn
        .prepare("DELETE FROM sessions WHERE site_id = ?1 AND session_id = ?2")
        .map_err(map_duck_err)?;
    for s in sessions {
        stmt.execute(params![
            s.site_id.as_bytes().as_slice(),
            s.session_id.as_slice()
        ])
        .map_err(map_duck_err)?;
    }
    Ok(())
}
