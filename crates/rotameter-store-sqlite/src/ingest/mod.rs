#![forbid(unsafe_code)]
//! `EventIngestStore` implementation for [`SqliteStore`].

mod writer;

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::VisitorId;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::storage::{EventIngestStore, WriteBatch};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::config::map_sql_err;
use crate::row_decode::decode_session_snapshot;
use crate::store::SqliteStore;

impl EventIngestStore for SqliteStore {
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> Result<Option<SessionSnapshot>, CoreError> {
        self.with_reader(|conn| {
            let mut snap = conn
                .query_row(
                    "SELECT session_id, visitor_id, started_at_ms, last_event_ms,
                            event_count, pageview_count
                     FROM sessions
                     WHERE site_id = ?1 AND visitor_id = ?2 AND last_event_ms >= ?3
                     ORDER BY last_event_ms DESC
                     LIMIT 1",
                    params![
                        site_id.as_bytes().as_slice(),
                        visitor_id.as_slice(),
                        min_last_event_ms
                    ],
                    decode_session_snapshot,
                )
                .optional()
                .map_err(map_sql_err)?;

            if let Some(ref mut found) = snap {
                let max_seq: i64 = conn
                    .query_row(
                        "SELECT COALESCE(MAX(seq_in_session), 0)
                         FROM events WHERE site_id = ?1 AND session_id = ?2",
                        params![site_id.as_bytes().as_slice(), found.session_id.as_slice()],
                        |row| row.get(0),
                    )
                    .map_err(map_sql_err)?;
                let seq_u32 = u32::try_from(max_seq.max(0)).unwrap_or(u32::MAX);
                found.last_seq = found.event_count.max(seq_u32);
            }
            Ok(snap)
        })
    }

    fn write_batch(&self, batch: &WriteBatch) -> Result<(), CoreError> {
        if batch.events.is_empty() && batch.sessions.is_empty() && batch.rollups.is_empty() {
            return Ok(());
        }
        self.with_writer(|state| {
            let tx = state
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_sql_err)?;
            for ev in &batch.events {
                writer::write_event(&tx, &mut state.interner, ev)?;
            }
            for sess in &batch.sessions {
                writer::write_session(&tx, &mut state.interner, sess)?;
            }
            for rollup in &batch.rollups {
                writer::write_rollup(&tx, rollup)?;
            }
            tx.commit().map_err(map_sql_err)
        })
    }
}
