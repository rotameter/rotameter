#![forbid(unsafe_code)]
//! Partitioned Arrow/Parquet analytics bundle exporter for [`DuckDbStore`].

use std::collections::BTreeMap;
use std::path::Path;

use chrono::DateTime;
use rotameter_arrow::{
    PartitionedParquetWriter, PortableRowCounts, events_schema, events_to_record_batch,
    rollups_daily_schema, rollups_daily_to_record_batch, rollups_hourly_schema,
    rollups_hourly_to_record_batch, rollups_path_daily_schema, rollups_path_daily_to_record_batch,
    sessions_schema, sessions_to_record_batch, sha256_file, write_parquet_batches,
};
use rotameter_core::error::CoreError;
use rotameter_core::storage::{IngestEvent, SessionUpsert};

use crate::store::DuckDbStore;

const CHUNK_SIZE: usize = 4_096;

macro_rules! export_duck_rollup {
    ($store:expr, $dir:expr, $files:expr, $rel:expr, $schema:ident, $read:ident, $encode:ident) => {{
        let mut offset = 0_usize;
        let iter = std::iter::from_fn(|| {
            let chunk = match $store.$read(offset, CHUNK_SIZE) {
                Ok(rows) => rows,
                Err(err) => return Some(Err(rotameter_arrow::ArrowError::Core(err))),
            };
            if chunk.is_empty() {
                return None;
            }
            offset += chunk.len();
            Some($encode(&chunk))
        });
        let total = write_parquet_batches(&$dir.join($rel), $schema(), iter)
            .map_err(|err| CoreError::Storage(format!("duckdb parquet write failed: {err}")))?;
        record_hash($dir, $rel, $files)?;
        Ok::<u64, CoreError>(total)
    }};
}

impl DuckDbStore {
    /// Checkpoints `DuckDB` and exports all analytics tables as portable Parquet files into `dir`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if checkpointing, reading, Arrow encoding, or I/O fails.
    pub fn export_analytics_dir_sync(
        &self,
        dir: &Path,
    ) -> Result<(BTreeMap<String, String>, PortableRowCounts), CoreError> {
        self.checkpoint()?;
        std::fs::create_dir_all(dir).map_err(|err| {
            CoreError::Storage(format!("cannot create `{}`: {err}", dir.display()))
        })?;

        let mut files = BTreeMap::new();
        let events = export_events_partitioned(self, dir, &mut files)?;
        let sessions = export_sessions_partitioned(self, dir, &mut files)?;
        let rollups_hourly = export_duck_rollup!(
            self,
            dir,
            &mut files,
            "rollups_hourly.parquet",
            rollups_hourly_schema,
            read_rollups_hourly_chunk_sync,
            rollups_hourly_to_record_batch
        )?;
        let rollups_daily = export_duck_rollup!(
            self,
            dir,
            &mut files,
            "rollups_daily.parquet",
            rollups_daily_schema,
            read_rollups_daily_chunk_sync,
            rollups_daily_to_record_batch
        )?;
        let rollups_path_daily = export_duck_rollup!(
            self,
            dir,
            &mut files,
            "rollups_path_daily.parquet",
            rollups_path_daily_schema,
            read_rollups_path_daily_chunk_sync,
            rollups_path_daily_to_record_batch
        )?;

        Ok((
            files,
            PortableRowCounts {
                events,
                sessions,
                rollups_hourly,
                rollups_daily,
                rollups_path_daily,
                ..PortableRowCounts::default()
            },
        ))
    }
}

fn export_events_partitioned(
    store: &DuckDbStore,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<u64, CoreError> {
    let mut writer = PartitionedParquetWriter::new(dir, "events", events_schema());
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = store.read_events_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let mut by_day: BTreeMap<String, Vec<IngestEvent>> = BTreeMap::new();
        for ev in chunk {
            by_day.entry(utc_day(ev.ts_ms)).or_default().push(ev);
        }
        for (day, rows) in by_day {
            let batch = events_to_record_batch(&rows)
                .map_err(|err| CoreError::Storage(format!("Arrow events encode failed: {err}")))?;
            writer
                .write_day_batch(&day, &batch)
                .map_err(|err| CoreError::Storage(err.to_string()))?;
        }
    }
    for (rel, _) in writer
        .finish()
        .map_err(|err| CoreError::Storage(err.to_string()))?
    {
        record_hash(dir, &rel, files)?;
    }
    Ok(total)
}

fn export_sessions_partitioned(
    store: &DuckDbStore,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<u64, CoreError> {
    let mut writer = PartitionedParquetWriter::new(dir, "sessions", sessions_schema());
    let mut offset = 0_usize;
    let mut total = 0_u64;
    loop {
        let chunk = store.read_sessions_chunk_sync(offset, CHUNK_SIZE)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len();
        total += u64::try_from(chunk.len()).unwrap_or(0);
        let mut by_day: BTreeMap<String, Vec<SessionUpsert>> = BTreeMap::new();
        for s in chunk {
            by_day.entry(utc_day(s.started_at_ms)).or_default().push(s);
        }
        for (day, rows) in by_day {
            let batch = sessions_to_record_batch(&rows).map_err(|err| {
                CoreError::Storage(format!("Arrow sessions encode failed: {err}"))
            })?;
            writer
                .write_day_batch(&day, &batch)
                .map_err(|err| CoreError::Storage(err.to_string()))?;
        }
    }
    for (rel, _) in writer
        .finish()
        .map_err(|err| CoreError::Storage(err.to_string()))?
    {
        record_hash(dir, &rel, files)?;
    }
    Ok(total)
}

fn record_hash(
    root: &Path,
    rel: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<(), CoreError> {
    let digest = sha256_file(&root.join(rel)).map_err(|err| CoreError::Storage(err.to_string()))?;
    files.insert(rel.to_string(), digest);
    Ok(())
}

fn utc_day(ts_ms: i64) -> String {
    DateTime::from_timestamp_millis(ts_ms).map_or_else(
        || "1970-01-01".to_string(),
        |dt| dt.format("%Y-%m-%d").to_string(),
    )
}
