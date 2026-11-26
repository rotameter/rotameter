#![forbid(unsafe_code)]
//! Arrow `RecordBatch` <-> `IngestEvent` codec (`events` schema v1).

use std::sync::Arc;

use arrow_array::builder::{FixedSizeBinaryBuilder, Int64Builder, StringBuilder, UInt32Builder};
use arrow_array::{ArrayRef, RecordBatch};
use rotameter_core::storage::IngestEvent;
use rotameter_core::types::EventKind;

use crate::ArrowError;
use crate::col_ext::{bytes16, col_bin16, col_i64, col_str, col_u32, decode_props, site_id_at};
use crate::schema::events_schema;

/// Encodes `events` into a canonical [`RecordBatch`] matching [`events_schema`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow construction or JSON serialization fails.
pub fn events_to_record_batch(events: &[IngestEvent]) -> Result<RecordBatch, ArrowError> {
    let n = events.len();
    let mut site_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut ts_ms = Int64Builder::with_capacity(n);
    let mut session_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut seqs = UInt32Builder::with_capacity(n);
    let mut visitor_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut kinds = StringBuilder::with_capacity(n, n * 16);
    let mut paths = StringBuilder::with_capacity(n, n * 32);
    let mut props = StringBuilder::with_capacity(n, n * 16);

    for ev in events {
        site_ids.append_value(ev.site_id.as_bytes())?;
        ts_ms.append_value(ev.ts_ms);
        session_ids.append_value(ev.session_id)?;
        seqs.append_value(ev.seq_in_session);
        visitor_ids.append_value(ev.visitor_id)?;
        kinds.append_value(ev.event_kind.as_str());
        paths.append_value(&ev.path);
        if ev.props.is_empty() {
            props.append_null();
        } else {
            props.append_value(serde_json::to_string(ev.props.as_map())?);
        }
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(site_ids.finish()),
        Arc::new(ts_ms.finish()),
        Arc::new(session_ids.finish()),
        Arc::new(seqs.finish()),
        Arc::new(visitor_ids.finish()),
        Arc::new(kinds.finish()),
        Arc::new(paths.finish()),
        Arc::new(props.finish()),
    ];
    Ok(RecordBatch::try_new(events_schema(), columns)?)
}

/// Decodes a [`RecordBatch`] produced with [`events_schema`] back into [`IngestEvent`] rows.
///
/// # Errors
///
/// Returns [`ArrowError`] if required columns are missing or contain invalid values.
pub fn record_batch_to_events(batch: &RecordBatch) -> Result<Vec<IngestEvent>, ArrowError> {
    let site_ids = col_bin16(batch, "site_id")?;
    let ts_ms = col_i64(batch, "ts_ms")?;
    let session_ids = col_bin16(batch, "session_id")?;
    let seqs = col_u32(batch, "seq_in_session")?;
    let visitor_ids = col_bin16(batch, "visitor_id")?;
    let kinds = col_str(batch, "event_kind")?;
    let paths = col_str(batch, "path")?;
    let props = col_str(batch, "props_json")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(IngestEvent {
            site_id: site_id_at(site_ids, row)?,
            ts_ms: ts_ms.value(row),
            session_id: bytes16(session_ids, row, "session_id")?,
            seq_in_session: seqs.value(row),
            visitor_id: bytes16(visitor_ids, row, "visitor_id")?,
            event_kind: EventKind::parse(kinds.value(row))?,
            path: paths.value(row).to_string(),
            props: decode_props(props, row)?,
        });
    }
    Ok(out)
}
