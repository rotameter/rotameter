#![forbid(unsafe_code)]
//! `WriteBatch <-> RecordBatch` adapters (`events`, `sessions`, and rollup deltas).

use std::collections::BTreeMap;

use arrow_array::RecordBatch;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::utc_yyyymmdd;
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::EventKind;

use crate::ArrowError;
use crate::events_codec::{events_to_record_batch, record_batch_to_events};
use crate::sessions_codec::{record_batch_to_sessions, sessions_to_record_batch};

/// Arrow columnar pair representing an atomic [`WriteBatch`].
#[derive(Debug, Clone)]
pub struct WriteBatchArrow {
    /// Raw ordered events batch ([`crate::events_schema`]).
    pub events: RecordBatch,
    /// Upserted 30-minute sliding sessions ([`crate::sessions_schema`]).
    pub sessions: RecordBatch,
    /// Aggregated rollup counter deltas preserved for OLAP write replay.
    pub rollups: Vec<RollupDelta>,
}

/// Converts a [`WriteBatch`] into Arrow [`RecordBatch`] values.
///
/// # Errors
///
/// Returns [`ArrowError`] if encoding `events` or `sessions` fails.
pub fn write_batch_to_arrow(batch: &WriteBatch) -> Result<WriteBatchArrow, ArrowError> {
    Ok(WriteBatchArrow {
        events: events_to_record_batch(&batch.events)?,
        sessions: sessions_to_record_batch(&batch.sessions)?,
        rollups: batch.rollups.clone(),
    })
}

/// Reconstructs a [`WriteBatch`] from its Arrow representation.
///
/// # Errors
///
/// Returns [`ArrowError`] if decoding `events` or `sessions` fails.
pub fn arrow_to_write_batch(arrow: &WriteBatchArrow) -> Result<WriteBatch, ArrowError> {
    let events = record_batch_to_events(&arrow.events)?;
    let sessions = record_batch_to_sessions(&arrow.sessions)?;
    let rollups = if arrow.rollups.is_empty() {
        derive_basic_rollups(&events, &sessions)
    } else {
        arrow.rollups.clone()
    };
    Ok(WriteBatch {
        events,
        sessions,
        rollups,
    })
}

fn derive_basic_rollups(events: &[IngestEvent], sessions: &[SessionUpsert]) -> Vec<RollupDelta> {
    let mut map: BTreeMap<(SiteId, i64, i32), RollupDelta> = BTreeMap::new();
    for ev in events {
        let hour = ev.ts_ms - ev.ts_ms.rem_euclid(3_600_000);
        let day = utc_yyyymmdd(ev.ts_ms);
        let delta = map.entry((ev.site_id, hour, day)).or_insert(RollupDelta {
            site_id: ev.site_id,
            hour_bucket_ms: hour,
            day_utc: day,
            pageviews: 0,
            events_total: 0,
            bounced_visits: 0,
            visits_started: 0,
            total_duration_ms: 0,
        });
        delta.events_total += 1;
        if ev.event_kind == EventKind::Pageview {
            delta.pageviews += 1;
        }
    }
    for sess in sessions {
        let hour = sess.started_at_ms - sess.started_at_ms.rem_euclid(3_600_000);
        let day = utc_yyyymmdd(sess.started_at_ms);
        let delta = map.entry((sess.site_id, hour, day)).or_insert(RollupDelta {
            site_id: sess.site_id,
            hour_bucket_ms: hour,
            day_utc: day,
            pageviews: 0,
            events_total: 0,
            bounced_visits: 0,
            visits_started: 0,
            total_duration_ms: 0,
        });
        delta.visits_started += 1;
        if sess.event_count <= 1 {
            delta.bounced_visits += 1;
        }
        delta.total_duration_ms += (sess.last_event_ms - sess.started_at_ms).max(0);
    }
    map.into_values().collect()
}
