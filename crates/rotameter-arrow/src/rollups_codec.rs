#![forbid(unsafe_code)]
//! Arrow `RecordBatch` <-> rollup row codecs (`rollups_hourly`, `rollups_daily`, `rollups_path_daily`).

use std::sync::Arc;

use arrow_array::builder::{FixedSizeBinaryBuilder, Int32Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use rotameter_core::storage::{RollupDailyRow, RollupHourlyRow, RollupPathDailyRow};

use crate::ArrowError;
use crate::col_ext::{col_bin16, col_i32, col_i64, col_str, site_id_at};
use crate::schema::{rollups_daily_schema, rollups_hourly_schema, rollups_path_daily_schema};

/// Encodes [`RollupHourlyRow`] rows into a canonical [`RecordBatch`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow construction fails.
pub fn rollups_hourly_to_record_batch(rows: &[RollupHourlyRow]) -> Result<RecordBatch, ArrowError> {
    let n = rows.len();
    let mut site_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut bucket = Int64Builder::with_capacity(n);
    let mut pv = Int64Builder::with_capacity(n);
    let mut ev = Int64Builder::with_capacity(n);
    let mut bounced = Int64Builder::with_capacity(n);
    let mut visits = Int64Builder::with_capacity(n);
    let mut dur = Int64Builder::with_capacity(n);

    for r in rows {
        site_ids.append_value(r.site_id.as_bytes())?;
        bucket.append_value(r.bucket_ts_ms);
        pv.append_value(r.pageviews);
        ev.append_value(r.events_total);
        bounced.append_value(r.bounced_visits);
        visits.append_value(r.visits_started);
        dur.append_value(r.total_duration_ms);
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(site_ids.finish()),
        Arc::new(bucket.finish()),
        Arc::new(pv.finish()),
        Arc::new(ev.finish()),
        Arc::new(bounced.finish()),
        Arc::new(visits.finish()),
        Arc::new(dur.finish()),
    ];
    Ok(RecordBatch::try_new(rollups_hourly_schema(), columns)?)
}

/// Decodes a [`RecordBatch`] produced with [`rollups_hourly_schema`].
///
/// # Errors
///
/// Returns [`ArrowError`] if required columns are missing or invalid.
pub fn record_batch_to_rollups_hourly(
    batch: &RecordBatch,
) -> Result<Vec<RollupHourlyRow>, ArrowError> {
    let site_ids = col_bin16(batch, "site_id")?;
    let bucket = col_i64(batch, "bucket_ts_ms")?;
    let pv = col_i64(batch, "pageviews")?;
    let ev = col_i64(batch, "events_total")?;
    let bounced = col_i64(batch, "bounced_visits")?;
    let visits = col_i64(batch, "visits_started")?;
    let dur = col_i64(batch, "total_duration_ms")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(RollupHourlyRow {
            site_id: site_id_at(site_ids, row)?,
            bucket_ts_ms: bucket.value(row),
            pageviews: pv.value(row),
            events_total: ev.value(row),
            bounced_visits: bounced.value(row),
            visits_started: visits.value(row),
            total_duration_ms: dur.value(row),
        });
    }
    Ok(out)
}

/// Encodes [`RollupDailyRow`] rows into a canonical [`RecordBatch`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow construction fails.
pub fn rollups_daily_to_record_batch(rows: &[RollupDailyRow]) -> Result<RecordBatch, ArrowError> {
    let n = rows.len();
    let mut site_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut day = Int32Builder::with_capacity(n);
    let mut pv = Int64Builder::with_capacity(n);
    let mut ev = Int64Builder::with_capacity(n);
    let mut bounced = Int64Builder::with_capacity(n);
    let mut visits = Int64Builder::with_capacity(n);
    let mut dur = Int64Builder::with_capacity(n);
    let mut visitors = Int64Builder::with_capacity(n);

    for r in rows {
        site_ids.append_value(r.site_id.as_bytes())?;
        day.append_value(r.day_utc);
        pv.append_value(r.pageviews);
        ev.append_value(r.events_total);
        bounced.append_value(r.bounced_visits);
        visits.append_value(r.visits_started);
        dur.append_value(r.total_duration_ms);
        visitors.append_value(r.visitors);
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(site_ids.finish()),
        Arc::new(day.finish()),
        Arc::new(pv.finish()),
        Arc::new(ev.finish()),
        Arc::new(bounced.finish()),
        Arc::new(visits.finish()),
        Arc::new(dur.finish()),
        Arc::new(visitors.finish()),
    ];
    Ok(RecordBatch::try_new(rollups_daily_schema(), columns)?)
}

