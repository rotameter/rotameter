#![forbid(unsafe_code)]
//! Streaming Zstd-compressed Parquet file writers and chunked [`RecordBatch`] readers.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use crate::ArrowError;

/// Writes a stream/iterator of [`RecordBatch`] values into a Zstd-compressed Parquet file.
///
/// # Errors
///
/// Returns [`ArrowError`] on parent directory creation, Parquet encoding, or flush errors.
pub fn write_parquet_batches<I>(
    path: &Path,
    schema: SchemaRef,
    batches: I,
) -> Result<u64, ArrowError>
where
    I: IntoIterator<Item = Result<RecordBatch, ArrowError>>,
{
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    let mut rows = 0_u64;
    for batch_res in batches {
        let batch = batch_res?;
        rows += u64::try_from(batch.num_rows()).unwrap_or(0);
        writer.write(&batch)?;
    }
    writer.close()?;
    Ok(rows)
}

/// Streams a Parquet file in bounded `RecordBatch` chunks of at most `batch_size` rows.
///
/// # Errors
///
/// Returns [`ArrowError`] if opening the file, decoding Parquet, or invoking `on_batch` fails.
pub fn stream_parquet_batches<F>(
    path: &Path,
    batch_size: usize,
    mut on_batch: F,
) -> Result<u64, ArrowError>
where
    F: FnMut(RecordBatch) -> Result<(), ArrowError>,
{
    let file = File::open(path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?
        .with_batch_size(batch_size.max(1))
        .build()?;
    let mut total = 0_u64;
    for next in reader {
        let batch = next?;
        total += u64::try_from(batch.num_rows()).unwrap_or(0);
        on_batch(batch)?;
    }
    Ok(total)
}

/// Appends [`RecordBatch`] slices into Hive-partitioned `<subdir>/day=YYYY-MM-DD.parquet` files.
pub struct PartitionedParquetWriter {
    root: PathBuf,
    subdir: &'static str,
    schema: SchemaRef,
    writers: BTreeMap<String, (ArrowWriter<File>, u64)>,
}

impl PartitionedParquetWriter {
    /// Creates a new partitioned writer targeting `root/subdir`.
    pub fn new(root: &Path, subdir: &'static str, schema: SchemaRef) -> Self {
        Self {
            root: root.to_path_buf(),
            subdir,
            schema,
            writers: BTreeMap::new(),
        }
    }

    /// Writes `batch` into the `day=<day>.parquet` partition, creating the file lazily.
    ///
    /// # Errors
    ///
    /// Returns [`ArrowError`] on directory creation or Parquet write failure.
    pub fn write_day_batch(&mut self, day: &str, batch: &RecordBatch) -> Result<(), ArrowError> {
        if batch.num_rows() == 0 {
            return Ok(());
        }
        if !self.writers.contains_key(day) {
            let path = self
                .root
                .join(self.subdir)
                .join(format!("day={day}.parquet"));
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let file = File::create(path)?;
            let props = WriterProperties::builder()
                .set_compression(Compression::ZSTD(ZstdLevel::default()))
                .build();
            let writer = ArrowWriter::try_new(file, self.schema.clone(), Some(props))?;
            self.writers.insert(day.to_string(), (writer, 0));
        }
        if let Some((writer, count)) = self.writers.get_mut(day) {
            writer.write(batch)?;
            *count += u64::try_from(batch.num_rows()).unwrap_or(0);
        }
        Ok(())
    }

    /// Closes every open Parquet partition writer and returns `(relative_path, row_count)` pairs.
    ///
    /// # Errors
    ///
    /// Returns [`ArrowError`] if flushing any Parquet footer fails.
    pub fn finish(self) -> Result<Vec<(String, u64)>, ArrowError> {
        let mut out = Vec::with_capacity(self.writers.len());
        for (day, (writer, count)) in self.writers {
            writer.close()?;
            out.push((format!("{}/day={day}.parquet", self.subdir), count));
        }
        Ok(out)
    }
}