/// Decodes a [`RecordBatch`] produced with [`rollups_daily_schema`].
///
/// # Errors
///
/// Returns [`ArrowError`] if required columns are missing or invalid.
pub fn record_batch_to_rollups_daily(
    batch: &RecordBatch,
) -> Result<Vec<RollupDailyRow>, ArrowError> {
    let site_ids = col_bin16(batch, "site_id")?;
    let day = col_i32(batch, "day_utc")?;
    let pv = col_i64(batch, "pageviews")?;
    let ev = col_i64(batch, "events_total")?;
    let bounced = col_i64(batch, "bounced_visits")?;
    let visits = col_i64(batch, "visits_started")?;
    let dur = col_i64(batch, "total_duration_ms")?;
    let visitors = col_i64(batch, "visitors")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(RollupDailyRow {
            site_id: site_id_at(site_ids, row)?,
            day_utc: day.value(row),
            pageviews: pv.value(row),
            events_total: ev.value(row),
            bounced_visits: bounced.value(row),
            visits_started: visits.value(row),
            total_duration_ms: dur.value(row),
            visitors: visitors.value(row),
        });
    }
    Ok(out)
}

/// Encodes [`RollupPathDailyRow`] rows into a canonical [`RecordBatch`].
///
/// # Errors
///
/// Returns [`ArrowError`] if Arrow construction fails.
pub fn rollups_path_daily_to_record_batch(
    rows: &[RollupPathDailyRow],
) -> Result<RecordBatch, ArrowError> {
    let n = rows.len();
    let mut site_ids = FixedSizeBinaryBuilder::with_capacity(n, 16);
    let mut day = Int32Builder::with_capacity(n);
    let mut paths = StringBuilder::with_capacity(n, n * 24);
    let mut pv = Int64Builder::with_capacity(n);
    let mut ev = Int64Builder::with_capacity(n);
    let mut visits = Int64Builder::with_capacity(n);
    let mut visitors = Int64Builder::with_capacity(n);
    let mut bounced = Int64Builder::with_capacity(n);
    let mut dur = Int64Builder::with_capacity(n);

    for r in rows {
        site_ids.append_value(r.site_id.as_bytes())?;
        day.append_value(r.day_utc);
        paths.append_value(&r.path);
        pv.append_value(r.pageviews);
        ev.append_value(r.events_total);
        visits.append_value(r.visits);
        visitors.append_value(r.visitors);
        bounced.append_value(r.bounced_visits);
        dur.append_value(r.total_duration_ms);
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(site_ids.finish()),
        Arc::new(day.finish()),
        Arc::new(paths.finish()),
        Arc::new(pv.finish()),
        Arc::new(ev.finish()),
        Arc::new(visits.finish()),
        Arc::new(visitors.finish()),
        Arc::new(bounced.finish()),
        Arc::new(dur.finish()),
    ];
    Ok(RecordBatch::try_new(rollups_path_daily_schema(), columns)?)
}

/// Decodes a [`RecordBatch`] produced with [`rollups_path_daily_schema`].
///
/// # Errors
///
/// Returns [`ArrowError`] if required columns are missing or invalid.
pub fn record_batch_to_rollups_path_daily(
    batch: &RecordBatch,
) -> Result<Vec<RollupPathDailyRow>, ArrowError> {
    let site_ids = col_bin16(batch, "site_id")?;
    let day = col_i32(batch, "day_utc")?;
    let paths = col_str(batch, "path")?;
    let pv = col_i64(batch, "pageviews")?;
    let ev = col_i64(batch, "events_total")?;
    let visits = col_i64(batch, "visits")?;
    let visitors = col_i64(batch, "visitors")?;
    let bounced = col_i64(batch, "bounced_visits")?;
    let dur = col_i64(batch, "total_duration_ms")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        out.push(RollupPathDailyRow {
            site_id: site_id_at(site_ids, row)?,
            day_utc: day.value(row),
            path: paths.value(row).to_string(),
            pageviews: pv.value(row),
            events_total: ev.value(row),
            visits: visits.value(row),
            visitors: visitors.value(row),
            bounced_visits: bounced.value(row),
            total_duration_ms: dur.value(row),
        });
    }
    Ok(out)
}
